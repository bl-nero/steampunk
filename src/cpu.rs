use crate::memory::Memory;
use std::fmt::Debug;

#[derive(Debug)]
enum SequenceState {
    Reset(u32),
    Ready,
    Opcode(u8, u32),
}

#[derive(Debug)]
pub struct CPU<'a, M: Memory> {
    // The "outside interface", a.k.a. "significant pins".
    address_bus: u16,
    data_bus: u8,

    // Registers.
    program_counter: u16,
    accumulator: u8,
    xreg: u8,
    memory: &'a mut M,
    yreg: u8,

    // Other internal state.

    // Number of cycle within execution of the current instruction.
    sequence_state: SequenceState,
    adh: u8,
    adl: u8,
}

impl<'a, M: Memory + Debug> CPU<'a, M> {
    /// Creates a new `CPU` that owns given `memory`. The newly created `CPU` is
    /// not yet ready for executing programs; it first needs to be reset using
    /// the [`reset`](#method.reset) method.
    pub fn new(memory: &'a mut M) -> CPU<'a, M> {
        CPU {
            address_bus: 0,
            data_bus: 0,

            program_counter: 0,
            accumulator: 0,
            xreg: 0,
            memory: memory,
            yreg: 0,

            sequence_state: SequenceState::Reset(0),
            adh: 0,
            adl: 0,
        }
    }

    pub fn memory(&mut self) -> &mut M {
        self.memory
    }

    /// Reinitialize the CPU. It reads an address from 0xFFFA and stores it in
    /// the `PC` register. Next [`tick`](#method.tick) will effectively resume
    /// program from this address.
    pub fn reset(&mut self) {
        let lsb = self.memory.read(0xFFFA) as u16;
        let msb = self.memory.read(0xFFFB) as u16;
        self.program_counter = (msb << 8) | lsb;
        self.sequence_state = SequenceState::Ready;
    }

    /// Performs a single CPU cycle.
    pub fn tick(&mut self) {
        match self.sequence_state {
            // Fetching the opcode. A small trick: at first, we use 0 for
            // subcycle number, and it will later get increased to 1. Funny
            // thing, returning from here with subcycle set to 1 is slower than
            // waiting for 0 to be increased. Benchmarked!
            SequenceState::Ready => {
                self.sequence_state =
                    SequenceState::Opcode(self.memory.read(self.program_counter), 0);
            }

            // List ALL the opcodes!
            SequenceState::Opcode(opcodes::LDA, subcycle) => match subcycle {
                1 => {
                    self.accumulator = self.memory.read(self.program_counter + 1);
                    self.program_counter += 2;
                    self.sequence_state = SequenceState::Ready;
                }
                _ => self.sequence_state = SequenceState::Ready,
            },
            SequenceState::Opcode(opcodes::STA, subcycle) => match subcycle {
                1 => self.adl = self.memory.read(self.program_counter + 1),
                2 => {
                    self.memory.write(self.adl as u16, self.accumulator);
                    self.program_counter += 2;
                    self.sequence_state = SequenceState::Ready;
                }
                _ => self.sequence_state = SequenceState::Ready,
            },
            SequenceState::Opcode(opcodes::LDX, subcycle) => match subcycle {
                1 => {
                    self.xreg = self.memory.read(self.program_counter + 1);
                    self.program_counter += 2;
                    self.sequence_state = SequenceState::Ready;
                }
                _ => self.sequence_state = SequenceState::Ready,
            },
            SequenceState::Opcode(opcodes::STX, subcycle) => match subcycle {
                1 => self.adl = self.memory.read(self.program_counter + 1),
                2 => {
                    self.memory.write(self.adl as u16, self.xreg);
                    self.program_counter += 2;
                    self.sequence_state = SequenceState::Ready;
                }
                _ => self.sequence_state = SequenceState::Ready,
            },
            SequenceState::Opcode(opcodes::INX, subcycle) => {
                match subcycle {
                    1 => {
                        self.xreg = self.xreg.wrapping_add(1);
                        self.program_counter += 1;
                        self.memory.read(self.program_counter); // discard
                        self.sequence_state = SequenceState::Ready;
                    }
                    _ => self.sequence_state = SequenceState::Ready,
                }
            }
            SequenceState::Opcode(opcodes::LDY, subcycle) => match subcycle {
                1 => {
                    self.yreg = self.memory.read(self.program_counter + 1);
                    self.program_counter += 2;
                    self.sequence_state = SequenceState::Ready;
                }
                _ => self.sequence_state = SequenceState::Ready,
            },
            SequenceState::Opcode(opcodes::INY, subcycle) => {
                match subcycle {
                    1 => {
                        self.yreg = self.yreg.wrapping_add(1);
                        self.program_counter += 1;
                        self.memory.read(self.program_counter); // discard
                        self.sequence_state = SequenceState::Ready;
                    }
                    _ => self.sequence_state = SequenceState::Ready,
                }
            }
            SequenceState::Opcode(opcodes::STY, subcycle) => match subcycle {
                1 => self.adl = self.memory.read(self.program_counter + 1),
                2 => {
                    self.memory.write(self.adl as u16, self.yreg);
                    self.program_counter += 2;
                    self.sequence_state = SequenceState::Ready;
                }
                _ => self.sequence_state = SequenceState::Ready,
            },
            SequenceState::Opcode(opcodes::JMP, subcycle) => match subcycle {
                1 => self.adl = self.memory.read(self.program_counter + 1),
                2 => {
                    self.adh = self.memory.read(self.program_counter + 2);
                    self.program_counter = (self.adl as u16) | ((self.adh as u16) << 8);
                    self.sequence_state = SequenceState::Ready;
                }
                _ => self.sequence_state = SequenceState::Ready,
            },
            SequenceState::Opcode(opcodes::TYA, subcycle) => {
                match subcycle {
                    1 => {
                        self.accumulator = self.yreg;
                        self.program_counter += 1;
                        self.memory.read(self.program_counter); // discard
                        self.sequence_state = SequenceState::Ready;
                    }
                    _ => self.sequence_state = SequenceState::Ready,
                }
            }
            SequenceState::Opcode(opcodes::TAX, subcycle) => {
                match subcycle {
                    1 => {
                        self.xreg = self.accumulator;
                        self.program_counter += 1;
                        self.memory.read(self.program_counter); // discard
                        self.sequence_state = SequenceState::Ready;
                    }
                    _ => self.sequence_state = SequenceState::Ready,
                }
            }

            // Oh no, we don't support it! (Yet.)
            SequenceState::Opcode(other_opcode, _) => {
                println!("{:X?}", &self);
                panic!(
                    "unknown opcode: ${:02X} at ${:04X}",
                    other_opcode, self.program_counter
                );
            }

            // Reset sequence. First 6 cycles are idle, the initialization
            // procedure starts after that.
            SequenceState::Reset(0..=4) => {}
            SequenceState::Reset(5) => self.address_bus = 0xFFFA,
            SequenceState::Reset(6) => {
                self.program_counter = self.data_bus as u16;
                self.address_bus = 0xFFFB;
            }
            SequenceState::Reset(7) => self.program_counter |= (self.data_bus as u16) << 8,
            SequenceState::Reset(unexpected_subcycle) => {
                panic!("Unexpected subcycle: {}", unexpected_subcycle);
            }
        }

        // Now move on to the next subcycle.
        match self.sequence_state {
            SequenceState::Opcode(opcode, subcycle) => {
                self.sequence_state = SequenceState::Opcode(opcode, subcycle + 1)
            }
            SequenceState::Reset(subcycle) => {
                self.sequence_state = SequenceState::Reset(subcycle + 1)
            }
            _ => {}
        }
    }

    pub fn ticks(&mut self, n_ticks: u32) {
        for _ in 0..n_ticks {
            self.tick();
        }
    }
}

mod opcodes {
    //opcodes are instruction in program codes
    pub const LDA: u8 = 0xa9; //0x means hexadecimal number
    pub const STA: u8 = 0x85;
    pub const LDX: u8 = 0xa2;
    pub const STX: u8 = 0x86;
    pub const INX: u8 = 0xe8;
    pub const JMP: u8 = 0x4c;
    pub const INY: u8 = 0xC8;
    pub const STY: u8 = 0x8C;
    pub const LDY: u8 = 0xA0;
    pub const TYA: u8 = 0x98;
    pub const TAX: u8 = 0xAA;
}

#[cfg(test)]
mod tests {
    extern crate test;

    use super::*;
    use crate::memory::RAM;
    use test::Bencher;

    struct TestDevice<'a> {
        cpu: CPU<'a, RAM>,
    }

    impl<'a> TestDevice<'a> {
        fn new(memory: &'a mut RAM) -> Self {
            TestDevice {
                cpu: CPU::new(memory),
            }
        }

        fn ticks(&mut self, n: u32) {
            self.cpu.ticks(n);
        }

        fn reset(&mut self) {
            self.cpu.reset();
        }

        pub fn memory(&mut self) -> &mut RAM {
            self.cpu.memory()
        }
    }

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
        let mut dev = TestDevice::new(&mut memory);
        dev.reset();
        dev.ticks(5);
        assert_eq!(dev.memory().bytes[0], 1); // The first program has been executed.

        dev.memory().bytes[0xFFFA] = 0x01;
        dev.memory().bytes[0xFFFB] = 0xF1;
        dev.reset();
        dev.ticks(5);
        assert_eq!(memory.bytes[0], 2); // The second program has been executed.
    }

    // #[test]
    // fn it_resets_in_the_middle_of_instruction_processing() {
    //     let mut memory = RAM::with_program(&mut [
    //         opcodes::LDA,
    //         12,
    //         opcodes::STA,
    //         3
    //     ]);
    //     let mut cpu = CPU::new(&mut memory);
    //     cpu.reset();
    //     cpu.ticks(5);
    //     assert_eq!(cpu.memory.bytes[3], 12);
    // }

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
            opcodes::INX,
            opcodes::STX,
            7,
        ]);
        let mut dev = TestDevice::new(&mut memory);
        dev.reset();
        dev.ticks(17);
        assert_eq!(dev.memory().bytes[5..8], [0xFF, 0x00, 0x01]);
    }

    #[test]
    fn iny() {
        let mut memory = RAM::with_program(&mut [
            opcodes::LDY,
            0xFE,
            opcodes::INY,
            opcodes::STY,
            5,
            opcodes::INY,
            opcodes::STY,
            6,
            opcodes::INY,
            opcodes::STY,
            7,
        ]);
        let mut dev = TestDevice::new(&mut memory);
        dev.reset();
        dev.ticks(17);
        assert_eq!(dev.memory().bytes[5..8], [0xFF, 0x00, 0x01]);
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
        let mut dev = TestDevice::new(&mut memory);
        dev.reset();
        dev.ticks(5);
        assert_eq!(dev.memory().bytes[4..6], [65, 0]);
        dev.ticks(5);
        assert_eq!(dev.memory().bytes[4..6], [73, 0]);
        dev.ticks(5);
        assert_eq!(dev.memory().bytes[4..6], [73, 12]);
    }

    #[test]
    fn ldy_sty() {
        let mut memory = RAM::with_program(&mut [
            opcodes::LDY,
            65,
            opcodes::STY,
            4,
            opcodes::LDY,
            73,
            opcodes::STY,
            4,
            opcodes::LDY,
            12,
            opcodes::STY,
            5,
        ]);
        let mut dev = TestDevice::new(&mut memory);
        dev.reset();
        dev.ticks(5);
        assert_eq!(dev.memory().bytes[4..6], [65, 0]);
        dev.ticks(5);
        assert_eq!(dev.memory().bytes[4..6], [73, 0]);
        dev.ticks(5);
        assert_eq!(dev.memory().bytes[4..6], [73, 12]);
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
        let mut dev = TestDevice::new(&mut memory);
        dev.reset();
        dev.ticks(5);
        assert_eq!(dev.memory().bytes[4..6], [65, 0]);
        dev.ticks(5);
        assert_eq!(dev.memory().bytes[4..6], [73, 0]);
        dev.ticks(5);
        assert_eq!(dev.memory().bytes[4..6], [73, 12]);
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
        let mut dev = TestDevice::new(&mut memory);
        dev.reset();
        dev.ticks(10);
        assert_eq!(dev.memory().bytes[0..2], [10, 20]);
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
        let mut dev = TestDevice::new(&mut memory);
        dev.reset();
        dev.ticks(13);
        assert_eq!(dev.memory().bytes[9], 2);
        dev.ticks(8);
        assert_eq!(dev.memory().bytes[9], 3);
    }

    #[test]
    fn tya() {
        let mut memory =
            RAM::with_program(&mut [opcodes::LDY, 15, opcodes::TYA, opcodes::STA, 0x01]);
        let mut dev = TestDevice::new(&mut memory);
        dev.reset();
        dev.ticks(7);
        assert_eq!(dev.memory().bytes[0x01], 15);
    }

    #[test]
    fn tax() {
        let mut memory =
            RAM::with_program(&mut [opcodes::LDA, 13, opcodes::TAX, opcodes::STX, 0x01]);
        let mut dev = TestDevice::new(&mut memory);
        dev.reset();
        dev.ticks(7);
        assert_eq!(dev.memory().bytes[0x01], 13);
    }

    #[bench]
    fn benchmark(b: &mut Bencher) {
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
        let mut dev = TestDevice::new(&mut memory);
        b.iter(|| {
            dev.reset();
            dev.ticks(1000);
        });
    }
}
