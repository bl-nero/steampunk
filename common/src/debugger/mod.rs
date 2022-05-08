pub mod adapter;
pub mod dap_types;

mod core;
mod disasm;
mod protocol;

use crate::debugger::adapter::DebugAdapter;
use crate::debugger::adapter::DebugAdapterError;
use crate::debugger::adapter::DebugAdapterResult;
use crate::debugger::core::DebuggerCore;
use crate::debugger::core::StopReason;
use crate::debugger::dap_types::Breakpoint;
use crate::debugger::dap_types::Capabilities;
use crate::debugger::dap_types::DisassembleArguments;
use crate::debugger::dap_types::DisassembleResponse;
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
use crate::debugger::dap_types::SetInstructionBreakpointsArguments;
use crate::debugger::dap_types::SetInstructionBreakpointsResponse;
use crate::debugger::dap_types::StackFrame;
use crate::debugger::dap_types::StackTraceResponse;
use crate::debugger::dap_types::StoppedEvent;
use crate::debugger::dap_types::Thread;
use crate::debugger::dap_types::ThreadsResponse;
use crate::debugger::dap_types::Variable;
use crate::debugger::dap_types::VariablesResponse;
use crate::debugger::disasm::disassemble;
use crate::debugger::disasm::seek_instruction;
use std::sync::mpsc::TryRecvError;
use ya6502::cpu::MachineInspector;

/// Default margin for disassembling code. Whenever a disassembly request comes
/// in, we adjust the instruction offset by this number to make sure that we get
/// enough "runway" to lock into a stable sequence of instructions (as opposed to
/// disassembling the preceding instruction's argument as opcode). We then simply
/// discard this amount of instructions before serving the result.
const DISASSEMBLY_MARGIN: usize = 20;

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

    pub fn stopped(&self) -> bool {
        self.core.stopped()
    }

    pub fn update(&mut self, inspector: &impl MachineInspector) -> DebugAdapterResult<()> {
        self.core.update(inspector);
        if let Some(reason) = self.core.last_stop_reason() {
            self.send_event(Event::Stopped(StoppedEvent {
                thread_id: 1,
                reason,
                all_threads_stopped: true,
            }))?;
        }
        Ok(())
    }

    pub fn process_messages(&mut self, inspector: &impl MachineInspector) {
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
            Request::SetInstructionBreakpoints(args) => self.set_instruction_breakpoints(args),
            Request::Attach {} => self.attach(),
            Request::Threads => self.threads(),
            Request::StackTrace {} => self.stack_trace(inspector),
            Request::Scopes {} => self.scopes(),
            Request::Variables {} => self.variables(inspector),
            Request::Disassemble(args) => self.disassemble(inspector, args),

            Request::Continue {} => self.resume(),
            Request::Pause {} => self.pause(),
            Request::Next {} => self.next(inspector),
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
                supports_instruction_breakpoints: true,
            }),
            Some(Box::new(|me| me.send_event(Event::Initialized))),
        )
    }

    fn set_exception_breakpoints(&self) -> RequestOutcome<A> {
        (Response::SetExceptionBreakpoints, None)
    }

    fn set_instruction_breakpoints(
        &mut self,
        args: SetInstructionBreakpointsArguments,
    ) -> RequestOutcome<A> {
        let addresses_iter = args.breakpoints.iter().map(|breakpoint| {
            (i64::from_str_radix(
                breakpoint.instruction_reference.strip_prefix("0x").unwrap(),
                16,
            )
            .unwrap()
                + breakpoint.offset.unwrap_or(0)) as u16
        });
        self.core
            .set_instruction_breakpoints(addresses_iter.clone().collect());
        (
            Response::SetInstructionBreakpoints(SetInstructionBreakpointsResponse {
                breakpoints: addresses_iter
                    .map(|address| Breakpoint {
                        verified: true,
                        instruction_reference: format!("0x{:04X}", address),
                    })
                    .collect(),
            }),
            None,
        )
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
                    instruction_pointer_reference: format!("0x{:04X}", inspector.reg_pc()),
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

    fn disassemble(
        &self,
        inspector: &impl MachineInspector,
        args: DisassembleArguments,
    ) -> RequestOutcome<A> {
        // TODO: So far, we just return dummy data.
        let mem_reference =
            i64::from_str_radix(&args.memory_reference.strip_prefix("0x").unwrap(), 16).unwrap();
        let origin = (mem_reference + args.offset.unwrap_or(0)) as u16;
        let disassembly_start = seek_instruction(
            inspector,
            origin,
            args.instruction_offset.unwrap_or(0) - DISASSEMBLY_MARGIN as i64,
        );
        let instructions = disassemble(
            inspector,
            origin,
            disassembly_start,
            DISASSEMBLY_MARGIN,
            usize::try_from(args.instruction_count).unwrap(),
        );
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
        self.core.step_into();
        (Response::StepIn {}, None)
    }

    fn next(&mut self, inspector: &impl MachineInspector) -> RequestOutcome<A> {
        self.core.step_over(inspector);
        (Response::Next {}, None)
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
    use crate::debugger::dap_types::Breakpoint;
    use crate::debugger::dap_types::DisassembledInstruction;
    use crate::debugger::dap_types::InitializeArguments;
    use crate::debugger::dap_types::InstructionBreakpoint;
    use crate::debugger::dap_types::MessageEnvelope;
    use crate::debugger::dap_types::SetInstructionBreakpointsArguments;
    use std::assert_matches::assert_matches;
    use ya6502::cpu::Cpu;
    use ya6502::cpu::MockMachineInspector;
    use ya6502::cpu_with_code;
    use ya6502::memory::Ram;

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

    fn tick_while_running<A: DebugAdapter>(debugger: &mut Debugger<A>, cpu: &mut Cpu<Ram>) {
        // Limit to 1000 ticks; we won't expect tests to run for that long, and
        // this way we avoid infinite loops.
        for _ in 0..1000 {
            if debugger.stopped() {
                return;
            }
            cpu.tick().unwrap();
            debugger.update(cpu).unwrap();
        }
        panic!("CPU still running at PC={:04X}", cpu.reg_pc());
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

        debugger.process_messages(&inspector);

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
        adapter.push_request(Request::SetInstructionBreakpoints(
            SetInstructionBreakpointsArguments {
                breakpoints: vec![],
            },
        ));
        adapter.push_request(Request::Threads {});
        let mut debugger = Debugger::new(adapter.clone());

        debugger.process_messages(&inspector);

        assert_responded_with(
            &adapter,
            Response::Initialize(Capabilities {
                supports_disassemble_request: true,
                supports_instruction_breakpoints: true,
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
            Response::SetInstructionBreakpoints(SetInstructionBreakpointsResponse {
                breakpoints: vec![],
            }),
        );
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

        debugger.process_messages(&inspector);

        assert_responded_with(
            &adapter,
            Response::StackTrace(StackTraceResponse {
                stack_frames: vec![StackFrame {
                    id: 1,
                    name: "".to_string(),
                    line: 0,
                    column: 0,
                    instruction_pointer_reference: "0x1234".to_string(),
                }],
                total_frames: 1,
            }),
        );
        assert_eq!(adapter.pop_outgoing(), None);

        inspector.expect_reg_pc().once().return_const(0x0A04u16);
        adapter.push_request(Request::StackTrace {});
        debugger.process_messages(&inspector);

        assert_responded_with(
            &adapter,
            Response::StackTrace(StackTraceResponse {
                stack_frames: vec![StackFrame {
                    id: 1,
                    name: "".to_string(),
                    line: 0,
                    column: 0,
                    instruction_pointer_reference: "0x0A04".to_string(),
                }],
                total_frames: 1,
            }),
        );
        assert_eq!(adapter.pop_outgoing(), None);
    }

    #[test]
    fn disassembly() {
        let cpu = cpu_with_code! {
                lda 0x45
                sta 0xEA
        };
        let adapter = FakeDebugAdapter::default();
        let mut debugger = Debugger::new(adapter.clone());

        adapter.push_request(Request::Disassemble(DisassembleArguments {
            memory_reference: "0xF000".to_string(),
            offset: Some(0),
            instruction_offset: Some(0),
            instruction_count: 2,
        }));
        adapter.push_request(Request::Disassemble(DisassembleArguments {
            memory_reference: "0xF002".to_string(),
            offset: None,
            instruction_offset: None,
            instruction_count: 1,
        }));
        debugger.process_messages(&cpu);

        assert_responded_with(
            &adapter,
            Response::Disassemble(DisassembleResponse {
                instructions: vec![
                    DisassembledInstruction {
                        address: "0xF000".to_string(),
                        instruction_bytes: "A5 45".to_string(),
                        instruction: "LDA $45".to_string(),
                    },
                    DisassembledInstruction {
                        address: "0xF002".to_string(),
                        instruction_bytes: "85 EA".to_string(),
                        instruction: "STA $EA".to_string(),
                    },
                ],
            }),
        );
        assert_responded_with(
            &adapter,
            Response::Disassemble(DisassembleResponse {
                instructions: vec![DisassembledInstruction {
                    address: "0xF002".to_string(),
                    instruction_bytes: "85 EA".to_string(),
                    instruction: "STA $EA".to_string(),
                }],
            }),
        );
        assert_eq!(adapter.pop_outgoing(), None);
    }

    #[test]
    fn disassembly_ambiguous() {
        let cpu = cpu_with_code! {
                lda 0x45
                sta 0xEA
                sta 0xAE
        };
        let adapter = FakeDebugAdapter::default();
        let mut debugger = Debugger::new(adapter.clone());

        adapter.push_request(Request::Disassemble(DisassembleArguments {
            memory_reference: "0xF002".to_string(),
            offset: Some(1),
            instruction_offset: Some(-2),
            instruction_count: 4,
        }));
        adapter.push_request(Request::Disassemble(DisassembleArguments {
            memory_reference: "0xF004".to_string(),
            offset: Some(0),
            instruction_offset: Some(-1),
            instruction_count: 2,
        }));
        debugger.process_messages(&cpu);

        assert_responded_with(
            &adapter,
            Response::Disassemble(DisassembleResponse {
                instructions: vec![
                    DisassembledInstruction {
                        address: "0xF000".to_string(),
                        instruction_bytes: "A5 45".to_string(),
                        instruction: "LDA $45".to_string(),
                    },
                    DisassembledInstruction {
                        address: "0xF002".to_string(),
                        instruction_bytes: "85".to_string(),
                        instruction: "".to_string(),
                    },
                    DisassembledInstruction {
                        address: "0xF003".to_string(),
                        instruction_bytes: "EA".to_string(),
                        instruction: "NOP".to_string(),
                    },
                    DisassembledInstruction {
                        address: "0xF004".to_string(),
                        instruction_bytes: "85 AE".to_string(),
                        instruction: "STA $AE".to_string(),
                    },
                ],
            }),
        );
        assert_responded_with(
            &adapter,
            Response::Disassemble(DisassembleResponse {
                instructions: vec![
                    DisassembledInstruction {
                        address: "0xF002".to_string(),
                        instruction_bytes: "85 EA".to_string(),
                        instruction: "STA $EA".to_string(),
                    },
                    DisassembledInstruction {
                        address: "0xF004".to_string(),
                        instruction_bytes: "85 AE".to_string(),
                        instruction: "STA $AE".to_string(),
                    },
                ],
            }),
        );
        assert_eq!(adapter.pop_outgoing(), None);
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
        debugger.process_messages(&inspector);

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
        assert!(debugger.stopped());

        debugger.process_messages(&inspector);

        assert_responded_with(&adapter, Response::Continue {});
        assert!(!debugger.stopped());

        adapter.push_request(Request::Pause {});
        debugger.process_messages(&inspector);

        assert_responded_with(&adapter, Response::Pause {});
        assert_emitted(
            &adapter,
            Event::Stopped(StoppedEvent {
                thread_id: 1,
                reason: StopReason::Pause,
                all_threads_stopped: true,
            }),
        );
        assert!(debugger.stopped());
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

        debugger.process_messages(&cpu);

        assert_responded_with(&adapter, Response::StepIn {});
        assert!(!debugger.stopped());

        cpu.tick().unwrap();
        debugger.update(&cpu).unwrap();
        cpu.tick().unwrap();
        assert_eq!(adapter.pop_outgoing(), None);

        debugger.update(&cpu).unwrap();
        assert!(debugger.stopped());
        assert_emitted(
            &adapter,
            Event::Stopped(StoppedEvent {
                thread_id: 1,
                reason: StopReason::Step,
                all_threads_stopped: true,
            }),
        )
    }

    #[test]
    fn next() {
        let mut cpu = cpu_with_code! {
                jsr subroutine
                nop
            subroutine:
                rts
        };

        let adapter = FakeDebugAdapter::default();
        adapter.push_request(Request::Next {});
        let mut debugger = Debugger::new(adapter.clone());

        debugger.process_messages(&cpu);
        assert_responded_with(&adapter, Response::Next {});

        tick_while_running(&mut debugger, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF003);
        assert_emitted(
            &adapter,
            Event::Stopped(StoppedEvent {
                thread_id: 1,
                reason: StopReason::Step,
                all_threads_stopped: true,
            }),
        );
        assert_eq!(adapter.pop_outgoing(), None);
    }

    #[test]
    fn instruction_breakpoints() {
        let mut cpu = cpu_with_code! {
                nop
                nop
                nop
                nop
            loop:
                jmp loop
        };
        let adapter = FakeDebugAdapter::default();
        let mut debugger = Debugger::new(adapter.clone());

        adapter.push_request(Request::SetInstructionBreakpoints(
            SetInstructionBreakpointsArguments {
                breakpoints: vec![
                    InstructionBreakpoint {
                        instruction_reference: "0xF001".to_string(),
                        offset: None,
                    },
                    InstructionBreakpoint {
                        instruction_reference: "0xEFFF".to_string(),
                        offset: Some(4), // Effective address: 0xF003
                    },
                ],
            },
        ));
        adapter.push_request(Request::Continue {});
        debugger.process_messages(&mut cpu);
        assert_responded_with(
            &adapter,
            Response::SetInstructionBreakpoints(SetInstructionBreakpointsResponse {
                breakpoints: vec![
                    Breakpoint {
                        verified: true,
                        instruction_reference: "0xF001".to_string(),
                    },
                    Breakpoint {
                        verified: true,
                        instruction_reference: "0xF003".to_string(),
                    },
                ],
            }),
        );
        assert_responded_with(&adapter, Response::Continue {});

        tick_while_running(&mut debugger, &mut cpu);
        assert_emitted(
            &adapter,
            Event::Stopped(StoppedEvent {
                thread_id: 1,
                reason: StopReason::Breakpoint,
                all_threads_stopped: true,
            }),
        );
        assert_eq!(cpu.reg_pc(), 0xF001);

        adapter.push_request(Request::Continue {});
        debugger.process_messages(&mut cpu);
        assert_responded_with(&adapter, Response::Continue {});

        tick_while_running(&mut debugger, &mut cpu);
        assert_emitted(
            &adapter,
            Event::Stopped(StoppedEvent {
                thread_id: 1,
                reason: StopReason::Breakpoint,
                all_threads_stopped: true,
            }),
        );
        assert_eq!(cpu.reg_pc(), 0xF003);
    }

    #[test]
    fn disconnects() {
        let inspector = MockMachineInspector::new();
        let adapter = FakeDebugAdapter::default();
        adapter.push_request(Request::Disconnect(None));
        adapter.expect_disconnect();
        let mut debugger = Debugger::new(adapter.clone());
        debugger.process_messages(&inspector);

        assert_responded_with(&adapter, Response::Disconnect);
        assert!(adapter.disconnected());
        assert!(!debugger.stopped());
    }
}
