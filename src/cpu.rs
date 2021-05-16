use crate::memory::{Memory, ReadError, ReadResult};
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
pub struct Cpu<M: Memory> {
    memory: Box<M>,

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

#[derive(Debug, Clone)]
struct CpuHaltedError {
    opcode: u8,
    address: u16,
}

impl error::Error for CpuHaltedError {}

impl fmt::Display for CpuHaltedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "CPU halted by opcode ${:02X} at ${:04X}",
            self.opcode, self.address
        )
    }
}

// impl From<ReadError> for CpuError {
//     fn from(err: ReadError) -> Self {
//         CpuError::ReadError(err)
//     }
// }

impl<M: Memory + Debug> Cpu<M> {
    /// Creates a new `CPU` that owns given `memory`. The newly created `CPU` is
    /// not yet ready for executing programs; it first needs to be reset using
    /// the [`reset`](#method.reset) method.
    pub fn new(memory: Box<M>) -> Self {
        let mut rng = rand::thread_rng();
        Cpu {
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
        &mut self.memory
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
                self.sequence_state = SequenceState::Opcode(self.consume_byte()?, 0);
            }

            // List ALL the opcodes!
            SequenceState::Opcode(opcodes::LDA_IMM, _) => {
                self.tick_load_register_immediate(&mut |me, value| me.set_reg_a(value))?;
            }
            SequenceState::Opcode(opcodes::LDX_IMM, _) => {
                self.tick_load_register_immediate(&mut |me, value| me.set_reg_x(value))?;
            }
            SequenceState::Opcode(opcodes::LDY_IMM, _) => {
                self.tick_load_register_immediate(&mut |me, value| me.set_reg_y(value))?;
            }
            SequenceState::Opcode(opcodes::STA_ZP, _) => {
                self.tick_store_zero_page(self.reg_a)?;
            }
            SequenceState::Opcode(opcodes::STA_ZP_X, _) => {
                self.tick_store_zero_page_x(self.reg_a)?;
            }
            SequenceState::Opcode(opcodes::STX_ZP, _) => {
                self.tick_store_zero_page(self.reg_x)?;
            }
            SequenceState::Opcode(opcodes::STY_ZP, _) => {
                self.tick_store_zero_page(self.reg_y)?;
            }
            SequenceState::Opcode(opcodes::STY_ZP_X, _) => {
                self.tick_store_zero_page_x(self.reg_y)?;
            }
            SequenceState::Opcode(opcodes::INX, _) => {
                self.tick_simple_internal_operation(&mut |me| {
                    me.set_reg_x(me.reg_x.wrapping_add(1))
                })?;
            }
            SequenceState::Opcode(opcodes::INY, _) => {
                self.tick_simple_internal_operation(&mut |me| {
                    me.set_reg_y(me.reg_y.wrapping_add(1))
                })?;
            }
            SequenceState::Opcode(opcodes::DEX, _) => {
                self.tick_simple_internal_operation(&mut |me| {
                    me.set_reg_x(me.reg_x.wrapping_sub(1))
                })?;
            }
            SequenceState::Opcode(opcodes::DEY, _) => {
                self.tick_simple_internal_operation(&mut |me| {
                    me.set_reg_y(me.reg_y.wrapping_sub(1))
                })?;
            }
            SequenceState::Opcode(opcodes::TYA, _) => {
                self.tick_simple_internal_operation(&mut |me| me.set_reg_a(me.reg_y))?;
            }
            SequenceState::Opcode(opcodes::TAX, _) => {
                self.tick_simple_internal_operation(&mut |me| me.set_reg_x(me.reg_a))?;
            }
            SequenceState::Opcode(opcodes::TXA, _) => {
                self.tick_simple_internal_operation(&mut |me| me.set_reg_a(me.reg_x))?;
            }
            SequenceState::Opcode(opcodes::TXS, _) => {
                self.tick_simple_internal_operation(&mut |me| me.reg_sp = me.reg_x)?;
            }
            SequenceState::Opcode(opcodes::TSX, _) => {
                self.tick_simple_internal_operation(&mut |me| me.set_reg_x(me.reg_sp))?;
            }
            SequenceState::Opcode(opcodes::PHP, _) => {
                self.tick_push(self.flags)?;
            }
            SequenceState::Opcode(opcodes::PLP, _) => {
                self.tick_pull(&mut |me, value| me.flags = value | flags::UNUSED)?;
            }
            SequenceState::Opcode(opcodes::PHA, _) => {
                self.tick_push(self.reg_a)?;
            }
            SequenceState::Opcode(opcodes::PLA, _) => {
                self.tick_pull(&mut |me, value| me.set_reg_a(value))?;
            }
            SequenceState::Opcode(opcodes::SEI, _) => {
                self.tick_simple_internal_operation(&mut |me| me.flags |= flags::I)?;
            }
            SequenceState::Opcode(opcodes::CLI, _) => {
                self.tick_simple_internal_operation(&mut |me| me.flags &= !flags::I)?;
            }
            SequenceState::Opcode(opcodes::CLD, _) => {
                self.tick_simple_internal_operation(&mut |me| me.flags &= !flags::D)?;
            }
            SequenceState::Opcode(opcodes::JMP_ABS, subcycle) => match subcycle {
                1 => self.adl = self.consume_byte()?,
                _ => {
                    let adh = self.memory.read(self.reg_pc)?;
                    self.reg_pc = (self.adl as u16) | ((adh as u16) << 8);
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::BNE, subcycle) => match subcycle {
                // TODO: handle additional cycle when crossing page boundaries
                1 => {
                    self.adl = self.consume_byte()?;
                    if self.flags & flags::Z != 0 {
                        self.sequence_state = SequenceState::Ready;
                    }
                }
                2 => {
                    let new_pc = self.reg_pc.wrapping_add(self.adl as i8 as u16);
                    if new_pc & 0xFF00 == self.reg_pc & 0xFF00 {
                        // No page boundary crossed. Do a phantom read of the
                        // computed address and skip the next cycle.
                        let _ = self.memory.read(self.reg_pc);
                        self.sequence_state = SequenceState::Ready;
                    } else {
                        let _ = self.memory.read((new_pc & 0x00FF) | (self.reg_pc & 0xFF00));
                        // Page boundary crossed. Do a phantom read of a
                        // partially computed address and continue to the next
                        // cycle.
                    }
                    self.reg_pc = new_pc;
                }
                _ => {
                    let _ = self.memory.read(self.reg_pc);
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::JSR, subcycle) => match subcycle {
                1 => self.adl = self.consume_byte()?,
                2 => {
                    let _ = self.memory.read(self.stack_pointer());
                }
                3 => {
                    self.memory
                        .write(self.stack_pointer(), (self.reg_pc >> 8) as u8)?;
                    self.reg_sp -= 1;
                }
                4 => {
                    self.memory.write(self.stack_pointer(), self.reg_pc as u8)?;
                    self.reg_sp -= 1;
                }
                _ => {
                    let adh = self.memory.read(self.reg_pc)?;
                    self.reg_pc = (self.adl as u16) | ((adh as u16) << 8);
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::RTS, subcycle) => match subcycle {
                1 => {
                    let _ = self.consume_byte();
                }
                2 => {
                    let _ = self.memory.read(self.stack_pointer());
                    self.reg_sp += 1;
                }
                3 => {
                    self.reg_pc =
                        self.reg_pc & 0xFF00 | self.memory.read(self.stack_pointer())? as u16;
                    self.reg_sp += 1;
                }
                4 => {
                    self.reg_pc =
                        self.reg_pc & 0xFF | ((self.memory.read(self.stack_pointer())? as u16) << 8)
                }
                _ => {
                    let _ = self.consume_byte();
                    self.sequence_state = SequenceState::Ready;
                }
            },

            // Unofficial opcodes
            SequenceState::Opcode(opcodes::HLT1, _) => {
                return Err(Box::new(CpuHaltedError {
                    opcode: opcodes::HLT1,
                    address: self.reg_pc.wrapping_sub(1),
                }));
            }

            // Oh no, we don't support it! (Yet.)
            SequenceState::Opcode(other_opcode, _) => {
                return Err(Box::new(UnknownOpcodeError {
                    opcode: other_opcode,
                    address: self.reg_pc.wrapping_sub(1),
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

    fn tick_load_register_immediate(
        &mut self,
        load: &mut dyn FnMut(&mut Self, u8),
    ) -> Result<(), ReadError> {
        let value = self.consume_byte()?;
        load(self, value);
        self.sequence_state = SequenceState::Ready;
        Ok(())
    }

    fn tick_store_zero_page(&mut self, value: u8) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.adl = self.consume_byte()?,
            _ => {
                self.memory.write(self.adl as u16, value)?;
                self.sequence_state = SequenceState::Ready;
            }
        };
        Ok(())
    }

    fn tick_store_zero_page_x(&mut self, value: u8) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.bal = self.consume_byte()?,
            SequenceState::Opcode(_, 2) => {
                let _ = self.memory.read(self.bal as u16);
            }
            _ => {
                self.memory.write((self.bal + self.reg_x) as u16, value)?;
                self.sequence_state = SequenceState::Ready;
            }
        };
        Ok(())
    }

    fn tick_simple_internal_operation(
        &mut self,
        operation: &mut dyn FnMut(&mut Self),
    ) -> Result<(), ReadError> {
        let _ = self.memory.read(self.reg_pc);
        operation(self);
        self.sequence_state = SequenceState::Ready;
        Ok(())
    }

    fn tick_push(&mut self, value: u8) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => {
                let _ = self.memory.read(self.reg_pc);
            }
            _ => {
                self.memory.write(self.stack_pointer(), value)?;
                self.reg_sp = self.reg_sp.wrapping_sub(1);
                self.sequence_state = SequenceState::Ready;
            }
        };
        Ok(())
    }

    fn tick_pull(&mut self, load: &mut dyn FnMut(&mut Self, u8)) -> Result<(), ReadError> {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => {
                let _ = self.memory.read(self.reg_pc);
            }
            SequenceState::Opcode(_, 2) => {
                let _ = self.memory.read(self.stack_pointer());
                self.reg_sp = self.reg_sp.wrapping_add(1);
            }
            _ => {
                load(self, self.memory.read(self.stack_pointer())?);
                self.sequence_state = SequenceState::Ready;
            }
        };
        Ok(())
    }

    fn consume_byte(&mut self) -> ReadResult {
        let result = self.memory.read(self.reg_pc)?;
        self.reg_pc = self.reg_pc.wrapping_add(1);
        return Ok(result);
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

    fn stack_pointer(&self) -> u16 {
        0x100 | self.reg_sp as u16
    }

    pub fn ticks(&mut self, n_ticks: u32) -> TickResult {
        for _ in 0..n_ticks {
            self.tick()?;
        }
        Ok(())
    }
}

impl<M: Memory> fmt::Display for Cpu<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "A  X  Y  SP PC   NV-BDIZC\n\
            {:02X} {:02X} {:02X} {:02X} {:04X} {}",
            self.reg_a,
            self.reg_x,
            self.reg_y,
            self.reg_sp,
            self.reg_pc,
            flags_to_string(self.flags)
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
    pub const STY_ZP_X: u8 = 0x94;
    pub const INY: u8 = 0xC8;
    pub const DEY: u8 = 0x88;
    pub const TYA: u8 = 0x98;
    pub const TAX: u8 = 0xAA;
    pub const TXA: u8 = 0x8A;
    pub const TXS: u8 = 0x9A;
    pub const TSX: u8 = 0xBA;
    pub const PHP: u8 = 0x08;
    pub const PLP: u8 = 0x28;
    pub const PHA: u8 = 0x48;
    pub const PLA: u8 = 0x68;
    pub const SEI: u8 = 0x78;
    pub const CLI: u8 = 0x58;
    // pub const SED: u8 = 0xF8;
    pub const CLD: u8 = 0xD8;
    pub const JMP_ABS: u8 = 0x4C;
    pub const BNE: u8 = 0xD0;
    pub const JSR: u8 = 0x20;
    pub const RTS: u8 = 0x60;

    pub const HLT1: u8 = 0x02;
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
    use crate::memory::SimpleRam;
    use test::Bencher;

    fn reset<M: Memory + Debug>(cpu: &mut Cpu<M>) {
        cpu.reset();
        cpu.ticks(8).unwrap();
    }

    fn cpu_with_program(program: &[u8]) -> Cpu<SimpleRam> {
        let memory = Box::new(SimpleRam::with_test_program(program));
        let mut cpu = Cpu::new(memory);
        reset(&mut cpu);
        return cpu;
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

        let mut cpu = cpu_with_program(&program);
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
        assert_eq!(cpu.memory.bytes[0], 2, "the second program wasn't executed");
    }

    #[test]
    fn lda_sta() {
        let mut cpu = cpu_with_program(&[
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
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    }

    #[test]
    fn ldx_stx() {
        let mut cpu = cpu_with_program(&[
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
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    }

    #[test]
    fn ldy_sty() {
        let mut cpu = cpu_with_program(&[
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
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
        cpu.ticks(5).unwrap();
        assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    }

    #[test]
    fn multiple_registers() {
        let mut cpu = cpu_with_program(&[
            opcodes::LDA_IMM,
            10,
            opcodes::LDX_IMM,
            20,
            opcodes::STA_ZP,
            0,
            opcodes::STX_ZP,
            1,
        ]);
        cpu.ticks(10).unwrap();
        assert_eq!(cpu.memory.bytes[0..2], [10, 20]);
    }

    #[test]
    fn loading_storing_addressing_modes() {
        let mut cpu = cpu_with_program(&[
            opcodes::LDX_IMM,
            5,
            opcodes::LDA_IMM,
            42,
            opcodes::LDY_IMM,
            100,
            opcodes::STA_ZP_X,
            3,
            opcodes::STY_ZP_X,
            9,
            opcodes::INX,
            opcodes::JMP_ABS,
            0x06,
            0xF0,
        ]);
        cpu.ticks(6 + 5 * 13).unwrap();
        assert_eq!(
            cpu.memory.bytes[8..19],
            [42, 42, 42, 42, 42, 0, 100, 100, 100, 100, 100]
        );
    }

    #[test]
    fn inx_dex() {
        let mut cpu = cpu_with_program(&[
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
        cpu.ticks(27).unwrap();
        assert_eq!(cpu.memory.bytes[5..10], [0xFF, 0x00, 0x01, 0x00, 0xFF]);
    }

    #[test]
    fn iny_dey() {
        let mut cpu = cpu_with_program(&[
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
        cpu.ticks(27).unwrap();
        assert_eq!(cpu.memory.bytes[5..10], [0xFF, 0x00, 0x01, 0x00, 0xFF]);
    }

    #[test]
    fn tya() {
        let mut cpu =
            cpu_with_program(&[opcodes::LDY_IMM, 15, opcodes::TYA, opcodes::STA_ZP, 0x01]);
        cpu.ticks(7).unwrap();
        assert_eq!(cpu.memory.bytes[0x01], 15);
    }

    #[test]
    fn tax() {
        let mut cpu =
            cpu_with_program(&[opcodes::LDA_IMM, 13, opcodes::TAX, opcodes::STX_ZP, 0x01]);
        cpu.ticks(7).unwrap();
        assert_eq!(cpu.memory.bytes[0x01], 13);
    }

    #[test]
    fn txa() {
        let mut cpu =
            cpu_with_program(&[opcodes::LDX_IMM, 43, opcodes::TXA, opcodes::STA_ZP, 0x01]);
        cpu.ticks(7).unwrap();
        assert_eq!(cpu.memory.bytes[0x01], 43);
    }

    #[test]
    fn flag_manipulation() {
        let mut cpu = cpu_with_program(&[
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
        let mut cpu = cpu_with_program(&[
            opcodes::LDX_IMM,
            1,
            opcodes::STX_ZP,
            9,
            opcodes::INX,
            opcodes::JMP_ABS,
            0x02,
            0xf0,
        ]);
        cpu.ticks(13).unwrap();
        assert_eq!(cpu.memory.bytes[9], 2);
        cpu.ticks(8).unwrap();
        assert_eq!(cpu.memory.bytes[9], 3);
    }

    #[test]
    fn bne() {
        let mut cpu = cpu_with_program(&[
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
        cpu.ticks(4 + 4 * 9 + 8 + 3).unwrap();
        assert_eq!(cpu.memory.bytes[9..16], [0, 5, 5, 0, 5, 5, 0]);
    }

    #[test]
    fn branching_across_pages_adds_one_cpu_cycle() {
        let memory = Box::new(SimpleRam::with_test_program_at(
            0xF0FB,
            &[
                opcodes::LDA_IMM,
                10,
                opcodes::BNE,
                1,
                opcodes::HLT1,
                opcodes::STA_ZP,
                20,
            ],
        ));
        let mut cpu = Cpu::new(memory);
        reset(&mut cpu);
        cpu.ticks(8).unwrap();
        assert_ne!(cpu.memory.bytes[20], 10);
        cpu.tick().unwrap();
        assert_eq!(cpu.memory.bytes[20], 10);
    }

    #[test]
    fn subroutines_and_stack() {
        let mut cpu = cpu_with_program(&[
            // Main program. Call subroutine A to store 6 at 25. Then call
            // subroutine B to store 7 at 28 and 6 at 26. Finally, store the 10
            // loaded to A in the beginning at 30. Duration: 25 cycles.
            opcodes::LDX_IMM,
            0xFF,
            opcodes::TXS,
            opcodes::LDA_IMM,
            10,
            opcodes::LDX_IMM,
            5,
            opcodes::JSR,
            0x11,
            0xF0,
            opcodes::INX,
            opcodes::JSR,
            0x19,
            0xF0,
            opcodes::STA_ZP,
            30,
            opcodes::HLT1,
            // Subroutine A: store 6 at 20+X. Address: $F011. Duration: 19
            // cycles.
            opcodes::PHA,
            opcodes::LDA_IMM,
            6,
            opcodes::STA_ZP_X,
            20,
            opcodes::PLA,
            opcodes::RTS,
            opcodes::HLT1,
            // Subroutine B: store 6 at 20+X and 7 at 22+X. Address: $F019.
            // Duration: 25 cycles.
            opcodes::PHA,
            opcodes::LDA_IMM,
            7,
            opcodes::JSR,
            0x11,
            0xF0,
            opcodes::STA_ZP_X,
            22,
            opcodes::PLA,
            opcodes::RTS,
            opcodes::HLT1,
        ]);
        cpu.ticks(25 + 19 + 25 + 19).unwrap();
        assert_eq!(cpu.memory.bytes[24..32], [0, 6, 6, 0, 7, 0, 10, 0]);
    }

    #[test]
    fn stack_wrapping() {
        let mut cpu = cpu_with_program(&[
            opcodes::LDX_IMM,
            1,
            opcodes::TXS,
            // ----
            opcodes::TXA,
            opcodes::PHA,
            opcodes::TSX,
            opcodes::TXA,
            opcodes::PHA,
            opcodes::TSX,
            opcodes::TXA,
            opcodes::PHA,
            opcodes::TSX,
            // ----
            opcodes::TXA,
            opcodes::PLA,
            opcodes::PLA,
            opcodes::PLA,
            opcodes::STA_ZP,
            5,
        ]);
        cpu.ticks(4 + 3 * 7 + 17).unwrap();
        assert_eq!(cpu.memory.bytes[0x1FF], 0xFF);
        assert_eq!(cpu.memory.bytes[0x100..0x102], [0, 1]);
        assert_eq!(cpu.memory.bytes[5], 1);
    }

    #[test]
    fn pc_wrapping() {
        let mut memory = Box::new(SimpleRam::with_test_program_at(
            0xFFF9,
            &[
                opcodes::JMP_ABS,
                0xFE,
                0xFF,
                0, // reset vector, will be filled
                0, // reset vector, will be filled
                opcodes::LDA_IMM,
                10,
            ],
        ));
        memory.bytes[0..2].copy_from_slice(&[opcodes::STA_ZP, 20]);
        let mut cpu = Cpu::new(memory);
        reset(&mut cpu);
        cpu.ticks(8).unwrap();
        assert_eq!(cpu.memory.bytes[20], 10);
    }

    #[test]
    fn pc_wrapping_during_branch() {
        let mut memory = Box::new(SimpleRam::with_test_program_at(
            0xFFF8,
            &[
                opcodes::LDA_IMM,
                10,
                // Jump by 4 bytes: 0xFFFC + 0x06 mod 0x10000 = 0x02
                opcodes::BNE,
                6,
                0, // reset vector, will be filled
                0, // reset vector, will be filled
            ],
        ));
        memory.bytes[2..4].copy_from_slice(&[opcodes::STA_ZP, 20]);
        let mut cpu = Cpu::new(memory);
        reset(&mut cpu);
        cpu.ticks(9).unwrap();
        assert_eq!(cpu.memory.bytes[20], 10);
    }

    #[bench]
    fn benchmark(b: &mut Bencher) {
        let memory = Box::new(SimpleRam::with_test_program(&mut [
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
        ]));
        let mut cpu = Cpu::new(memory);
        b.iter(|| {
            reset(&mut cpu);
            cpu.ticks(1000).unwrap();
        });
    }
}
