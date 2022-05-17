use bounded_vec_deque::BoundedVecDeque;
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
    SteppingOut { target_stack_depth: usize },
}

/// The actual logic of the debugger, free of all of the communication noise.
pub struct DebuggerCore {
    run_mode: RunMode,
    last_stop_reason: Option<StopReason>,
    instruction_breakpoints: Vec<u16>,
    /// Stack frames, captured by recognizing JSR/RTS instructions. Note that
    /// this is not a simple vector, but a bounded deque, since we can't
    /// guarantee that the underlying program is sane and won't overflow the
    /// stack. An edge case of consistently overflowing stack would cause a
    /// dramatic memory leak here, and since the stack entries would be
    /// clobbered anyway, the bounded deque is the perfect structure here.
    ///
    /// TODO: Support stepping out of interrupt handlers.
    stack_frames: BoundedVecDeque<StackFrame>,
    will_enter_subroutine: bool,
    will_return_from_subroutine: bool,
}

impl DebuggerCore {
    pub fn new() -> Self {
        Self {
            run_mode: RunMode::Stopped,
            last_stop_reason: None,
            instruction_breakpoints: vec![],
            stack_frames: BoundedVecDeque::new(256),
            will_enter_subroutine: true,
            will_return_from_subroutine: false,
        }
    }

    pub fn set_instruction_breakpoints(&mut self, breakpoints: Vec<u16>) {
        self.instruction_breakpoints = breakpoints;
    }

    /// Reads the machine state. Expected to be called after the CPU is
    /// initialized, and then after every single cycle.
    pub fn update(&mut self, inspector: &impl MachineInspector) {
        if inspector.at_instruction_start() {
            if self.will_enter_subroutine {
                self.stack_frames.push_back(StackFrame {
                    entry: inspector.reg_pc(),
                    pc: 0,
                });
                self.will_enter_subroutine = false;
            }
            if self.will_return_from_subroutine {
                self.stack_frames.pop_back();
                self.will_return_from_subroutine = false;
            }
            let opcode = inspector.inspect_memory(inspector.reg_pc());
            match opcode {
                opcodes::JSR => {
                    self.will_enter_subroutine = true;
                    if let Some(current_frame) = self.stack_frames.back_mut() {
                        current_frame.pc = inspector.reg_pc();
                    }
                }
                opcodes::RTS => {
                    self.will_return_from_subroutine = true;
                }
                _ => {}
            }
            match self.run_mode {
                RunMode::Running => {
                    if self.instruction_breakpoints.contains(&inspector.reg_pc()) {
                        self.stop(StopReason::Breakpoint);
                    }
                }
                RunMode::SteppingIn => self.stop(StopReason::Step),
                RunMode::SteppingOut { target_stack_depth } => {
                    if self.stack_frames.len() == target_stack_depth {
                        self.stop(StopReason::Step);
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

    pub fn stack_trace(&self, inspector: &impl MachineInspector) -> Vec<StackFrame> {
        let mut frames: Vec<StackFrame> = self.stack_frames.clone().into_unbounded().into();
        frames.reverse();
        if let Some(top_frame) = frames.first_mut() {
            top_frame.pc = inspector.reg_pc();
        }
        return frames;
    }

    pub fn stack_depth(&self) -> usize {
        self.stack_frames.len()
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
        if opcode == opcodes::JSR {
            self.run(RunMode::SteppingOut {
                target_stack_depth: self.stack_frames.len(),
            });
        } else {
            self.run(RunMode::SteppingIn);
        };
    }

    pub fn step_out(&mut self) {
        self.run(RunMode::SteppingOut {
            target_stack_depth: self.stack_frames.len() - 1,
        });
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct StackFrame {
    pub entry: u16,
    pub pc: u16,
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
        dc.update(&cpu);
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
        dc.update(&cpu);
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
        dc.update(&cpu);

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
    fn step_over_multiple() {
        let mut cpu = cpu_with_code! {
                jsr subroutine // 0xF000
                jsr subroutine // 0xF003
            loop:
                jmp loop       // 0xF006

            subroutine:
                rts
        };
        let mut dc = DebuggerCore::new();
        dc.update(&cpu);

        dc.step_over(&cpu);
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF003);

        dc.step_over(&cpu);
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF006);
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
        dc.update(&cpu);

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
        let mut dc = DebuggerCore::new();
        dc.update(&cpu);
        dc.step_into();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF006);

        dc.step_out();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF003);
    }

    #[test]
    fn step_out_nested() {
        let mut cpu = cpu_with_code! {
                jsr sub1 // 0xF000
            loop:
                jmp loop // 0xF003

            sub1:
                nop      // 0xF006
                nop
                jsr sub2
                rts

            sub2:
                rts
        };
        let mut dc = DebuggerCore::new();
        dc.update(&cpu);
        dc.step_into();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF006);

        dc.step_out();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF003);
    }

    #[test]
    fn step_out_with_stack_operations() {
        let mut cpu = cpu_with_code! {
                jsr subroutine // 0xF000
            loop:
                jmp loop       // 0xF003

            subroutine:
                pha            // 0xF006
                pla            // 0xF007
                rts
        };
        let mut dc = DebuggerCore::new();
        dc.update(&cpu);
        dc.step_into();
        tick_while_running(&mut dc, &mut cpu);
        dc.step_into();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF007);

        dc.step_out();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(cpu.reg_pc(), 0xF003);
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
        dc.update(&cpu);
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

    #[test]
    fn stack_frames_only_top() {
        let mut cpu = cpu_with_code! {
                nop
                nop
                jsr sub1
            loop:
                jmp loop

            sub1:
                jsr sub2
                rts

            sub2:
                rts
        };
        let mut dc = DebuggerCore::new();
        dc.update(&cpu);
        assert_eq!(
            dc.stack_trace(&cpu),
            vec![StackFrame {
                entry: 0xF000,
                pc: 0xF000
            }]
        );

        dc.step_into();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(
            dc.stack_trace(&cpu),
            vec![StackFrame {
                entry: 0xF000,
                pc: 0xF001
            }]
        );

        dc.step_into();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(
            dc.stack_trace(&cpu),
            vec![StackFrame {
                entry: 0xF000,
                pc: 0xF002
            }]
        );

        dc.step_into();
        tick_while_running(&mut dc, &mut cpu);
        assert_eq!(
            dc.stack_trace(&cpu),
            vec![
                StackFrame {
                    entry: 0xF008,
                    pc: 0xF008
                },
                StackFrame {
                    entry: 0xF000,
                    pc: 0xF002
                }
            ]
        );
    }
}
