use serde::Deserialize;
use serde::Serialize;
use std::mem::replace;
use ya6502::cpu::opcodes;
use ya6502::cpu::MachineInspector;

#[derive(PartialEq)]
enum RunMode {
    Running,
    Stopped,
    SteppingIn,
    SteppingOver {
        target_address: u16,
        target_stack_pointer: u8,
    },
}

/// The actual logic of the debugger, free of all of the communication noise.
pub struct DebuggerCore {
    run_mode: RunMode,
    last_stop_reason: Option<StopReason>,
    instruction_breakpoints: Vec<u16>,
}

impl DebuggerCore {
    pub fn new() -> Self {
        Self {
            run_mode: RunMode::Stopped,
            last_stop_reason: None,
            instruction_breakpoints: vec![],
        }
    }

    pub fn set_instruction_breakpoints(&mut self, breakpoints: Vec<u16>) {
        self.instruction_breakpoints = breakpoints;
    }

    pub fn update(&mut self, inspector: &impl MachineInspector) {
        if inspector.at_instruction_start() {
            match self.run_mode {
                RunMode::SteppingIn => self.stop(StopReason::Step),
                RunMode::SteppingOver {
                    target_address,
                    target_stack_pointer,
                } => {
                    if inspector.reg_pc() == target_address
                        && inspector.reg_sp() == target_stack_pointer
                    {
                        self.stop(StopReason::Step);
                    }
                }
                RunMode::Running => {
                    if self.instruction_breakpoints.contains(&inspector.reg_pc()) {
                        self.stop(StopReason::Breakpoint);
                    }
                }
                RunMode::Stopped => {}
            }
        }
    }

    pub fn stopped(&self) -> bool {
        self.run_mode == RunMode::Stopped
    }

    /// Returns `Some(reason)` if the core has just stopped and resets the value
    /// to `None`. Note that this is a kludge that works here instead of
    /// introducing a proper observer or event emitter pattern; that's because
    /// we don't want to dispatch these events to the debugger client
    /// immediately anyway.
    pub fn last_stop_reason(&mut self) -> Option<StopReason> {
        replace(&mut self.last_stop_reason, None)
    }

    pub fn resume(&mut self) {
        self.run(RunMode::Running);
    }

    fn run(&mut self, mode: RunMode) {
        self.run_mode = mode;
        self.last_stop_reason = None;
    }

    pub fn pause(&mut self) {
        self.stop(StopReason::Pause);
    }

    fn stop(&mut self, reason: StopReason) {
        self.run_mode = RunMode::Stopped;
        self.last_stop_reason = Some(reason);
    }

    pub fn step_into(&mut self) {
        self.run(RunMode::SteppingIn);
    }

    pub fn step_over(&mut self, inspector: &impl MachineInspector) {
        let pc = inspector.reg_pc();
        let opcode = inspector.inspect_memory(pc);
        // Note: Stepping over is only "special" when we perform a jump into a
        // subroutine. Otherwise, it's the same as stepping in.
        self.run(if opcode == opcodes::JSR {
            RunMode::SteppingOver {
                target_address: pc.wrapping_add(3),
                target_stack_pointer: inspector.reg_sp(),
            }
        } else {
            RunMode::SteppingIn
        });
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    Entry,
    Pause,
    Step,
    Breakpoint,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ya6502::cpu::Cpu;
    use ya6502::cpu_with_code;
    use ya6502::memory::Ram;

    fn tick_while_running(dc: &mut DebuggerCore, cpu: &mut Cpu<Ram>) {
        // Limit to 1000 ticks; we won't expect tests to run for that long, and
        // this way we avoid infinite loops.
        for _ in 0..1000 {
            if dc.stopped() {
                return;
            }
            cpu.tick().unwrap();
            dc.update(cpu);
        }
        panic!("CPU still running at PC={:04X}", cpu.reg_pc());
    }

    #[test]
    fn runs_and_pauses() {
        let mut cpu = cpu_with_code! {
                nop
                nop
        };
        let mut dc = DebuggerCore::new();
        assert!(dc.stopped());

        dc.resume();
        assert!(!dc.stopped());

        cpu.tick().unwrap();
        dc.update(&cpu);
        assert!(!dc.stopped());

        cpu.tick().unwrap();
        dc.update(&cpu);
        assert!(!dc.stopped());

        cpu.tick().unwrap();
        dc.update(&cpu);
        assert!(!dc.stopped());

        cpu.tick().unwrap();
        dc.update(&cpu);
        assert!(!dc.stopped());

        dc.pause();
        assert!(dc.stopped());
    }

    #[test]
    fn last_stop_reason() {
        let mut dc = DebuggerCore::new();
        assert_eq!(dc.last_stop_reason(), None);
        assert_eq!(dc.last_stop_reason(), None);

        dc.resume();
        assert_eq!(dc.last_stop_reason(), None);
        assert_eq!(dc.last_stop_reason(), None);

        dc.pause();
        assert_eq!(dc.last_stop_reason(), Some(StopReason::Pause));
        assert_eq!(dc.last_stop_reason(), None);

        dc.pause();
        dc.resume();
        assert_eq!(dc.last_stop_reason(), None);

        dc.pause();
        dc.step_into();
        assert_eq!(dc.last_stop_reason(), None);
    }

    #[test]
    fn step_into() {
        let mut cpu = cpu_with_code! {
                lda #1         // 0xF000
                sta 1          // 0xF002
                jsr subroutine // 0xF004
            loop:
                nop            // 0xF007
                jmp loop       // 0xF008

            subroutine:
                nop            // 0xF00B
                rts            // 0xF00C
        };
        let mut dc = DebuggerCore::new();
        assert_eq!(cpu.reg_pc(), 0xF000);

        dc.step_into();
        assert!(!dc.stopped());
        assert_eq!(dc.last_stop_reason(), None);

        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF002);
        assert_eq!(dc.last_stop_reason(), Some(StopReason::Step));

        dc.step_into();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF004);

        dc.step_into();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF00B);

        dc.step_into();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF00C);

        dc.step_into();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF007);

        dc.resume();
        cpu.tick().unwrap();
        dc.update(&cpu);
        assert!(!dc.stopped());

        cpu.tick().unwrap();
        dc.update(&cpu);
        assert!(!dc.stopped());
    }

    #[test]
    fn step_over() {
        let mut cpu = cpu_with_code! {
                lda #1         // 0xF000
                sta 1          // 0xF002
                jsr subroutine // 0xF004
            loop:
                nop            // 0xF007
                jmp loop       // 0xF008

            subroutine:
                nop            // 0xF00B
                rts            // 0xF00C
        };
        let mut dc = DebuggerCore::new();

        dc.step_over(&cpu);
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF002);
        assert_eq!(dc.last_stop_reason(), Some(StopReason::Step));

        dc.step_over(&cpu);
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF004);

        dc.step_over(&cpu);
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF007);
    }

    #[test]
    fn step_over_recursive() {
        let mut cpu = cpu_with_code! {
                ldx #6         // 0xF000
                ldy #0         // 0xF002
                jsr subroutine // 0xF004
            loop:
                jmp loop       // 0xF007
            subroutine:
                dex            // 0xF00A
                beq skip       // 0xF00B
                jsr subroutine // 0xF00D
            skip:
                iny            // 0xF010
                rts            // 0xF011
        };
        let mut dc = DebuggerCore::new();

        dc.step_over(&cpu);
        tick_while_running(&mut dc, &mut cpu);
        dc.step_over(&cpu);
        tick_while_running(&mut dc, &mut cpu);
        dc.step_into();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF00A);

        dc.step_over(&cpu);
        tick_while_running(&mut dc, &mut cpu);
        dc.step_over(&cpu);
        tick_while_running(&mut dc, &mut cpu);
        dc.step_over(&cpu);
        tick_while_running(&mut dc, &mut cpu);

        assert_eq!(cpu.reg_pc(), 0xF010);
        assert_eq!(cpu.reg_y(), 5);
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
        let mut dc = DebuggerCore::new();
        dc.set_instruction_breakpoints(vec![0xF002]);
        dc.resume();

        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF002);
        assert_eq!(dc.last_stop_reason(), Some(StopReason::Breakpoint));

        cpu.reset();
        dc.set_instruction_breakpoints(vec![0xF001, 0xF003]);

        dc.resume();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF001);
        assert_eq!(dc.last_stop_reason(), Some(StopReason::Breakpoint));

        dc.resume();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF003);
        assert_eq!(dc.last_stop_reason(), Some(StopReason::Breakpoint));
    }
}
