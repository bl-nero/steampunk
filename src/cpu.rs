use crate::memory::RAM;

#[derive(Debug)] //this generates function that translates CPU to text
pub struct CPU<'a> {
    program_counter: u16, // u means unsigned and 16 means it is 16 bit
    accumulator: u8,
    memory: &'a mut RAM, // & means reference
}

impl<'a> CPU<'a> {
    /// Creates a new `CPU` that owns given `memory`. The newly created `CPU` is
    /// not yet ready for executing programs; it first needs to be reset using
    /// the [`reset`](#method.reset) method.
    pub fn new(memory: &'a mut RAM) -> CPU<'a> {
        CPU {
            program_counter: 0,
            accumulator: 0,
            memory: memory,
        }
    }

    /// Reinitialize the CPU. It reads an address from 0xFFFA and stores it in
    /// the `PC` register. Next [`tick`](#method.tick) will effectively resume
    /// program from this address.
    pub fn reset(&mut self) {
        let lsb = self.memory.read(0xFFFA) as u16;
        let msb = self.memory.read(0xFFFB) as u16;
        self.program_counter = msb << 8 | lsb;
    }

    /// Performs a single CPU cycle.
    // self is CPU object we execute functiion on
    pub fn tick(&mut self) {
        // Read memory from address stored in program_counter. Store the value
        // in the opcode variable.
        let opcode = self.memory.read(self.program_counter);
        match opcode {
            opcodes::LDA => {
                self.accumulator = self.memory.read(self.program_counter + 1);
                self.program_counter = self.program_counter + 2;
            }
            opcodes::STA => {
                let address = self.memory.read(self.program_counter + 1);
                self.memory.write(address as u16, self.accumulator);
                self.program_counter = self.program_counter + 2;
            }
            other => {
                // Matches everything else.
                panic!(
                    "unknown opcode: ${:02X} at ${:04X}",
                    other, self.program_counter
                );
            }
        }
    }
}

mod opcodes {
    //opcodes are instruction in program codes
    pub const LDA: u8 = 0xa9; //0x means hexadecimal number
    pub const STA: u8 = 0x85;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_resets() {
        let mut program = vec![opcodes::LDA, 1, opcodes::STA, 0];
        program.resize(0x101, 0);
        program.extend_from_slice(&[opcodes::LDA, 2, opcodes::STA, 0]);

        let mut memory = RAM::new(&program);
        let mut cpu = CPU::new(&mut memory);
        cpu.reset();
        cpu.tick();
        cpu.tick();
        assert_eq!(cpu.memory.bytes[0], 1);

        cpu.memory.bytes[0xFFFA] = 0x01;
        cpu.memory.bytes[0xFFFB] = 0xF1;
        cpu.reset();
        cpu.tick();
        cpu.tick();
        assert_eq!(memory.bytes[0], 2);
    }

    #[test]
    fn lda_sta() {
        let mut memory = RAM::new(&mut [
            opcodes::LDA,
            65,
            opcodes::STA,
            4,
            opcodes::LDA,
            73,
            opcodes::STA,
            4,
            opcodes::LDA,
            12,
            opcodes::STA,
            5,
        ]);
        let mut cpu = CPU::new(&mut memory);
        cpu.reset();
        cpu.tick();
        cpu.tick();
        assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
        cpu.tick();
        cpu.tick();
        assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
        cpu.tick();
        cpu.tick();
        assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    }
}
