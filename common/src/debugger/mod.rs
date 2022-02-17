pub mod adapter;
pub mod dap_types;
mod protocol;

use crate::app::MachineInspector;
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
use crate::debugger::dap_types::Scope;
use crate::debugger::dap_types::ScopePresentationHint;
use crate::debugger::dap_types::ScopesResponse;
use crate::debugger::dap_types::StackFrame;
use crate::debugger::dap_types::StackTraceResponse;
use crate::debugger::dap_types::StopReason;
use crate::debugger::dap_types::StoppedEvent;
use crate::debugger::dap_types::Thread;
use crate::debugger::dap_types::ThreadsResponse;
use crate::debugger::dap_types::Variable;
use crate::debugger::dap_types::VariablesResponse;
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

    pub fn process_meessages(&mut self, inspector: &impl MachineInspector) {
        loop {
            match self.adapter.try_receive_message() {
                Ok(envelope) => self.process_message(envelope, inspector),
                Err(DebugAdapterError::TryRecvError(TryRecvError::Empty)) => return,
                Err(e) => panic!("{}", e),
            }
        }
    }

    fn process_message(&mut self, envelope: MessageEnvelope, inspector: &impl MachineInspector) {
        match envelope.message {
            Message::Request(request) => self.process_request(envelope.seq, request, inspector),
            other => eprintln!("Unsupported message: {:?}", other),
        };
    }

    fn process_request(
        &mut self,
        request_seq: i64,
        request: Request,
        inspector: &impl MachineInspector,
    ) {
        let (response, event) = match request {
            Request::Initialize(args) => self.initialize(args),
            Request::SetExceptionBreakpoints {} => self.set_exception_breakpoints(),
            Request::Attach {} => self.attach(),
            Request::Threads => self.threads(),
            Request::StackTrace {} => self.stack_trace(),
            Request::Scopes {} => self.scopes(),
            Request::Variables {} => self.variables(inspector),
            Request::Disconnect(_) => self.disconnect(),
        };
        self.send_message(Message::Response(ResponseEnvelope {
            request_seq,
            success: true,
            response,
        }))
        .unwrap();
        if let Some(event) = event {
            self.send_message(Message::Event(event)).unwrap();
        }
    }

    fn initialize(&self, args: InitializeArguments) -> (Response, Option<Event>) {
        eprintln!(
            "Initializing debugger session with {}",
            args.client_name.as_deref().unwrap_or("an unnamed client")
        );
        (Response::Initialize, Some(Event::Initialized))
    }

    fn set_exception_breakpoints(&self) -> (Response, Option<Event>) {
        (Response::SetExceptionBreakpoints, None)
    }

    fn attach(&self) -> (Response, Option<Event>) {
        (
            Response::Attach,
            Some(Event::Stopped(StoppedEvent {
                reason: StopReason::Entry,
                thread_id: 1,
                all_threads_stopped: true,
            })),
        )
    }

    fn threads(&self) -> (Response, Option<Event>) {
        (
            Response::Threads(ThreadsResponse {
                threads: vec![Thread {
                    id: 1,
                    name: "main thread".to_string(),
                }],
            }),
            None,
        )
    }

    fn stack_trace(&self) -> (Response, Option<Event>) {
        (
            Response::StackTrace(StackTraceResponse {
                stack_frames: vec![StackFrame {
                    id: 1,
                    name: "".to_string(),
                    line: 0,
                    column: 0,
                }],
                total_frames: 1,
            }),
            None,
        )
    }

    fn scopes(&self) -> (Response, Option<Event>) {
        (
            Response::Scopes(ScopesResponse {
                scopes: vec![Scope {
                    name: "Registers".to_string(),
                    presentation_hint: ScopePresentationHint::Registers,
                    variables_reference: 1,
                    expensive: false,
                }],
            }),
            None,
        )
    }

    fn variables(&self, inspector: &impl MachineInspector) -> (Response, Option<Event>) {
        (
            Response::Variables(VariablesResponse {
                variables: vec![
                    byte_variable("A", inspector.reg_a()),
                    byte_variable("X", inspector.reg_x()),
                    byte_variable("Y", inspector.reg_y()),
                    byte_variable("SP", inspector.reg_sp()),
                    Variable {
                        name: "PC".to_string(),
                        value: format_word(inspector.reg_pc()),
                        variables_reference: 0,
                    },
                    byte_variable("FLAGS", inspector.cpu_flags()),
                ],
            }),
            None,
        )
    }

    fn disconnect(&self) -> (Response, Option<Event>) {
        todo!();
    }

    fn send_message(&mut self, message: Message) -> DebugAdapterResult<()> {
        let seq = self.next_sequence_number();
        return self.adapter.send_message(MessageEnvelope { seq, message });
    }

    fn next_sequence_number(&mut self) -> i64 {
        self.sequence_number += 1;
        return self.sequence_number;
    }
}

fn format_byte(val: u8) -> String {
    format!("${:02X}", val)
}

fn format_word(val: u16) -> String {
    format!("${:04X}", val)
}

fn byte_variable(name: &str, value: u8) -> Variable {
    Variable {
        name: name.to_string(),
        value: format_byte(value),
        variables_reference: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::MockMachineInspector;
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

    fn scopes_request() -> DebugAdapterResult<MessageEnvelope> {
        Ok(MessageEnvelope {
            seq: 45,
            message: Message::Request(Request::Scopes {}),
        })
    }

    fn variables_request() -> DebugAdapterResult<MessageEnvelope> {
        Ok(MessageEnvelope {
            seq: 46,
            message: Message::Request(Request::Variables {}),
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

    fn assert_responded_with(
        adapter_internals: &RefCell<FakeDebugAdapterInternals>,
        expected_response: Response,
    ) {
        assert_matches!(
            pop_outgoing(adapter_internals),
            Some(MessageEnvelope {
                message: Message::Response(ResponseEnvelope {
                    response,
                    ..
                }),
                ..
            }) if response == expected_response,
            "Expected response: {:?}",
            expected_response,
        );
    }

    #[test]
    fn initialization_sequence() {
        let inspector = MockMachineInspector::new();
        let (adapter, adapter_internals) = FakeDebugAdapter::new();
        push_incoming(&*adapter_internals, initialize_request());
        push_incoming(&*adapter_internals, attach_request());
        push_incoming(&*adapter_internals, set_exception_breakpoints_request());
        push_incoming(&*adapter_internals, threads_request());
        push_incoming(&*adapter_internals, stack_trace_request());
        let mut debugger = Debugger::new(adapter);

        debugger.process_meessages(&inspector);

        assert_responded_with(&*adapter_internals, Response::Initialize);
        assert_matches!(
            pop_outgoing(&*adapter_internals),
            Some(MessageEnvelope {
                message: Message::Event(Event::Initialized),
                ..
            })
        );
        assert_responded_with(&*adapter_internals, Response::Attach);
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
        assert_responded_with(&*adapter_internals, Response::SetExceptionBreakpoints);
        assert_responded_with(
            &*adapter_internals,
            Response::Threads(ThreadsResponse {
                threads: vec![Thread {
                    id: 1,
                    name: "main thread".into(),
                }],
            }),
        );
        assert_responded_with(
            &*adapter_internals,
            Response::StackTrace(StackTraceResponse {
                stack_frames: vec![StackFrame {
                    id: 1,
                    name: "".to_string(),
                    line: 0,
                    column: 0,
                }],
                total_frames: 1,
            }),
        );
        assert_eq!(pop_outgoing(&*adapter_internals), None);
    }

    #[test]
    fn uses_sequence_numbers() {
        let inspector = MockMachineInspector::new();
        let (adapter, adapter_internals) = FakeDebugAdapter::new();
        push_incoming(&*adapter_internals, initialize_request());
        push_incoming(&*adapter_internals, attach_request());
        let mut debugger = Debugger::new(adapter);

        debugger.process_meessages(&inspector);

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

    #[test]
    fn sends_registers() {
        let mut inspector = MockMachineInspector::new();
        let (adapter, adapter_internals) = FakeDebugAdapter::new();
        push_incoming(&*adapter_internals, scopes_request());
        push_incoming(&*adapter_internals, variables_request());
        let mut debugger = Debugger::new(adapter);

        inspector.expect_reg_a().return_const(0x04);
        inspector.expect_reg_x().return_const(0x13);
        inspector.expect_reg_y().return_const(0x22);
        inspector.expect_reg_sp().return_const(0x31);
        inspector.expect_reg_pc().return_const(0x0ABCu16);
        inspector.expect_cpu_flags().return_const(0x40);
        debugger.process_meessages(&inspector);

        assert_responded_with(
            &*adapter_internals,
            Response::Scopes(ScopesResponse {
                scopes: vec![Scope {
                    name: "Registers".to_string(),
                    presentation_hint: ScopePresentationHint::Registers,
                    variables_reference: 1,
                    expensive: false,
                }],
            }),
        );
        assert_responded_with(
            &*adapter_internals,
            Response::Variables(VariablesResponse {
                variables: vec![
                    Variable {
                        name: "A".to_string(),
                        value: "$04".to_string(),
                        variables_reference: 0,
                    },
                    Variable {
                        name: "X".to_string(),
                        value: "$13".to_string(),
                        variables_reference: 0,
                    },
                    Variable {
                        name: "Y".to_string(),
                        value: "$22".to_string(),
                        variables_reference: 0,
                    },
                    Variable {
                        name: "SP".to_string(),
                        value: "$31".to_string(),
                        variables_reference: 0,
                    },
                    Variable {
                        name: "PC".to_string(),
                        value: "$0ABC".to_string(),
                        variables_reference: 0,
                    },
                    Variable {
                        name: "FLAGS".to_string(),
                        value: "$40".to_string(),
                        variables_reference: 0,
                    },
                ],
            }),
        );
    }
}
