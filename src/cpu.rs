use crate::memory::Memory;

#[derive(Debug)] //this generates function that translates CPU to text
pub struct CPU<'a, M: Memory> {
    program_counter: u16, // u means unsigned and 16 means it is 16 bit
    accumulator: u8,
    xreg: u8,
    memory: &'a mut M, // & means reference
    yreg: u8,
}

impl<'a, M: Memory> CPU<'a, M> {
    /// Creates a new `CPU` that owns given `memory`. The newly created `CPU` is
    /// not yet ready for executing programs; it first needs to be reset using
    /// the [`reset`](#method.reset) method.
    pub fn new(memory: &'a mut M) -> CPU<'a, M> {
        CPU {
            program_counter: 0,
            accumulator: 0,
            xreg: 0,
            memory: memory,
            yreg: 0,
        }
    }

    /// Reinitialize the CPU. It reads an address from 0xFFFA and stores it in
    /// the `PC` register. Next [`tick`](#method.tick) will effectively resume
    /// program from this address.
    pub fn reset(&mut self) {
        let lsb = self.memory.read(0xFFFA) as u16;
        let msb = self.memory.read(0xFFFB) as u16;
        self.program_counter = (msb << 8) | lsb;
    }

    /// Performs a single CPU cycle.
    // self is CPU object we execute function on
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
            opcodes::LDX => {
                self.xreg = self.memory.read(self.program_counter + 1);
                self.program_counter = self.program_counter + 2;
            }
            opcodes::STX => {
                let address = self.memory.read(self.program_counter + 1);
                self.memory.write(address as u16, self.xreg);
                self.program_counter = self.program_counter + 2;
            }
            opcodes::INX => {
                self.xreg = self.xreg.wrapping_add(1);
                self.program_counter = self.program_counter + 1;
            }
            opcodes::LDY => {
                self.yreg = self.memory.read(self.program_counter + 1);
                self.program_counter = self.program_counter + 2;
            }
            opcodes::INY => {
                self.yreg = self.xreg.wrapping_add(1);
                self.program_counter = self.program_counter + 1;
            }
            opcodes::STY => {
                let address = self.memory.read(self.program_counter + 1);
                self.memory.write(address as u16, self.yreg);
                self.program_counter = self.program_counter + 2;
            }
            opcodes::JMP => {
                let lsb = self.memory.read(self.program_counter + 1);
                let msb = self.memory.read(self.program_counter + 2);
                self.program_counter = (lsb as u16)|((msb as u16)<<8);
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
    pub const LDX: u8 = 0xa2;
    pub const STX: u8 = 0x44;
    pub const INX: u8 = 0xe8;
    pub const JMP: u8 = 0x4c;
    pub const INY: u8 = 0xC8;
    pub const STY: u8 = 0x8C;
    pub const LDY: u8 = 0xBC;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::RAM;

    #[test]
    fn it_resets() {
        // We test resetting the CPU by providing a memory image with two
        // separate programs. The first starts, as usually, at 0xF000, and it
        // will store a value of 1 at 0x0000.
        let mut program = vec![opcodes::LDA, 1, opcodes::STA, 0];
        // The next one will start exactly 0x101 bytes later, at 0xF101. This is
        // because we want to change both bytes of the program's address. We
        // resize the memory so that it contains zeros until 0xF101.
        program.resize(0x101, 0);
        // Finally, the second program. It stores 2 at 0x0000.
        program.extend_from_slice(&[opcodes::LDA, 2, opcodes::STA, 0]);

        let mut memory = RAM::with_program(&program);
        let mut cpu = CPU::new(&mut memory);
        cpu.reset();
        cpu.tick();
        cpu.tick();
        assert_eq!(cpu.memory.bytes[0], 1); // The first program has been executed.

        cpu.memory.bytes[0xFFFA] = 0x01;
        cpu.memory.bytes[0xFFFB] = 0xF1;
        cpu.reset();
        cpu.tick();
        cpu.tick();
        assert_eq!(memory.bytes[0], 2); // The second program has been executed.
    }

    #[test]
    fn inx() {
        let mut memory = RAM::with_program(&mut [
            opcodes::LDX,
            0xFE,
            opcodes::INX,
            opcodes::STX,
            5,
            opcodes::INX,
            opcodes::STX,
            6,
        ]);
        let mut cpu = CPU::new(&mut memory);
        cpu.reset();
        cpu.tick();
        cpu.tick();
        cpu.tick();
        cpu.tick();
        cpu.tick();
        assert_eq!(cpu.memory.bytes[5..7], [0xFF, 0x00]);
    }
    #[test]
    fn ldx_stx() {
        let mut memory = RAM::with_program(&mut [
            opcodes::LDX,
            65,
            opcodes::STX,
            4,
            opcodes::LDX,
            73,
            opcodes::STX,
            4,
            opcodes::LDX,
            12,
            opcodes::STX,
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

    #[test]
    fn lda_sta() {
        let mut memory = RAM::with_program(&mut [
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

    #[test]
    fn multiple_registers() {
        let mut memory = RAM::with_program(&mut [
            opcodes::LDA,
            10,
            opcodes::LDX,
            20,
            opcodes::STA,
            0,
            opcodes::STX,
            1,
        ]);
        let mut cpu = CPU::new(&mut memory);
        cpu.reset();
        cpu.tick();
        cpu.tick();
        cpu.tick();
        cpu.tick();
        assert_eq!(cpu.memory.bytes[0..2], [10, 20]);
    }
    #[test]
    fn jmp_working() {
        let mut memory = RAM::with_program(&mut [
            opcodes::LDX,
            1,
            opcodes::STX,
            9,
            opcodes::INX,
            opcodes::JMP,
            0x02,
            0xf0,
        ]);
        let mut cpu = CPU::new(&mut memory);
        cpu.reset();
        cpu.tick();
        cpu.tick();
        cpu.tick();
        cpu.tick();
        cpu.tick();
        assert_eq!(cpu.memory.bytes[9], 2);
    }
}    
