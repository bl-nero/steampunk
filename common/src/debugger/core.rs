use std::mem::replace;
use ya6502::cpu::MachineInspector;

/// The actual logic of the debugger, free of all of the communication noise.
pub struct DebuggerCore {
    paused: bool,
    will_step_in: bool,
    has_just_paused: bool,
}

impl DebuggerCore {
    pub fn new() -> Self {
        Self {
            paused: true,
            will_step_in: false,
            has_just_paused: false,
        }
    }

    pub fn update<I: MachineInspector>(&mut self, inspector: &I) {
        if self.will_step_in && inspector.at_instruction_start() {
            self.pause();
        }
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    /// Returns `true` if the core has just paused and resets the value to
    /// `false`. Note that this is a kludge that works here instead of
    /// introducing a proper observer or event emitter pattern; that's because
    /// we don't want to dispatch these events to the debugger client
    /// immediately anyway.
    pub fn has_just_paused(&mut self) -> bool {
        replace(&mut self.has_just_paused, false)
    }

    pub fn resume(&mut self) {
        self.paused = false;
        self.has_just_paused = false;
        self.will_step_in = false;
    }

    pub fn pause(&mut self) {
        self.paused = true;
        self.has_just_paused = true;
    }

    pub fn step_in(&mut self) {
        self.resume();
        self.will_step_in = true;
    }
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
            if dc.paused() {
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
        assert!(dc.paused());

        dc.resume();
        assert!(!dc.paused());

        cpu.tick().unwrap();
        dc.update(&cpu);
        assert!(!dc.paused());

        cpu.tick().unwrap();
        dc.update(&cpu);
        assert!(!dc.paused());

        cpu.tick().unwrap();
        dc.update(&cpu);
        assert!(!dc.paused());

        cpu.tick().unwrap();
        dc.update(&cpu);
        assert!(!dc.paused());

        dc.pause();
        assert!(dc.paused());
    }

    #[test]
    fn has_just_paused() {
        let mut dc = DebuggerCore::new();
        assert!(!dc.has_just_paused());
        assert!(!dc.has_just_paused());

        dc.resume();
        assert!(!dc.has_just_paused());
        assert!(!dc.has_just_paused());

        dc.pause();
        assert!(dc.has_just_paused());
        assert!(!dc.has_just_paused());

        dc.pause();
        dc.resume();
        assert!(!dc.has_just_paused());

        dc.pause();
        dc.step_in();
        assert!(!dc.has_just_paused());
    }

    #[test]
    fn has_just_paused_while_stepping() {
        let mut cpu = cpu_with_code! {
                nop
        };
        let mut dc = DebuggerCore::new();

        dc.step_in();
        assert!(!dc.has_just_paused());
        tick_while_running(&mut dc, &mut cpu);
        assert!(dc.has_just_paused());
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
        assert!(!dc.paused());
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF002);

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
        assert!(!dc.paused());

        cpu.tick().unwrap();
        dc.update(&cpu);
        assert!(!dc.paused());
    }
}
