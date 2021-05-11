use crate::memory::{Memory, ReadError};
use rand::Rng;
use std::error;
use std::fmt;
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
    reg_pc: u16,
    reg_a: u8,
    reg_x: u8,
    reg_y: u8,
    reg_sp: u8,
    flags: u8,

    // Other internal state.

    // Number of cycle within execution of the current instruction.
    sequence_state: SequenceState,
    adl: u8,
    bal: u8,
}

type TickResult = Result<(), Box<dyn error::Error>>;

// enum CpuError {
//     ReadError,
//     WriteError,
// }

#[derive(Debug, Clone)]
struct UnknownOpcodeError {
    opcode: u8,
    address: u16,
}

impl error::Error for UnknownOpcodeError {}

impl fmt::Display for UnknownOpcodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Unknown opcode: ${:02X} at ${:04X}",
            self.opcode, self.address
        )
    }
}

// impl From<ReadError> for CpuError {
//     fn from(err: ReadError) -> Self {
//         CpuError::ReadError(err)
//     }
// }

impl<'a, M: Memory + Debug> CPU<'a, M> {
    /// Creates a new `CPU` that owns given `memory`. The newly created `CPU` is
    /// not yet ready for executing programs; it first needs to be reset using
    /// the [`reset`](#method.reset) method.
    pub fn new(memory: &'a mut M) -> Self {
        let mut rng = rand::thread_rng();
        CPU {
            memory: memory,

            reg_pc: rng.gen(),
            reg_a: rng.gen(),
            reg_x: rng.gen(),
            reg_y: rng.gen(),
            reg_sp: rng.gen(),
            flags: rng.gen(),

            sequence_state: SequenceState::Reset(0),
            // adh: rng.gen(),
            adl: rng.gen(),
            bal: rng.gen(),
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
    pub fn tick(&mut self) -> TickResult {
        match self.sequence_state {
            // Fetching the opcode. A small trick: at first, we use 0 for
            // subcycle number, and it will later get increased to 1. Funny
            // thing, returning from here with subcycle set to 1 is slower than
            // waiting for 0 to be increased. Benchmarked!
            SequenceState::Ready => {
                self.sequence_state = SequenceState::Opcode(self.memory.read(self.reg_pc)?, 0);
                self.reg_pc += 1;
            }

            // List ALL the opcodes!
            SequenceState::Opcode(opcodes::LDA_IMM, _) => {
                self.load_register_immediate(&mut |me, value| me.set_reg_a(value))?;
            }
            SequenceState::Opcode(opcodes::LDX_IMM, _) => {
                self.load_register_immediate(&mut |me, value| me.set_reg_x(value))?;
            }
            SequenceState::Opcode(opcodes::LDY_IMM, _) => {
                self.load_register_immediate(&mut |me, value| me.set_reg_y(value))?;
            }
            SequenceState::Opcode(opcodes::STA_ZP, _) => {
                self.store_zero_page(self.reg_a)?;
            }
            SequenceState::Opcode(opcodes::STA_ZP_X, subcycle) => match subcycle {
                1 => {
                    self.bal = self.memory.read(self.reg_pc)?;
                    self.reg_pc += 1;
                }
                2 => {
                    let _ = self.memory.read(self.bal as u16);
                }
                _ => {
                    self.memory
                        .write((self.bal + self.reg_x) as u16, self.reg_a)?;
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::STX_ZP, _) => {
                self.store_zero_page(self.reg_x)?;
            }
            SequenceState::Opcode(opcodes::STY_ZP, _) => {
                self.store_zero_page(self.reg_y)?;
            }
            SequenceState::Opcode(opcodes::INX, _) => {
                self.simple_internal_operation(&mut |me| me.set_reg_x(me.reg_x.wrapping_add(1)))?;
            }
            SequenceState::Opcode(opcodes::INY, _) => {
                self.simple_internal_operation(&mut |me| me.set_reg_y(me.reg_y.wrapping_add(1)))?;
            }
            SequenceState::Opcode(opcodes::DEX, _) => {
                self.simple_internal_operation(&mut |me| me.set_reg_x(me.reg_x.wrapping_sub(1)))?;
            }
            SequenceState::Opcode(opcodes::DEY, _) => {
                self.simple_internal_operation(&mut |me| me.set_reg_y(me.reg_y.wrapping_sub(1)))?;
            }
            SequenceState::Opcode(opcodes::TYA, _) => {
                self.simple_internal_operation(&mut |me| me.reg_a = me.reg_y)?;
            }
            SequenceState::Opcode(opcodes::TAX, _) => {
                self.simple_internal_operation(&mut |me| me.reg_x = me.reg_a)?;
            }
            SequenceState::Opcode(opcodes::TXA, _) => {
                self.simple_internal_operation(&mut |me| me.reg_a = me.reg_x)?;
            }
            SequenceState::Opcode(opcodes::TXS, _) => {
                self.simple_internal_operation(&mut |me| me.reg_sp = me.reg_x)?;
            }
            SequenceState::Opcode(opcodes::PHP, subcycle) => match subcycle {
                1 => {
                    let _ = self.memory.read(self.reg_pc);
                }
                _ => {
                    self.memory.write(0x100 | self.reg_sp as u16, self.flags)?;
                    self.reg_sp -= 1;
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::PLP, subcycle) => match subcycle {
                1 => {
                    let _ = self.memory.read(self.reg_pc);
                }
                2 => {
                    let _ = self.memory.read(0x100 | self.reg_sp as u16);
                    self.reg_sp += 1;
                }
                _ => {
                    self.flags = self.memory.read(0x100 | self.reg_sp as u16)? | flags::UNUSED;
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::SEI, _) => {
                self.simple_internal_operation(&mut |me| me.flags |= flags::I)?;
            }
            SequenceState::Opcode(opcodes::CLI, _) => {
                self.simple_internal_operation(&mut |me| me.flags &= !flags::I)?;
            }
            SequenceState::Opcode(opcodes::CLD, _) => {
                self.simple_internal_operation(&mut |me| me.flags &= !flags::D)?;
            }
            SequenceState::Opcode(opcodes::JMP_ABS, subcycle) => match subcycle {
                1 => {
                    self.adl = self.memory.read(self.reg_pc)?;
                    self.reg_pc += 1;
                }
                _ => {
                    let adh = self.memory.read(self.reg_pc)?;
                    self.reg_pc = (self.adl as u16) | ((adh as u16) << 8);
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::BNE, subcycle) => match subcycle {
                // TODO: handle additional cycle when crossing page boundaries
                1 => {
                    self.adl = self.memory.read(self.reg_pc)?;
                    self.reg_pc += 1;
                    if self.flags & flags::Z != 0 {
                        self.sequence_state = SequenceState::Ready;
                    }
                }
                _ => {
                    self.reg_pc = self.reg_pc.wrapping_add(self.adl as i8 as i16 as u16);
                    let _ = self.memory.read(self.reg_pc);
                    self.sequence_state = SequenceState::Ready;
                }
            },

            // Oh no, we don't support it! (Yet.)
            SequenceState::Opcode(other_opcode, _) => {
                return Err(Box::new(UnknownOpcodeError {
                    opcode: other_opcode,
                    address: self.reg_pc - 1,
                }));
            }

            // Reset sequence. First 6 cycles are idle, the initialization
            // procedure starts after that.
            SequenceState::Reset(0) => {
                self.flags |= flags::UNUSED | flags::I;
            }
            SequenceState::Reset(1..=5) => {}
            SequenceState::Reset(6) => {
                self.reg_pc = self.memory.read(0xFFFC)? as u16;
            }
            SequenceState::Reset(7) => {
                self.reg_pc |= (self.memory.read(0xFFFD)? as u16) << 8;
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
        };
        Ok(())
    }

    fn set_reg_a(&mut self, value: u8) {
        self.reg_a = value;
        let flag_z = if value == 0 { flags::Z } else { 0 };
        let flag_n = if value & 0b1000_0000 != 0 {
            flags::N
        } else {
            0
        };
        self.flags = (self.flags & !(flags::Z | flags::N)) | flag_z | flag_n;
    }

    fn set_reg_x(&mut self, value: u8) {
        self.reg_x = value;
        let flag_z = if value == 0 { flags::Z } else { 0 };
        let flag_n = if value & 0b1000_0000 != 0 {
            flags::N
        } else {
            0
        };
        self.flags = (self.flags & !(flags::Z | flags::N)) | flag_z | flag_n;
    }

    fn set_reg_y(&mut self, value: u8) {
        self.reg_y = value;
        let flag_z = if value == 0 { flags::Z } else { 0 };
        let flag_n = if value & 0b1000_0000 != 0 {
            flags::N
        } else {
            0
        };
        self.flags = (self.flags & !(flags::Z | flags::N)) | flag_z | flag_n;
    }

    fn load_register_immediate(
        &mut self,
        load: &mut dyn FnMut(&mut Self, u8),
    ) -> Result<(), ReadError> {
        self.memory.read(self.reg_pc).map(|value| {
            load(self, value);
            self.reg_pc += 1;
            self.sequence_state = SequenceState::Ready;
        })
    }

    fn store_zero_page(&mut self, value: u8) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => {
                self.adl = self.memory.read(self.reg_pc)?;
                self.reg_pc += 1;
            }
            _ => {
                self.memory.write(self.adl as u16, value)?;
                self.sequence_state = SequenceState::Ready;
            }
        };
        Ok(())
    }

    fn simple_internal_operation(
        &mut self,
        operation: &mut dyn FnMut(&mut Self),
    ) -> Result<(), ReadError> {
        let _ = self.memory.read(self.reg_pc);
        operation(self);
        self.sequence_state = SequenceState::Ready;
        Ok(())
    }

    pub fn ticks(&mut self, n_ticks: u32) -> TickResult {
        for _ in 0..n_ticks {
            self.tick()?;
        }
        Ok(())
    }
}

impl<'a, M: Memory> fmt::Display for CPU<'a, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "A  X  Y  SP PC   NV-BDIZC\n\
            {:02X} {:02X} {:02X} {:02X} {:04X} {}",
            self.reg_a, self.reg_x, self.reg_y, self.reg_sp, self.reg_pc, flags_to_string(self.flags)
        )
    }
}

fn flags_to_string(flags: u8) -> String {
    format!("{:08b}", flags)
        .chars()
        .map(|ch| match ch {
            '0' => '.',
            '1' => '*',
            _ => ch,
        })
        .collect()
}

mod opcodes {
    pub const LDA_IMM: u8 = 0xA9;
    pub const STA_ZP: u8 = 0x85;
    pub const STA_ZP_X: u8 = 0x95;
    pub const LDX_IMM: u8 = 0xA2;
    pub const STX_ZP: u8 = 0x86;
    pub const INX: u8 = 0xE8;
    pub const DEX: u8 = 0xCA;
    pub const LDY_IMM: u8 = 0xA0;
    pub const STY_ZP: u8 = 0x8C;
    pub const INY: u8 = 0xC8;
    pub const DEY: u8 = 0x88;
    pub const TYA: u8 = 0x98;
    pub const TAX: u8 = 0xAA;
    pub const TXA: u8 = 0x8A;
    pub const TXS: u8 = 0x9A;
    pub const PHP: u8 = 0x08;
    pub const PLP: u8 = 0x28;
    pub const SEI: u8 = 0x78;
    pub const CLI: u8 = 0x58;
    // pub const SED: u8 = 0xF8;
    pub const CLD: u8 = 0xD8;
    pub const JMP_ABS: u8 = 0x4C;
    pub const BNE: u8 = 0xD0;
}

mod flags {
    pub const N: u8 = 1 << 7;
    // pub const V: u8 = 1 << 6;
    pub const UNUSED: u8 = 1 << 5;
    // pub const B: u8 = 1 << 4;
    pub const D: u8 = 1 << 3;
    pub const I: u8 = 1 << 2;
    pub const Z: u8 = 1 << 1;
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
        cpu.ticks(8).unwrap();
    }

    #[test]
    fn it_resets() {
        // We test resetting the CPU by providing a memory image with two
        // separate programs. The first starts, as usually, at 0xF000, and it
        // will store a value of 1 at 0x0000.
        let mut program = vec![
            opcodes::LDX_IMM,
            1,
            opcodes::STX_ZP,
            0,
            opcodes::TXS,
            opcodes::PHP,
        ];
        // The next one will start exactly 0x101 bytes later, at 0xF101. This is
        // because we want to change both bytes of the program's address. We
        // resize the memory so that it contains zeros until 0xF101.
        program.resize(0x101, 0);
        // Finally, the second program. It stores 2 at 0x0000.
        program.extend_from_slice(&[opcodes::LDX_IMM, 2, opcodes::STX_ZP, 0]);

        let mut memory = RAM::with_test_program(&program);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(10).unwrap();
        assert_eq!(cpu.memory.bytes[0], 1, "the first program wasn't executed");
        assert_eq!(
            cpu.memory.bytes[0x101] & (flags::UNUSED | flags::I),
            flags::UNUSED | flags::I,
            "I and UNUSED flags are not set by default"
        );

        cpu.memory.bytes[0xFFFC] = 0x01;
        cpu.memory.bytes[0xFFFD] = 0xF1;
        reset(&mut cpu);
        cpu.ticks(5).unwrap();
        assert_eq!(memory.bytes[0], 2, "the second program wasn't executed");
    }

    #[test]
    fn lda_sta() {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDA_IMM,
            65,
            opcodes::STA_ZP,
            4,
            opcodes::LDA_IMM,
            73,
            opcodes::STA_ZP,
            4,
            opcodes::LDA_IMM,
            12,
            opcodes::STA_ZP,
            5,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    }

    #[test]
    fn ldx_stx() {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDX_IMM,
            65,
            opcodes::STX_ZP,
            4,
            opcodes::LDX_IMM,
            73,
            opcodes::STX_ZP,
            4,
            opcodes::LDX_IMM,
            12,
            opcodes::STX_ZP,
            5,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    }

    #[test]
    fn ldy_sty() {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDY_IMM,
            65,
            opcodes::STY_ZP,
            4,
            opcodes::LDY_IMM,
            73,
            opcodes::STY_ZP,
            4,
            opcodes::LDY_IMM,
            12,
            opcodes::STY_ZP,
            5,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    }

    #[test]
    fn multiple_registers() {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDA_IMM,
            10,
            opcodes::LDX_IMM,
            20,
            opcodes::STA_ZP,
            0,
            opcodes::STX_ZP,
            1,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(10).unwrap();
        assert_eq!(cpu.memory.bytes[0..2], [10, 20]);
    }

    #[test]
    fn loading_storing_addressing_modes() {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDX_IMM,
            5,
            opcodes::LDA_IMM,
            42,
            opcodes::STA_ZP_X,
            3,
            opcodes::INX,
            opcodes::JMP_ABS,
            0x04,
            0xF0,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(4 + 5 * 9).unwrap();
        assert_eq!(cpu.memory.bytes[8..13], [42, 42, 42, 42, 42]);
    }

    #[test]
    fn inx_dex() {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDX_IMM,
            0xFE,
            opcodes::INX,
            opcodes::STX_ZP,
            5,
            opcodes::INX,
            opcodes::STX_ZP,
            6,
            opcodes::INX,
            opcodes::STX_ZP,
            7,
            opcodes::DEX,
            opcodes::STX_ZP,
            8,
            opcodes::DEX,
            opcodes::STX_ZP,
            9,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(27).unwrap();
        assert_eq!(cpu.memory.bytes[5..10], [0xFF, 0x00, 0x01, 0x00, 0xFF]);
    }

    #[test]
    fn iny_dey() {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDY_IMM,
            0xFE,
            opcodes::INY,
            opcodes::STY_ZP,
            5,
            opcodes::INY,
            opcodes::STY_ZP,
            6,
            opcodes::INY,
            opcodes::STY_ZP,
            7,
            opcodes::DEY,
            opcodes::STY_ZP,
            8,
            opcodes::DEY,
            opcodes::STY_ZP,
            9,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(27).unwrap();
        assert_eq!(cpu.memory.bytes[5..10], [0xFF, 0x00, 0x01, 0x00, 0xFF]);
    }

    #[test]
    fn tya() {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDY_IMM,
            15,
            opcodes::TYA,
            opcodes::STA_ZP,
            0x01,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(7).unwrap();
        assert_eq!(cpu.memory.bytes[0x01], 15);
    }

    #[test]
    fn tax() {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDA_IMM,
            13,
            opcodes::TAX,
            opcodes::STX_ZP,
            0x01,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(7).unwrap();
        assert_eq!(cpu.memory.bytes[0x01], 13);
    }

    #[test]
    fn txa() {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDX_IMM,
            43,
            opcodes::TXA,
            opcodes::STA_ZP,
            0x01,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(7).unwrap();
        assert_eq!(cpu.memory.bytes[0x01], 43);
    }

    #[test]
    fn flag_manipulation() {
        let mut memory = RAM::with_test_program(&mut [
            // Load 0 to flags and initialize SP.
            opcodes::LDX_IMM,
            0xFE,
            opcodes::TXS,
            opcodes::PLP,
            // Set I and Z.
            opcodes::SEI,
            opcodes::LDA_IMM,
            0,
            opcodes::PHP,
            // Clear Z, set N.
            opcodes::LDX_IMM,
            0xFF,
            opcodes::PHP,
            // Clear I, Z, and N.
            opcodes::CLI,
            opcodes::LDY_IMM,
            0x01,
            opcodes::PHP,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(27).unwrap();
        assert_eq!(
            cpu.memory.bytes[0x1FD..0x200],
            [
                flags::UNUSED,
                flags::I | flags::N | flags::UNUSED,
                flags::I | flags::Z | flags::UNUSED,
            ]
        );
    }

    #[test]
    fn jmp() {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDX_IMM,
            1,
            opcodes::STX_ZP,
            9,
            opcodes::INX,
            opcodes::JMP_ABS,
            0x02,
            0xf0,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(13).unwrap();
        assert_eq!(cpu.memory.bytes[9], 2);
        cpu.ticks(8).unwrap();
        assert_eq!(cpu.memory.bytes[9], 3);
    }

    #[test]
    fn bne() {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDX_IMM,
            5,
            opcodes::LDA_IMM,
            5,
            opcodes::STA_ZP_X,
            9,
            opcodes::DEX,
            opcodes::BNE,
            (-5i8) as u8,
            opcodes::STX_ZP,
            12,
        ]);
        let mut cpu = CPU::new(&mut memory);
        reset(&mut cpu);
        cpu.ticks(4 + 4 * 9 + 8 + 3).unwrap();
        assert_eq!(cpu.memory.bytes[9..16], [0, 5, 5, 0, 5, 5, 0]);
    }

    #[bench]
    fn benchmark(b: &mut Bencher) {
        let mut memory = RAM::with_test_program(&mut [
            opcodes::LDX_IMM,
            1,
            opcodes::LDA_IMM,
            42,
            opcodes::STA_ZP_X,
            0x00,
            opcodes::LDA_IMM,
            64,
            opcodes::STA_ZP_X,
            0x80,
            opcodes::INX,
            opcodes::JMP_ABS,
            0x02,
            0xf0,
        ]);
        let mut cpu = CPU::new(&mut memory);
        b.iter(|| {
            reset(&mut cpu);
            cpu.ticks(1000).unwrap();
        });
    }
}
