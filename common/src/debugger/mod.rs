pub mod adapter;
pub mod dap_types;

mod core;
mod disasm;
mod protocol;
mod tests;

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
use crate::debugger::dap_types::ReadMemoryArguments;
use crate::debugger::dap_types::ReadMemoryResponse;
use crate::debugger::dap_types::Request;
use crate::debugger::dap_types::Response;
use crate::debugger::dap_types::ResponseEnvelope;
use crate::debugger::dap_types::Scope;
use crate::debugger::dap_types::ScopePresentationHint;
use crate::debugger::dap_types::ScopesArguments;
use crate::debugger::dap_types::ScopesResponse;
use crate::debugger::dap_types::SetInstructionBreakpointsArguments;
use crate::debugger::dap_types::SetInstructionBreakpointsResponse;
use crate::debugger::dap_types::StackFrame;
use crate::debugger::dap_types::StackTraceResponse;
use crate::debugger::dap_types::StoppedEvent;
use crate::debugger::dap_types::Thread;
use crate::debugger::dap_types::ThreadsResponse;
use crate::debugger::dap_types::Variable;
use crate::debugger::dap_types::VariablesArguments;
use crate::debugger::dap_types::VariablesResponse;
use crate::debugger::disasm::disassemble;
use crate::debugger::disasm::seek_instruction;
use std::cmp::min;
use std::sync::mpsc::TryRecvError;
use ya6502::cpu::MachineInspector;

/// Default margin for disassembling code. Whenever a disassembly request comes
/// in, we adjust the instruction offset by this number to make sure that we get
/// enough "runway" to lock into a stable sequence of instructions (as opposed to
/// disassembling the preceding instruction's argument as opcode). We then simply
/// discard this amount of instructions before serving the result.
const DISASSEMBLY_MARGIN: usize = 20;

const REGISTERS_VARIABLES_REFERENCE: i64 = 1;
const MEMORY_VARIABLES_REFERENCE: i64 = 2;

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
            Request::Scopes(args) => self.scopes(args),
            Request::Variables(args) => self.variables(inspector, args),
            Request::Disassemble(args) => self.disassemble(inspector, args),
            Request::ReadMemory(args) => self.read_memory(inspector, args),

            Request::Continue {} => self.resume(),
            Request::Pause {} => self.pause(),
            Request::Next {} => self.next(inspector),
            Request::StepIn {} => self.step_in(),
            Request::StepOut {} => self.step_out(),

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
                supports_read_memory_request: true,
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
        let stack_trace = self.core.stack_trace(inspector);
        let num_frames = stack_trace.len();
        let stack_frames = stack_trace
            .iter()
            .enumerate()
            .map(|(i, frame)| StackFrame {
                id: (num_frames - i) as i64,
                name: format_word(frame.entry),
                instruction_pointer_reference: format!("0x{:04X}", frame.pc),
                line: 0,
                column: 0,
            })
            .collect();
        (
            Response::StackTrace(StackTraceResponse {
                stack_frames,
                total_frames: num_frames as i64,
            }),
            None,
        )
    }

    fn scopes(&self, args: ScopesArguments) -> RequestOutcome<A> {
        let mut scopes = if args.frame_id == self.core.stack_depth() as i64 {
            vec![Scope {
                name: "Registers".to_string(),
                presentation_hint: Some(ScopePresentationHint::Registers),
                variables_reference: REGISTERS_VARIABLES_REFERENCE,
                expensive: false,
            }]
        } else {
            vec![]
        };
        scopes.push(Scope {
            name: "Memory".to_string(),
            presentation_hint: None,
            variables_reference: MEMORY_VARIABLES_REFERENCE,
            expensive: false,
        });
        return (Response::Scopes(ScopesResponse { scopes }), None);
    }

    fn variables(
        &self,
        inspector: &impl MachineInspector,
        args: VariablesArguments,
    ) -> RequestOutcome<A> {
        let vars = match args.variables_reference {
            REGISTERS_VARIABLES_REFERENCE => vec![
                byte_variable("A", inspector.reg_a()),
                byte_variable("X", inspector.reg_x()),
                byte_variable("Y", inspector.reg_y()),
                byte_variable("SP", inspector.reg_sp()),
                Variable {
                    name: "PC".to_string(),
                    value: format_word(inspector.reg_pc()),
                    variables_reference: 0,
                    memory_reference: None,
                },
                byte_variable("FLAGS", inspector.flags()),
            ],
            MEMORY_VARIABLES_REFERENCE => vec![Variable {
                name: "Memory".to_string(),
                value: "$0000".to_string(),
                variables_reference: 0,
                memory_reference: Some("0x0000".to_string()),
            }],
            _ => vec![],
        };
        return (
            Response::Variables(VariablesResponse { variables: vars }),
            None,
        );
    }

    fn disassemble(
        &self,
        inspector: &impl MachineInspector,
        args: DisassembleArguments,
    ) -> RequestOutcome<A> {
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

    fn read_memory(
        &self,
        inspector: &impl MachineInspector,
        args: ReadMemoryArguments,
    ) -> RequestOutcome<A> {
        let start_address =
            i64::from_str_radix(&args.memory_reference.strip_prefix("0x").unwrap(), 16).unwrap()
                + args.offset.unwrap_or(0);
        let end_address = min(start_address + args.count, 0x10000);
        let mem_dump: Vec<u8> = (start_address..end_address)
            .map(|a| inspector.inspect_memory(a as u16))
            .collect();
        let data = base64::encode(mem_dump);
        (
            Response::ReadMemory(ReadMemoryResponse {
                address: format!("0x{:04X}", start_address),
                data,
            }),
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

    fn step_out(&mut self) -> RequestOutcome<A> {
        self.core.step_out();
        (Response::StepOut {}, None)
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
        memory_reference: None,
    }
}
