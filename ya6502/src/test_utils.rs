use crate::cpu::opcodes;
use crate::cpu::Cpu;
use crate::memory::Memory;
use crate::memory::Ram;
use std::fmt::Debug;

/// Resets the CPU and waits until the reset procedure is finished.
pub fn reset<M: Memory + Debug>(cpu: &mut Cpu<M>) {
    cpu.reset();
    cpu.ticks(7).unwrap();
}

/// Creates a CPU with a given program loaded at 0xF000. Add a HLT instruction
/// at the end to make sure we got the timing right and don't execute one
/// instruction too many. It also sets the reset vector to the beginning of
/// program.
pub fn cpu_with_program(program: &[u8]) -> Cpu<Ram> {
    let mut memory = Box::new(Ram::with_test_program(program));
    memory.bytes[0xF000 + program.len()] = opcodes::HLT1;
    let mut cpu = Cpu::new(memory);
    reset(&mut cpu);
    return cpu;
}

/// Returns a CPU that will execute given assembly code.
#[macro_export]
macro_rules! cpu_with_code {
    ($($tokens:tt)*) => {
        cpu_with_program(&assemble6502!({
            start: 0xF000,
            code: {$($tokens)*}
        }))
    };
}
