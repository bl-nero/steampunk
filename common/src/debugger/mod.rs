pub mod adapter;
pub mod dap_types;

mod core;
mod protocol;

use crate::debugger::adapter::DebugAdapter;
use crate::debugger::adapter::DebugAdapterError;
use crate::debugger::adapter::DebugAdapterResult;
use crate::debugger::core::DebuggerCore;
use crate::debugger::dap_types::Capabilities;
use crate::debugger::dap_types::DisassembleArguments;
use crate::debugger::dap_types::DisassembleResponse;
use crate::debugger::dap_types::DisassembledInstruction;
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
use ya6502::cpu::MachineInspector;

/// A debugger for 6502-based machines. Uses Debug Adapter Protocol internally
/// to communicate with a debugger UI.
pub struct Debugger<A: DebugAdapter> {
    adapter: A,
    sequence_number: i64,
    core: DebuggerCore,
}

type RequestOutcome<A> = (
    Response,
    Option<Box<dyn FnOnce(&mut Debugger<A>) -> DebugAdapterResult<()>>>,
);

impl<A: DebugAdapter> Debugger<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            sequence_number: 0,
            core: DebuggerCore::new(),
        }
    }

    pub fn paused(&self) -> bool {
        self.core.paused()
    }

    pub fn update(&mut self, inspector: &impl MachineInspector) -> DebugAdapterResult<()> {
        self.core.update(inspector);
        if self.core.has_just_paused() {
            self.send_event(Event::Stopped(StoppedEvent {
                thread_id: 1,
                reason: StopReason::Step,
                all_threads_stopped: true,
            }))?;
        }
        Ok(())
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
        let (response, continuation) = match request {
            Request::Initialize(args) => self.initialize(args),
            Request::SetExceptionBreakpoints {} => self.set_exception_breakpoints(),
            Request::Attach {} => self.attach(),
            Request::Threads => self.threads(),
            Request::StackTrace {} => self.stack_trace(inspector),
            Request::Scopes {} => self.scopes(),
            Request::Variables {} => self.variables(inspector),
            Request::Disassemble(args) => self.disassemble(args),

            Request::Continue {} => self.resume(),
            Request::Pause {} => self.pause(),
            Request::Next {} => todo!(),
            Request::StepIn {} => self.step_in(),
            Request::StepOut {} => todo!(),

            Request::Disconnect(_) => self.disconnect(),
        };
        self.send_message(Message::Response(ResponseEnvelope {
            request_seq,
            success: true,
            response,
        }))
        .unwrap();
        if let Some(continuation) = continuation {
            continuation(self).unwrap();
        }
    }

    fn send_event(&mut self, event: Event) -> DebugAdapterResult<()> {
        self.send_message(Message::Event(event))
    }

    fn initialize(&self, args: InitializeArguments) -> RequestOutcome<A> {
        eprintln!(
            "Initializing debugger session with {}",
            args.client_name.as_deref().unwrap_or("an unnamed client")
        );
        (
            Response::Initialize(Capabilities {
                supports_disassemble_request: true,
            }),
            Some(Box::new(|me| me.send_event(Event::Initialized))),
        )
    }

    fn set_exception_breakpoints(&self) -> RequestOutcome<A> {
        (Response::SetExceptionBreakpoints, None)
    }

    fn attach(&self) -> RequestOutcome<A> {
        (
            Response::Attach,
            Some(Box::new(|me| {
                me.send_event(Event::Stopped(StoppedEvent {
                    reason: StopReason::Entry,
                    thread_id: 1,
                    all_threads_stopped: true,
                }))
            })),
        )
    }

    fn threads(&self) -> RequestOutcome<A> {
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

    fn stack_trace(&self, inspector: &impl MachineInspector) -> RequestOutcome<A> {
        (
            Response::StackTrace(StackTraceResponse {
                stack_frames: vec![StackFrame {
                    id: 1,
                    name: "".to_string(),
                    line: 0,
                    column: 0,
                    instruction_pointer_reference: format!("{:04X}", inspector.reg_pc()),
                }],
                total_frames: 1,
            }),
            None,
        )
    }

    fn scopes(&self) -> RequestOutcome<A> {
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

    fn variables(&self, inspector: &impl MachineInspector) -> RequestOutcome<A> {
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
                    byte_variable("FLAGS", inspector.flags()),
                ],
            }),
            None,
        )
    }

    fn disassemble(&self, args: DisassembleArguments) -> RequestOutcome<A> {
        // TODO: So far, we just return dummy data.
        let mem_reference = i64::from_str_radix(&args.memory_reference, 16).unwrap();
        let start_address = mem_reference + args.offset.unwrap() + args.instruction_offset.unwrap();
        let instructions: Vec<_> = (0..args.instruction_count)
            .map(|i| DisassembledInstruction {
                address: format!("0x{:04X}", start_address + i),
                instruction_bytes: "EA".to_string(),
                instruction: "NOP".to_string(),
            })
            .collect();
        (
            Response::Disassemble(DisassembleResponse { instructions }),
            None,
        )
    }

    fn resume(&mut self) -> RequestOutcome<A> {
        self.core.resume();
        (Response::Continue {}, None)
    }

    fn pause(&mut self) -> RequestOutcome<A> {
        self.core.pause();
        (
            Response::Pause {},
            Some(Box::new(|me| {
                me.send_event(Event::Stopped(StoppedEvent {
                    reason: StopReason::Pause,
                    thread_id: 1,
                    all_threads_stopped: true,
                }))
            })),
        )
    }

    fn step_in(&mut self) -> RequestOutcome<A> {
        self.core.step_in();
        (Response::StepIn {}, None)
    }

    fn disconnect(&mut self) -> RequestOutcome<A> {
        self.core.resume();
        (
            Response::Disconnect,
            Some(Box::new(|me| me.adapter.disconnect())),
        )
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
    use crate::debugger::adapter::FakeDebugAdapter;
    use crate::debugger::dap_types::InitializeArguments;
    use crate::debugger::dap_types::MessageEnvelope;
    use std::assert_matches::assert_matches;
    use ya6502::cpu::MockMachineInspector;
    use ya6502::cpu_with_code;

    fn assert_responded_with(adapter: &FakeDebugAdapter, expected_response: Response) {
        assert_matches!(
            adapter.pop_outgoing(),
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

    fn assert_emitted(adapter: &FakeDebugAdapter, expected_event: Event) {
        assert_matches!(
            adapter.pop_outgoing(),
            Some(MessageEnvelope {
                message: Message::Event(event),
                ..
            }) if event == expected_event,
            "Expected event: {:?}",
            expected_event,
        );
    }

    #[test]
    fn uses_sequence_numbers() {
        let inspector = MockMachineInspector::new();
        let adapter = FakeDebugAdapter::default();
        adapter.push_incoming(Ok(MessageEnvelope {
            seq: 5,
            message: Message::Request(Request::Initialize(InitializeArguments {
                client_name: Some("Visual Studio Code".into()),
            })),
        }));
        adapter.push_incoming(Ok(MessageEnvelope {
            seq: 8,
            message: Message::Request(Request::Threads {}),
        }));
        adapter.push_incoming(Ok(MessageEnvelope {
            seq: 9,
            message: Message::Request(Request::Threads {}),
        }));
        let mut debugger = Debugger::new(adapter.clone());

        debugger.process_meessages(&inspector);

        assert_matches!(
            adapter.pop_outgoing(),
            Some(MessageEnvelope {
                seq: 1,
                message: Message::Response(ResponseEnvelope { request_seq: 5, .. }),
                ..
            })
        );
        assert_matches!(adapter.pop_outgoing(), Some(MessageEnvelope { seq: 2, .. }));
        assert_matches!(
            adapter.pop_outgoing(),
            Some(MessageEnvelope {
                seq: 3,
                message: Message::Response(ResponseEnvelope { request_seq: 8, .. })
            })
        );
        assert_matches!(
            adapter.pop_outgoing(),
            Some(MessageEnvelope {
                seq: 4,
                message: Message::Response(ResponseEnvelope { request_seq: 9, .. })
            })
        );
        assert_eq!(adapter.pop_outgoing(), None);
    }

    #[test]
    fn initialization_sequence() {
        let inspector = MockMachineInspector::new();
        let adapter = FakeDebugAdapter::default();
        adapter.push_request(Request::Initialize(InitializeArguments {
            client_name: Some("Visual Studio Code".into()),
        }));
        adapter.push_request(Request::Attach {});
        adapter.push_request(Request::SetExceptionBreakpoints {});
        adapter.push_request(Request::Threads {});
        let mut debugger = Debugger::new(adapter.clone());

        debugger.process_meessages(&inspector);

        assert_responded_with(
            &adapter,
            Response::Initialize(Capabilities {
                supports_disassemble_request: true,
            }),
        );
        assert_emitted(&adapter, Event::Initialized);
        assert_responded_with(&adapter, Response::Attach);
        assert_emitted(
            &adapter,
            Event::Stopped(StoppedEvent {
                thread_id: 1,
                reason: StopReason::Entry,
                all_threads_stopped: true,
            }),
        );
        assert_responded_with(&adapter, Response::SetExceptionBreakpoints);
        assert_responded_with(
            &adapter,
            Response::Threads(ThreadsResponse {
                threads: vec![Thread {
                    id: 1,
                    name: "main thread".into(),
                }],
            }),
        );
        assert_eq!(adapter.pop_outgoing(), None);
    }

    #[test]
    fn stack_trace() {
        let mut inspector = MockMachineInspector::new();
        inspector.expect_reg_pc().once().return_const(0x1234u16);
        let adapter = FakeDebugAdapter::default();
        adapter.push_request(Request::StackTrace {});
        let mut debugger = Debugger::new(adapter.clone());

        debugger.process_meessages(&inspector);

        assert_responded_with(
            &adapter,
            Response::StackTrace(StackTraceResponse {
                stack_frames: vec![StackFrame {
                    id: 1,
                    name: "".to_string(),
                    line: 0,
                    column: 0,
                    instruction_pointer_reference: "1234".to_string(),
                }],
                total_frames: 1,
            }),
        );
        assert_eq!(adapter.pop_outgoing(), None);

        inspector.expect_reg_pc().once().return_const(0x0A04u16);
        adapter.push_request(Request::StackTrace {});
        debugger.process_meessages(&inspector);

        assert_responded_with(
            &adapter,
            Response::StackTrace(StackTraceResponse {
                stack_frames: vec![StackFrame {
                    id: 1,
                    name: "".to_string(),
                    line: 0,
                    column: 0,
                    instruction_pointer_reference: "0A04".to_string(),
                }],
                total_frames: 1,
            }),
        );
        assert_eq!(adapter.pop_outgoing(), None);
    }

    #[test]
    fn disconnects() {
        let inspector = MockMachineInspector::new();
        let adapter = FakeDebugAdapter::default();
        adapter.push_request(Request::Disconnect(None));
        adapter.expect_disconnect();
        let mut debugger = Debugger::new(adapter.clone());
        debugger.process_meessages(&inspector);

        assert_responded_with(&adapter, Response::Disconnect);
        assert!(adapter.disconnected());
        assert!(!debugger.paused());
    }

    #[test]
    fn sends_registers() {
        let mut inspector = MockMachineInspector::new();
        let adapter = FakeDebugAdapter::default();
        adapter.push_request(Request::Scopes {});
        adapter.push_request(Request::Variables {});
        let mut debugger = Debugger::new(adapter.clone());

        inspector.expect_reg_a().return_const(0x04);
        inspector.expect_reg_x().return_const(0x13);
        inspector.expect_reg_y().return_const(0x22);
        inspector.expect_reg_sp().return_const(0x31);
        inspector.expect_reg_pc().return_const(0x0ABCu16);
        inspector.expect_flags().return_const(0x40);
        debugger.process_meessages(&inspector);

        assert_responded_with(
            &adapter,
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
            &adapter,
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
        assert_eq!(adapter.pop_outgoing(), None);
    }

    #[test]
    fn continue_and_pause() {
        let inspector = MockMachineInspector::new();
        let adapter = FakeDebugAdapter::default();
        adapter.push_request(Request::Continue {});
        let mut debugger = Debugger::new(adapter.clone());
        assert!(debugger.paused());

        debugger.process_meessages(&inspector);

        assert_responded_with(&adapter, Response::Continue {});
        assert!(!debugger.paused());

        adapter.push_request(Request::Pause {});
        debugger.process_meessages(&inspector);

        assert_responded_with(&adapter, Response::Pause {});
        assert_emitted(
            &adapter,
            Event::Stopped(StoppedEvent {
                thread_id: 1,
                reason: StopReason::Pause,
                all_threads_stopped: true,
            }),
        );
        assert!(debugger.paused());
        assert_eq!(adapter.pop_outgoing(), None);
    }

    #[test]
    fn step_in() {
        let mut cpu = cpu_with_code! {
                nop
        };

        let adapter = FakeDebugAdapter::default();
        adapter.push_request(Request::StepIn {});
        let mut debugger = Debugger::new(adapter.clone());

        debugger.process_meessages(&cpu);

        assert_responded_with(&adapter, Response::StepIn {});
        assert!(!debugger.paused());

        cpu.tick().unwrap();
        debugger.update(&cpu).unwrap();
        cpu.tick().unwrap();
        assert_eq!(adapter.pop_outgoing(), None);

        debugger.update(&cpu).unwrap();
        assert!(debugger.paused());
        assert_emitted(
            &adapter,
            Event::Stopped(StoppedEvent {
                thread_id: 1,
                reason: StopReason::Step,
                all_threads_stopped: true,
            }),
        )
    }
}
