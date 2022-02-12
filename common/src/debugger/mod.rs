pub mod adapter;
pub mod dap_types;
mod protocol;

use crate::debugger::adapter::DebugAdapter;
use crate::debugger::adapter::DebugAdapterError;
use crate::debugger::adapter::DebugAdapterResult;
use crate::debugger::dap_types::Event;
use crate::debugger::dap_types::InitializeArguments;
use crate::debugger::dap_types::Message;
use crate::debugger::dap_types::MessageEnvelope;
use crate::debugger::dap_types::Request;
use crate::debugger::dap_types::Response;
use crate::debugger::dap_types::ResponseEnvelope;
use crate::debugger::dap_types::StackTraceResponse;
use crate::debugger::dap_types::StopReason;
use crate::debugger::dap_types::StoppedEvent;
use crate::debugger::dap_types::Thread;
use crate::debugger::dap_types::ThreadsResponse;
use std::sync::mpsc::TryRecvError;

/// A debugger for 6502-based machines. Uses Debug Adapter Protocol internally
/// to communicate with a debugger UI.
pub struct Debugger<A: DebugAdapter> {
    adapter: A,
    sequence_number: i64,
}

impl<A: DebugAdapter> Debugger<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            sequence_number: 0,
        }
    }

    pub fn process_meessages(&mut self) {
        loop {
            match self.adapter.try_receive_message() {
                Ok(envelope) => self.process_message(envelope),
                Err(DebugAdapterError::TryRecvError(TryRecvError::Empty)) => return,
                Err(e) => panic!("{}", e),
            }
        }
    }

    fn process_message(&mut self, envelope: MessageEnvelope) {
        let message_seq = envelope.seq;
        match envelope.message {
            Message::Request(Request::Initialize(args)) => self.initialize(message_seq, args),
            Message::Request(Request::SetExceptionBreakpoints {}) => {
                self.set_exception_breakpoints(message_seq)
            }
            Message::Request(Request::Attach {}) => self.attach(message_seq),
            Message::Request(Request::Threads) => self.threads(message_seq),
            Message::Request(Request::StackTrace {}) => self.stack_trace(message_seq),
            other => eprintln!("Unsupported message: {:?}", other),
        }
    }

    fn initialize(&mut self, request_seq: i64, args: InitializeArguments) {
        eprintln!(
            "Initializing debugger session with {}",
            args.client_name.as_deref().unwrap_or("an unnamed client")
        );
        self.send_message(MessageEnvelope {
            seq: -1,
            message: Message::Response(ResponseEnvelope {
                request_seq,
                success: true,
                response: Response::Initialize,
            }),
        })
        .unwrap();
        self.send_message(MessageEnvelope {
            seq: -1,
            message: Message::Event(Event::Initialized),
        })
        .unwrap();
    }

    fn set_exception_breakpoints(&mut self, request_seq: i64) {
        self.send_message(MessageEnvelope {
            seq: -1,
            message: Message::Response(ResponseEnvelope {
                request_seq,
                success: true,
                response: Response::SetExceptionBreakpoints,
            }),
        })
        .unwrap();
    }

    fn attach(&mut self, request_seq: i64) {
        self.send_message(MessageEnvelope {
            seq: -1,
            message: Message::Response(ResponseEnvelope {
                request_seq,
                success: true,
                response: Response::Attach,
            }),
        })
        .unwrap();
        self.send_message(MessageEnvelope {
            seq: -1,
            message: Message::Event(Event::Stopped(StoppedEvent {
                reason: StopReason::Entry,
                thread_id: 1,
                all_threads_stopped: true,
            })),
        })
        .unwrap();
    }

    fn threads(&mut self, request_seq: i64) {
        self.send_message(MessageEnvelope {
            seq: -1,
            message: Message::Response(ResponseEnvelope {
                request_seq,
                success: true,
                response: Response::Threads(ThreadsResponse {
                    threads: vec![Thread {
                        id: 1,
                        name: "main thread".to_string(),
                    }],
                }),
            }),
        })
        .unwrap();
    }

    fn stack_trace(&mut self, request_seq: i64) {
        self.send_message(MessageEnvelope {
            seq: -1,
            message: Message::Response(ResponseEnvelope {
                request_seq,
                success: true,
                response: Response::StackTrace(StackTraceResponse {
                    stack_frames: vec![],
                    total_frames: 0,
                }),
            }),
        })
        .unwrap();
    }

    fn send_message(&mut self, mut message: MessageEnvelope) -> DebugAdapterResult<()> {
        message.seq = self.next_sequence_number();
        return self.adapter.send_message(message);
    }

    fn next_sequence_number(&mut self) -> i64 {
        self.sequence_number += 1;
        return self.sequence_number;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debugger::adapter::DebugAdapterResult;
    use crate::debugger::dap_types::InitializeArguments;
    use crate::debugger::dap_types::MessageEnvelope;
    use std::assert_matches::assert_matches;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    fn initialize_request() -> DebugAdapterResult<MessageEnvelope> {
        Ok(MessageEnvelope {
            seq: 5,
            message: Message::Request(Request::Initialize(InitializeArguments {
                client_name: Some("Visual Studio Code".into()),
            })),
        })
    }

    fn set_exception_breakpoints_request() -> DebugAdapterResult<MessageEnvelope> {
        Ok(MessageEnvelope {
            seq: 6,
            message: Message::Request(Request::SetExceptionBreakpoints {}),
        })
    }

    fn attach_request() -> DebugAdapterResult<MessageEnvelope> {
        Ok(MessageEnvelope {
            seq: 8,
            message: Message::Request(Request::Attach {}),
        })
    }

    fn threads_request() -> DebugAdapterResult<MessageEnvelope> {
        Ok(MessageEnvelope {
            seq: 10,
            message: Message::Request(Request::Threads {}),
        })
    }

    fn stack_trace_request() -> DebugAdapterResult<MessageEnvelope> {
        Ok(MessageEnvelope {
            seq: 15,
            message: Message::Request(Request::StackTrace {}),
        })
    }

    #[derive(Default)]
    struct FakeDebugAdapterInternals {
        receiver_queue: VecDeque<DebugAdapterResult<MessageEnvelope>>,
        sender_queue: VecDeque<MessageEnvelope>,
    }

    fn push_incoming(
        adapter_internals: &RefCell<FakeDebugAdapterInternals>,
        message: DebugAdapterResult<MessageEnvelope>,
    ) {
        adapter_internals
            .borrow_mut()
            .receiver_queue
            .push_back(message);
    }

    fn pop_outgoing(
        adapter_internals: &RefCell<FakeDebugAdapterInternals>,
    ) -> Option<MessageEnvelope> {
        adapter_internals.borrow_mut().sender_queue.pop_front()
    }

    #[derive(Default)]
    struct FakeDebugAdapter {
        internals: Rc<RefCell<FakeDebugAdapterInternals>>,
    }

    impl FakeDebugAdapter {
        fn new() -> (Self, Rc<RefCell<FakeDebugAdapterInternals>>) {
            let adapter = Self::default();
            let internals = adapter.internals.clone();
            return (adapter, internals);
        }
    }

    impl DebugAdapter for FakeDebugAdapter {
        fn try_receive_message(&self) -> DebugAdapterResult<MessageEnvelope> {
            self.internals
                .borrow_mut()
                .receiver_queue
                .pop_front()
                .unwrap_or(Err(TryRecvError::Empty.into()))
        }
        fn send_message(&self, message: MessageEnvelope) -> DebugAdapterResult<()> {
            Ok(self.internals.borrow_mut().sender_queue.push_back(message))
        }
    }

    #[test]
    fn initialization_sequence() {
        let (adapter, adapter_internals) = FakeDebugAdapter::new();
        push_incoming(&*adapter_internals, initialize_request());
        push_incoming(&*adapter_internals, attach_request());
        push_incoming(&*adapter_internals, set_exception_breakpoints_request());
        push_incoming(&*adapter_internals, threads_request());
        push_incoming(&*adapter_internals, stack_trace_request());
        let mut debugger = Debugger::new(adapter);

        debugger.process_meessages();

        assert_matches!(
            pop_outgoing(&*adapter_internals),
            Some(MessageEnvelope {
                message: Message::Response(ResponseEnvelope {
                    response: Response::Initialize,
                    ..
                }),
                ..
            })
        );
        assert_matches!(
            pop_outgoing(&*adapter_internals),
            Some(MessageEnvelope {
                message: Message::Event(Event::Initialized),
                ..
            })
        );
        assert_matches!(
            pop_outgoing(&*adapter_internals),
            Some(MessageEnvelope {
                message: Message::Response(ResponseEnvelope {
                    response: Response::Attach,
                    ..
                }),
                ..
            })
        );
        assert_matches!(
            pop_outgoing(&*adapter_internals),
            Some(MessageEnvelope {
                message: Message::Event(Event::Stopped(StoppedEvent {
                    thread_id: 1,
                    reason: StopReason::Entry,
                    all_threads_stopped: true,
                })),
                ..
            })
        );
        assert_matches!(
            pop_outgoing(&*adapter_internals),
            Some(MessageEnvelope {
                message: Message::Response(ResponseEnvelope {
                    response: Response::SetExceptionBreakpoints,
                    ..
                }),
                ..
            })
        );
        assert_matches!(
            pop_outgoing(&*adapter_internals),
            Some(MessageEnvelope {
                message: Message::Response(ResponseEnvelope {
                    response: Response::Threads(ThreadsResponse { threads }),
                    ..
                }),
                ..
            }) if threads == vec![Thread {
                id: 1,
                name: "main thread".into(),
            }]
        );
        assert_matches!(
            pop_outgoing(&*adapter_internals),
            Some(MessageEnvelope {
                message: Message::Response(ResponseEnvelope {
                    response: Response::StackTrace(StackTraceResponse {
                        stack_frames,
                        total_frames: 0
                    }),
                    ..
                }),
                ..
            }) if stack_frames == vec![]
        );
        assert_eq!(pop_outgoing(&*adapter_internals), None);
    }

    #[test]
    fn uses_sequence_numbers() {
        let (adapter, adapter_internals) = FakeDebugAdapter::new();
        push_incoming(&*adapter_internals, initialize_request());
        push_incoming(&*adapter_internals, attach_request());
        let mut debugger = Debugger::new(adapter);

        debugger.process_meessages();

        // TODO: The initialization sequence isn't really good to verify this.
        // Let's use some repeatable messages of the same type.
        assert_matches!(
            pop_outgoing(&*adapter_internals),
            Some(MessageEnvelope {
                seq: 1,
                message: Message::Response(ResponseEnvelope { request_seq: 5, .. }),
                ..
            })
        );
        assert_matches!(
            pop_outgoing(&*adapter_internals),
            Some(MessageEnvelope { seq: 2, .. })
        );
        assert_matches!(
            pop_outgoing(&*adapter_internals),
            Some(MessageEnvelope {
                seq: 3,
                message: Message::Response(ResponseEnvelope { request_seq: 8, .. })
            })
        );
    }
}
