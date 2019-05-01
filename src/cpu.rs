use crate::memory::RAM;

#[derive(Debug)] //this generates function that translates CPU to text
pub struct CPU<'a> {
    program_counter: u16, // u means unsigned and 16 means it is 16 bit
    accumulator: u8,
    memory: &'a mut RAM<'a>, // & means reference
}

impl<'a> CPU<'a> {
    pub fn new(memory: &'a mut RAM<'a>) -> CPU<'a> {
        CPU {
            program_counter: 0,
            accumulator: 0,
            memory: memory,
        }
    }
    // self is CPU object we execute functiion on
    pub fn tick(&mut self) {
        let opcode = self.memory.read(self.program_counter); //it creates opcode variable then finds memory, reads what is written in it and writes it to opcode
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
            _ => {
                //_ means whatever else
                panic!("unknown opcode");
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
    fn lda_sta() {
        let mut memory = RAM {
            bytes: &mut [
                opcodes::LDA,
                65,
                opcodes::STA,
                12,
                opcodes::LDA,
                73,
                opcodes::STA,
                12,
                opcodes::LDA,
                12,
                opcodes::STA,
                13,
                0,
                0,
            ],
        };
        let mut cpu = CPU::new(&mut memory);
        cpu.tick();
        cpu.tick();
        assert_eq!(cpu.memory.bytes[12..14], [65, 0]);
        cpu.tick();
        cpu.tick();
        assert_eq!(cpu.memory.bytes[12..14], [73, 0]);
        cpu.tick();
        cpu.tick();
        assert_eq!(cpu.memory.bytes[12..14], [73, 12]);
    }
}
