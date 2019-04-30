#[derive(Debug)] //this generates function that translates CPU to text
struct CPU<'a> {
    program_counter: u16, // u means unsigned and 16 means it is 16 bit
    accumulator: u8,
    memory: &'a mut RAM, // & means reference
}

impl<'a> CPU<'a> {
    // self is CPU object we execute functiion on
    fn tick(&mut self) {
        let opcode = self.memory.read(self.program_counter); //it creates opcode variable then finds memory, reads what is written in it and writes it to opcode
        match opcode {
            opcodes::LDA => {
                self.accumulator = self.memory.read(self.program_counter + 1);
                self.program_counter = self.program_counter + 2;
            }
            opcodes::STA => {
                let address = self.memory.read(self.program_counter + 1);
                self.memory.write(address as u16, self.accumulator);
            }
            _ => {
                //_ means whatever else
                panic!("unknown opcode");
            }
        }
    }
}

#[derive(Debug)]
struct RAM {
    bytes: [u8; 10], //this means that computer has 25 u8's
}

impl RAM {
    fn read(&self, address: u16) -> u8 {
        // this arrow means we give u16 they return u8
        self.bytes[address as usize]
    }
    fn write(&mut self, address: u16, value: u8) {
        self.bytes[address as usize] = value;
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
    fn loads_accumulator() {
        let mut memory = RAM {
            bytes: [opcodes::LDA, 6, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        let mut cpu = CPU {
            program_counter: 0,
            accumulator: 0,
            memory: &mut memory,
        };
        cpu.tick();
        assert_eq!(cpu.accumulator, 6);
    }
    #[test]
    fn stores_accumulator() {
        let mut memory = RAM {
            bytes: [opcodes::STA, 4, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        let mut cpu = CPU {
            program_counter: 0,
            accumulator: 100,
            memory: &mut memory,
        };
        cpu.tick();
        assert_eq!(memory.bytes[4], 100);

        let mut memory = RAM {
            bytes: [opcodes::STA, 4, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        let mut cpu = CPU {
            program_counter: 0,
            accumulator: 50,
            memory: &mut memory,
        };
        cpu.tick();
        assert_eq!(memory.bytes[4], 50);

        let mut memory = RAM {
            bytes: [opcodes::STA, 5, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        let mut cpu = CPU {
            program_counter: 0,
            accumulator: 199,
            memory: &mut memory,
        };
        cpu.tick();
        assert_eq!(memory.bytes[5], 199);
    }
    #[test]
    fn lda_sta() {
        let mut memory = RAM {
            bytes: [opcodes::LDA, 65, opcodes::STA, 6, 0, 0, 0, 0, 0, 0],
        };
        let mut cpu = CPU {
            program_counter: 0,
            accumulator: 0,
            memory: &mut memory,
        };
        cpu.tick();
        cpu.tick();
        println!("{:?}", memory);
        assert_eq!(memory.bytes[6], 65);
    }
}

fn main() {
    println!("Welcome player ONE!");
}
