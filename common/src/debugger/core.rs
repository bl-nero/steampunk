use serde::Deserialize;
use serde::Serialize;
use std::mem::replace;
use ya6502::cpu::MachineInspector;

#[derive(PartialEq)]
enum RunMode {
    Running,
    Stopped,
    SteppingIn,
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
        if self.run_mode == RunMode::SteppingIn && inspector.at_instruction_start() {
            self.stop(StopReason::Step);
        }
        if inspector.at_instruction_start()
            && self.instruction_breakpoints.contains(&inspector.reg_pc())
        {
            self.stop(StopReason::Breakpoint);
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
        self.set_run_mode(RunMode::Running);
    }

    fn set_run_mode(&mut self, mode: RunMode) {
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

    pub fn step_in(&mut self) {
        self.set_run_mode(RunMode::SteppingIn);
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
        dc.step_in();
        assert_eq!(dc.last_stop_reason(), None);
    }

    #[test]
    fn step_in() {
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

        dc.step_in();
        assert!(!dc.stopped());
        assert_eq!(dc.last_stop_reason(), None);

        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF002);
        assert_eq!(dc.last_stop_reason(), Some(StopReason::Step));

        dc.step_in();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF004);

        dc.step_in();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF00B);

        dc.step_in();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF00C);

        dc.step_in();
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
