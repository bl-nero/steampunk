#![cfg(test)]

use super::*;
use crate::debugger::adapter::FakeDebugAdapter;
use crate::debugger::dap_types::Breakpoint;
use crate::debugger::dap_types::DisassembledInstruction;
use crate::debugger::dap_types::InitializeArguments;
use crate::debugger::dap_types::InstructionBreakpoint;
use crate::debugger::dap_types::MessageEnvelope;
use crate::debugger::dap_types::ScopesArguments;
use crate::debugger::dap_types::SetInstructionBreakpointsArguments;
use crate::debugger::dap_types::VariablesArguments;
use std::assert_matches::assert_matches;
use ya6502::cpu::Cpu;
use ya6502::cpu::MockMachineInspector;
use ya6502::cpu_with_code;
use ya6502::memory::Ram;
use ya6502::test_utils::cpu_with_program;

fn pop_response(adapter: &FakeDebugAdapter) -> Response {
    match adapter.pop_outgoing() {
        Some(MessageEnvelope {
            message: Message::Response(ResponseEnvelope { response, .. }),
            ..
        }) => response,
        other => panic!("Expected a response, got {:?}", other),
    }
}

fn assert_responded_with(adapter: &FakeDebugAdapter, expected_response: Response) {
    let response = pop_response(adapter);
    assert_eq!(response, expected_response);
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

fn purge_messages(adapter: &FakeDebugAdapter) {
    while adapter.pop_outgoing().is_some() {}
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
            supports_read_memory_request: true,
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
    let mut cpu = cpu_with_code! {
            nop            // 0xF000
            jsr subroutine // 0xF001
            nop            // 0xF004
        subroutine:
            rts            // 0xF005
    };

    let adapter = FakeDebugAdapter::default();
    let mut debugger = Debugger::new(adapter.clone());
    debugger.update(&cpu).unwrap();

    adapter.push_request(Request::StackTrace {});
    debugger.process_messages(&cpu);
    assert_responded_with(
        &adapter,
        Response::StackTrace(StackTraceResponse {
            stack_frames: vec![StackFrame {
                id: 1,
                name: "$F000".to_string(),
                line: 0,
                column: 0,
                instruction_pointer_reference: "0xF000".to_string(),
            }],
            total_frames: 1,
        }),
    );
    assert_eq!(adapter.pop_outgoing(), None);

    adapter.push_request(Request::StepIn {});
    debugger.process_messages(&cpu);
    tick_while_running(&mut debugger, &mut cpu);
    adapter.push_request(Request::StepIn {});
    debugger.process_messages(&cpu);
    tick_while_running(&mut debugger, &mut cpu);
    purge_messages(&adapter);
    assert_eq!(cpu.reg_pc(), 0xF005);

    adapter.push_request(Request::StackTrace {});
    debugger.process_messages(&cpu);
    assert_responded_with(
        &adapter,
        Response::StackTrace(StackTraceResponse {
            stack_frames: vec![
                StackFrame {
                    id: 2,
                    name: "$F005".to_string(),
                    line: 0,
                    column: 0,
                    instruction_pointer_reference: "0xF005".to_string(),
                },
                StackFrame {
                    id: 1,
                    name: "$F000".to_string(),
                    line: 0,
                    column: 0,
                    instruction_pointer_reference: "0xF001".to_string(),
                },
            ],
            total_frames: 2,
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
    debugger.update(&cpu).unwrap();

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
    debugger.update(&cpu).unwrap();

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
fn read_memory() {
    let cpu = cpu_with_program(&[0x8B, 0xAD, 0xF0, 0x0D]);
    let adapter = FakeDebugAdapter::default();
    let mut debugger = Debugger::new(adapter.clone());
    debugger.update(&cpu).unwrap();

    adapter.push_request(Request::ReadMemory(ReadMemoryArguments {
        memory_reference: "0xF000".to_string(),
        offset: None,
        count: 4,
    }));
    adapter.push_request(Request::ReadMemory(ReadMemoryArguments {
        memory_reference: "0xF001".to_string(),
        offset: None,
        count: 2,
    }));
    debugger.process_messages(&cpu);

    assert_responded_with(
        &adapter,
        Response::ReadMemory(ReadMemoryResponse {
            address: "0xF000".to_string(),
            data: "i63wDQ==".to_string(),
        }),
    );
    assert_responded_with(
        &adapter,
        Response::ReadMemory(ReadMemoryResponse {
            address: "0xF001".to_string(),
            data: "rfA=".to_string(),
        }),
    );
    assert_eq!(adapter.pop_outgoing(), None);
}

#[test]
fn read_memory_with_offset() {
    let cpu = cpu_with_program(&[0x8B, 0xAD, 0xF0, 0x0D]);
    let adapter = FakeDebugAdapter::default();
    let mut debugger = Debugger::new(adapter.clone());
    debugger.update(&cpu).unwrap();

    adapter.push_request(Request::ReadMemory(ReadMemoryArguments {
        memory_reference: "0xF003".to_string(),
        offset: Some(-2),
        count: 2,
    }));
    debugger.process_messages(&cpu);

    assert_responded_with(
        &adapter,
        Response::ReadMemory(ReadMemoryResponse {
            address: "0xF001".to_string(),
            data: "rfA=".to_string(),
        }),
    );
    assert_eq!(adapter.pop_outgoing(), None);
}

#[test]
fn read_memory_truncates_after_last_bytes() {
    let mut cpu = cpu_with_program(&[]);
    cpu.mut_memory().bytes[0xFFFE..=0xFFFF].copy_from_slice(&[0xF0, 0x0D]);
    let adapter = FakeDebugAdapter::default();
    let mut debugger = Debugger::new(adapter.clone());
    debugger.update(&cpu).unwrap();

    adapter.push_request(Request::ReadMemory(ReadMemoryArguments {
        memory_reference: "0xFFFE".to_string(),
        offset: Some(0),
        count: 10,
    }));
    debugger.process_messages(&cpu);

    assert_responded_with(
        &adapter,
        Response::ReadMemory(ReadMemoryResponse {
            address: "0xFFFE".to_string(),
            data: "8A0=".to_string(),
        }),
    );
    assert_eq!(adapter.pop_outgoing(), None);
}

fn get_stack_frames(
    adapter: &FakeDebugAdapter,
    debugger: &mut Debugger<FakeDebugAdapter>,
    cpu: &Cpu<Ram>,
) -> Vec<StackFrame> {
    adapter.push_request(Request::StackTrace {});
    debugger.process_messages(cpu);
    let stack_trace_response = pop_response(&adapter);
    return match stack_trace_response {
        Response::StackTrace(StackTraceResponse { stack_frames, .. }) => stack_frames,
        other => panic!("Expected StackTraceResponse, got {:?}", other),
    };
}

fn get_scopes(
    adapter: &FakeDebugAdapter,
    debugger: &mut Debugger<FakeDebugAdapter>,
    cpu: &Cpu<Ram>,
    frame_id: i64,
) -> Vec<Scope> {
    adapter.push_request(Request::Scopes(ScopesArguments { frame_id }));
    debugger.process_messages(cpu);
    let scopes_response = pop_response(&adapter);
    return match scopes_response {
        Response::Scopes(ScopesResponse { scopes }) => scopes,
        other => panic!("Expected a ScopesResponse, got {:?}", other),
    };
}

// And the prize for the uglies test in this entire codebase goes to...
#[test]
fn variables() {
    let mut cpu = cpu_with_code! {
            ldx #0xFE      // 0xF000
            txs            // 0xF002
            plp            // 0xF003
            lda #0xAB      // 0xF004
            ldy #0x12      // 0xF006
            jsr subroutine // 0xF008
        loop:
            jmp loop       // 0xF00B

        subroutine:
            inx            // 0xF00E
            dey            // 0xF00F
            rts            // 0xF010
    };

    let adapter = FakeDebugAdapter::default();
    let mut debugger = Debugger::new(adapter.clone());
    debugger.update(&cpu).unwrap();

    adapter.push_request(Request::SetInstructionBreakpoints(
        SetInstructionBreakpointsArguments {
            breakpoints: vec![
                InstructionBreakpoint {
                    instruction_reference: "0xF008".to_string(),
                    offset: None,
                },
                InstructionBreakpoint {
                    instruction_reference: "0xF010".to_string(),
                    offset: None,
                },
            ],
        },
    ));
    adapter.push_request(Request::Continue {});
    debugger.process_messages(&cpu);
    tick_while_running(&mut debugger, &mut cpu);
    purge_messages(&adapter);
    assert_eq!(cpu.reg_pc(), 0xF008);

    let stack_frames = get_stack_frames(&adapter, &mut debugger, &cpu);
    let frame_1_id = stack_frames[0].id;
    let scopes = get_scopes(&adapter, &mut debugger, &cpu, frame_1_id);
    assert_eq!(scopes.len(), 1);
    assert_eq!(scopes[0].name, "Registers");
    assert_eq!(
        scopes[0].presentation_hint,
        ScopePresentationHint::Registers,
    );
    assert_eq!(scopes[0].expensive, false);
    let variables_reference = scopes[0].variables_reference;

    adapter.push_request(Request::Variables(VariablesArguments {
        variables_reference,
    }));
    debugger.process_messages(&cpu);
    assert_responded_with(
        &adapter,
        Response::Variables(VariablesResponse {
            variables: vec![
                Variable {
                    name: "A".to_string(),
                    value: "$AB".to_string(),
                    variables_reference: 0,
                    memory_reference: None,
                },
                Variable {
                    name: "X".to_string(),
                    value: "$FE".to_string(),
                    variables_reference: 0,
                    memory_reference: None,
                },
                Variable {
                    name: "Y".to_string(),
                    value: "$12".to_string(),
                    variables_reference: 0,
                    memory_reference: None,
                },
                Variable {
                    name: "SP".to_string(),
                    value: "$FF".to_string(),
                    variables_reference: 0,
                    memory_reference: None,
                },
                Variable {
                    name: "PC".to_string(),
                    value: "$F008".to_string(),
                    variables_reference: 0,
                    memory_reference: None,
                },
                Variable {
                    name: "FLAGS".to_string(),
                    value: "$00".to_string(),
                    variables_reference: 0,
                    memory_reference: None,
                },
            ],
        }),
    );

    adapter.push_request(Request::Continue {});
    debugger.process_messages(&cpu);
    tick_while_running(&mut debugger, &mut cpu);
    purge_messages(&adapter);
    assert_eq!(cpu.reg_pc(), 0xF010);

    let stack_frames = get_stack_frames(&adapter, &mut debugger, &cpu);
    assert_eq!(stack_frames.len(), 2);
    let frame_2_id = stack_frames[0].id;
    let scopes = get_scopes(&adapter, &mut debugger, &cpu, frame_2_id);
    assert_eq!(scopes.len(), 1);
    assert_eq!(scopes[0].name, "Registers");
    assert_eq!(
        scopes[0].presentation_hint,
        ScopePresentationHint::Registers,
    );
    assert_eq!(scopes[0].expensive, false);
    let variables_reference = scopes[0].variables_reference;

    adapter.push_request(Request::Variables(VariablesArguments {
        variables_reference,
    }));
    debugger.process_messages(&cpu);
    assert_responded_with(
        &adapter,
        Response::Variables(VariablesResponse {
            variables: vec![
                Variable {
                    name: "A".to_string(),
                    value: "$AB".to_string(),
                    variables_reference: 0,
                    memory_reference: None,
                },
                Variable {
                    name: "X".to_string(),
                    value: "$FF".to_string(),
                    variables_reference: 0,
                    memory_reference: None,
                },
                Variable {
                    name: "Y".to_string(),
                    value: "$11".to_string(),
                    variables_reference: 0,
                    memory_reference: None,
                },
                Variable {
                    name: "SP".to_string(),
                    value: "$FD".to_string(),
                    variables_reference: 0,
                    memory_reference: None,
                },
                Variable {
                    name: "PC".to_string(),
                    value: "$F010".to_string(),
                    variables_reference: 0,
                    memory_reference: None,
                },
                Variable {
                    name: "FLAGS".to_string(),
                    value: "$00".to_string(),
                    variables_reference: 0,
                    memory_reference: None,
                },
            ],
        }),
    );

    assert_eq!(stack_frames[1].id, frame_1_id);
    let scopes = get_scopes(&adapter, &mut debugger, &cpu, frame_1_id);
    assert_eq!(scopes.len(), 0);
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
    debugger.update(&cpu).unwrap();

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
    debugger.update(&cpu).unwrap();

    debugger.process_messages(&cpu);

    purge_messages(&adapter);
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
fn step_out() {
    let mut cpu = cpu_with_code! {
            jsr subroutine // 0xF000
        loop:
            jmp loop       // 0xF003
        subroutine:
            nop            // 0xF006
            nop
            rts
    };

    let adapter = FakeDebugAdapter::default();
    adapter.push_request(Request::StepIn {});
    let mut debugger = Debugger::new(adapter.clone());
    debugger.update(&cpu).unwrap();
    debugger.process_messages(&cpu);
    tick_while_running(&mut debugger, &mut cpu);
    assert_eq!(cpu.reg_pc(), 0xF006);

    purge_messages(&adapter);
    adapter.push_request(Request::StepOut {});
    debugger.process_messages(&cpu);
    assert_responded_with(&adapter, Response::StepOut {});
    assert_eq!(adapter.pop_outgoing(), None);

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
    debugger.update(&cpu).unwrap();

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

    purge_messages(&adapter);
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

    purge_messages(&adapter);
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
