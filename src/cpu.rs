use crate::memory::Memory;
use rand::Rng;
use std::fmt::Debug;

#[derive(Debug)]
enum SequenceState {
    Reset(u32),
    Ready,
    Opcode(u8, u32),
}

#[derive(Debug)]
pub struct CPU<'a, M: Memory> {
    memory: &'a mut M,

    // Registers.
    program_counter: u16,
    accumulator: u8,
    xreg: u8,
    yreg: u8,
    stack_pointer: u8,
    flags: u8,

    // Other internal state.

    // Number of cycle within execution of the current instruction.
    sequence_state: SequenceState,
    adl: u8,
}

impl<'a, M: Memory + Debug> CPU<'a, M> {
    /// Creates a new `CPU` that owns given `memory`. The newly created `CPU` is
    /// not yet ready for executing programs; it first needs to be reset using
    /// the [`reset`](#method.reset) method.
    pub fn new(memory: &'a mut M) -> Self {
        let mut rng = rand::thread_rng();
        CPU {
            memory: memory,

            program_counter: rng.gen(),
            accumulator: rng.gen(),
            xreg: rng.gen(),
            yreg: rng.gen(),
            stack_pointer: rng.gen(),
            flags: rng.gen(),

            sequence_state: SequenceState::Reset(0),
            // adh: rng.gen(),
            adl: rng.gen(),
        }
    }

    pub fn memory(&mut self) -> &mut M {
        self.memory
    }

    /// Start the CPU reset sequence. It will last for the next 8 cycles. During
    /// initialization, the CPU reads an address from 0xFFFC and stores it in
    /// the `PC` register. The subsequent [`tick`](#method.tick) will
    /// effectively resume program from this address.
    pub fn reset(&mut self) {
        self.sequence_state = SequenceState::Reset(0);
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
                self.program_counter += 1;
            }

            // List ALL the opcodes!
            SequenceState::Opcode(opcodes::LDA, _) => {
                self.accumulator = self.memory.read(self.program_counter);
                self.program_counter += 1;
                self.sequence_state = SequenceState::Ready;
            }
            SequenceState::Opcode(opcodes::STA, subcycle) => match subcycle {
                1 => {
                    self.adl = self.memory.read(self.program_counter);
                    self.program_counter += 1;
                }
                _ => {
                    self.memory.write(self.adl as u16, self.accumulator);
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::LDX, _) => {
                self.xreg = self.memory.read(self.program_counter);
                self.program_counter += 1;
                self.sequence_state = SequenceState::Ready;
            }
            SequenceState::Opcode(opcodes::STX, subcycle) => match subcycle {
                1 => {
                    self.adl = self.memory.read(self.program_counter);
                    self.program_counter += 1;
                }
                _ => {
                    self.memory.write(self.adl as u16, self.xreg);
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::INX, _) => {
                self.memory.read(self.program_counter); // discard
                self.xreg = self.xreg.wrapping_add(1);
                self.sequence_state = SequenceState::Ready;
            }
            SequenceState::Opcode(opcodes::LDY, _) => {
                self.yreg = self.memory.read(self.program_counter);
                self.program_counter += 1;
                self.sequence_state = SequenceState::Ready;
            }
            SequenceState::Opcode(opcodes::STY, subcycle) => match subcycle {
                1 => {
                    self.adl = self.memory.read(self.program_counter);
                    self.program_counter += 1;
                }
                _ => {
                    self.memory.write(self.adl as u16, self.yreg);
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::INY, _) => {
                self.memory.read(self.program_counter); // discard
                self.yreg = self.yreg.wrapping_add(1);
                self.sequence_state = SequenceState::Ready;
            }
            SequenceState::Opcode(opcodes::TYA, _) => {
                self.memory.read(self.program_counter); // discard
                self.accumulator = self.yreg;
                self.sequence_state = SequenceState::Ready;
            }
            SequenceState::Opcode(opcodes::TAX, _) => {
                self.memory.read(self.program_counter); // discard
                self.xreg = self.accumulator;
                self.sequence_state = SequenceState::Ready;
            }
            SequenceState::Opcode(opcodes::TXS, _) => {
                self.memory.read(self.program_counter); // discard
                self.stack_pointer = self.xreg;
                self.sequence_state = SequenceState::Ready;
            }
            SequenceState::Opcode(opcodes::PHP, subcycle) => match subcycle {
                1 => {
                    self.memory.read(self.program_counter); // discard
                }
                _ => {
                    self.memory
                        .write(0x100 | self.stack_pointer as u16, self.flags);
                    self.stack_pointer -= 1;
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::PLP, subcycle) => match subcycle {
                1 => {
                    self.memory.read(self.program_counter); // discard
                }
                2 => {
                    self.memory.read(0x100 | self.stack_pointer as u16); // discard
                    self.stack_pointer += 1;
                }
                _ => {
                    self.flags =
                        self.memory.read(0x100 | self.stack_pointer as u16) | flags::UNUSED;
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::SEI, _) => {
                self.memory.read(self.program_counter); // discard
                self.flags |= flags::I;
                self.sequence_state = SequenceState::Ready;
            }
            SequenceState::Opcode(opcodes::CLI, _) => {
                self.memory.read(self.program_counter); // discard
                self.flags &= !flags::I;
                self.sequence_state = SequenceState::Ready;
            }
            SequenceState::Opcode(opcodes::JMP, subcycle) => match subcycle {
                1 => {
                    self.adl = self.memory.read(self.program_counter);
                    self.program_counter += 1;
                }
                _ => {
                    let adh = self.memory.read(self.program_counter);
                    self.program_counter = (self.adl as u16) | ((adh as u16) << 8);
                    self.sequence_state = SequenceState::Ready;
                }
            },

            // Oh no, we don't support it! (Yet.)
            SequenceState::Opcode(other_opcode, _) => {
                println!("{:X?}", &self);
                panic!(
                    "unknown opcode: ${:02X} at ${:04X}",
                    other_opcode,
                    self.program_counter - 1,
                );
            }

            // Reset sequence. First 6 cycles are idle, the initialization
            // procedure starts after that.
            SequenceState::Reset(0) => {
                self.flags |= flags::UNUSED | flags::I;
            }
            SequenceState::Reset(1..=5) => {}
            SequenceState::Reset(6) => {
                self.program_counter = self.memory.read(0xFFFC) as u16;
            }
            SequenceState::Reset(7) => {
                self.program_counter |= (self.memory.read(0xFFFD) as u16) << 8;
                self.sequence_state = SequenceState::Ready;
            }
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
    pub const LDA: u8 = 0xa9;
    pub const STA: u8 = 0x85;
    pub const LDX: u8 = 0xa2;
    pub const STX: u8 = 0x86;
    pub const INX: u8 = 0xe8;
    pub const LDY: u8 = 0xA0;
    pub const STY: u8 = 0x8C;
    pub const INY: u8 = 0xC8;
    pub const TYA: u8 = 0x98;
    pub const TAX: u8 = 0xAA;
    pub const TXS: u8 = 0x9A;
    pub const PHP: u8 = 0x08;
    pub const PLP: u8 = 0x28;
    pub const SEI: u8 = 0x78;
    pub const CLI: u8 = 0x58;
    pub const JMP: u8 = 0x4c;
}

mod flags {
    // pub const N: u8 = 1 << 7;
    // pub const V: u8 = 1 << 6;
    pub const UNUSED: u8 = 1 << 5;
    // pub const B: u8 = 1 << 4;
    // pub const D: u8 = 1 << 3;
    pub const I: u8 = 1 << 2;
    // pub const Z: u8 = 1 << 1;
    // pub const C: u8 = 1;
}

#[cfg(test)]
mod tests {
    extern crate test;

    use super::*;
    use crate::memory::RAM;
    use test::Bencher;

    fn reset<M: Memory + Debug>(cpu: &mut CPU<M>) {
        cpu.reset();
        cpu.ticks(8);
    }

    #[test]
    fn it_resets() {
        // We test resetting the CPU by providing a memory image with two
        // separate programs. The first starts, as usually, at 0xF000, and it
        // will store a value of 1 at 0x0000.
        let mut program = vec![opcodes::LDX, 1, opcodes::STX, 0, opcodes::TXS, opcodes::PHP];
        // The next one will start exactly 0x101 bytes later, at 0xF101. This is
        // because we want to change both bytes of the program's address. We
        // resize the memory so that it contains zeros until 0xF101.
        program.resize(0x101, 0);
        // Finally, the second program. It stores 2 at 0x0000.
        program.extend_from_slice(&[opcodes::LDX, 2, opcodes::STX, 0]);

        let mut memory = RAM::with_test_program(&program);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(10);
        assert_eq!(cpu.memory.bytes[0], 1, "the first program wasn't executed");
        assert_eq!(
            cpu.memory.bytes[0x101] & (flags::UNUSED | flags::I),
            flags::UNUSED | flags::I,
            "I and UNUSED flags are not set by default"
        );

        cpu.memory.bytes[0xFFFC] = 0x01;
        cpu.memory.bytes[0xFFFD] = 0xF1;
        reset(&mut cpu);
        cpu.ticks(5);
        assert_eq!(memory.bytes[0], 2, "the second program wasn't executed");
    }

    #[test]
    fn inx() {
        let mut memory = RAM::with_test_program(&mut [
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
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(17);
        assert_eq!(cpu.memory.bytes[5..8], [0xFF, 0x00, 0x01]);
    }

    #[test]
    fn iny() {
        let mut memory = RAM::with_test_program(&mut [
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
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(17);
        assert_eq!(cpu.memory.bytes[5..8], [0xFF, 0x00, 0x01]);
    }

    #[test]
    fn ldx_stx() {
        let mut memory = RAM::with_test_program(&mut [
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
        reset(&mut cpu);
        cpu.ticks(5);
        assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
        cpu.ticks(5);
        assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
        cpu.ticks(5);
        assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    }

    #[test]
    fn ldy_sty() {
        let mut memory = RAM::with_test_program(&mut [
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
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(5);
        assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
        cpu.ticks(5);
        assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
        cpu.ticks(5);
        assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    }

    #[test]
    fn lda_sta() {
        let mut memory = RAM::with_test_program(&mut [
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
        reset(&mut cpu);
        cpu.ticks(5);
        assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
        cpu.ticks(5);
        assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
        cpu.ticks(5);
        assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    }

    #[test]
    fn multiple_registers() {
        let mut memory = RAM::with_test_program(&mut [
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
        reset(&mut cpu);
        cpu.ticks(10);
        assert_eq!(cpu.memory.bytes[0..2], [10, 20]);
    }

    #[test]
    fn jmp_working() {
        let mut memory = RAM::with_test_program(&mut [
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
        reset(&mut cpu);
        cpu.ticks(13);
        assert_eq!(cpu.memory.bytes[9], 2);
        cpu.ticks(8);
        assert_eq!(cpu.memory.bytes[9], 3);
    }

    #[test]
    fn tya() {
        let mut memory =
            RAM::with_test_program(&mut [opcodes::LDY, 15, opcodes::TYA, opcodes::STA, 0x01]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(7);
        assert_eq!(cpu.memory.bytes[0x01], 15);
    }

    #[test]
    fn tax() {
        let mut memory =
            RAM::with_test_program(&mut [opcodes::LDA, 13, opcodes::TAX, opcodes::STX, 0x01]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(7);
        assert_eq!(cpu.memory.bytes[0x01], 13);
    }

    #[test]
    fn flag_manipulation() {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDX,
            0xFE,
            opcodes::TXS,
            opcodes::PLP,
            opcodes::SEI,
            opcodes::PHP,
            opcodes::CLI,
            opcodes::PHP,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(18);
        println!("{:X?}", &cpu);
        println!("{:X?}", &cpu.memory.bytes[0x100..0x200]);
        assert_eq!(
            cpu.memory.bytes[0x1FE..0x200],
            [flags::UNUSED, flags::I | flags::UNUSED]
        );
    }

    #[bench]
    fn benchmark(b: &mut Bencher) {
        let mut memory = RAM::with_test_program(&mut [
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
        b.iter(|| {
            reset(&mut cpu);
            cpu.ticks(1000);
        });
    }
}
