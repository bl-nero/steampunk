mod bcd;
mod flags;
pub mod opcodes;
mod tests;

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

/// A 6502 CPU that operates on a given type of memory. A key to creating a
/// working hardware implementation is to provide a `Memory` implementation
/// specific to your particular hardware.
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
    // Address
    adl: u8,
    adh: u8,
    // Base address
    bal: u8,
    bah: u8,
    // Indirect address
    ial: u8,
    tmp_data: u8,
}

type TickResult = Result<(), Box<dyn error::Error>>;

// enum CpuError {
//     ReadError,
//     WriteError,
// }

#[derive(Debug, Clone)]
struct UnknownOpcodeError {
    pub opcode: u8,
    pub address: u16,
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

#[derive(Debug, Clone, PartialEq)]
pub struct CpuHaltedError {
    pub opcode: u8,
    pub address: u16,
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
            adh: rng.gen(),
            bal: rng.gen(),
            bah: rng.gen(),
            ial: rng.gen(),
            tmp_data: rng.gen(),
        }
    }

    pub fn memory(&self) -> &M {
        &self.memory
    }

    pub fn mut_memory(&mut self) -> &mut M {
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
                self.sequence_state = SequenceState::Opcode(self.consume_program_byte()?, 0);
            }

            // List ALL the opcodes!
            SequenceState::Opcode(opcodes::NOP, _) => {
                self.tick_simple_internal_operation(&mut |_| {})?;
            }

            SequenceState::Opcode(opcodes::LDA_IMM, _) => {
                self.tick_load_immediate(&mut |me, value| me.set_reg_a(value))?;
            }
            SequenceState::Opcode(opcodes::LDX_IMM, _) => {
                self.tick_load_immediate(&mut |me, value| me.set_reg_x(value))?;
            }
            SequenceState::Opcode(opcodes::LDY_IMM, _) => {
                self.tick_load_immediate(&mut |me, value| me.set_reg_y(value))?;
            }

            SequenceState::Opcode(opcodes::LDA_ZP, _) => {
                self.tick_load_zero_page(&mut |me, value| me.set_reg_a(value))?;
            }
            SequenceState::Opcode(opcodes::LDX_ZP, _) => {
                self.tick_load_zero_page(&mut |me, value| me.set_reg_x(value))?;
            }
            SequenceState::Opcode(opcodes::LDY_ZP, _) => {
                self.tick_load_zero_page(&mut |me, value| me.set_reg_y(value))?;
            }

            SequenceState::Opcode(opcodes::LDA_ZP_X, _) => {
                self.tick_load_zero_page_x(&mut |me, value| me.set_reg_a(value))?;
            }
            SequenceState::Opcode(opcodes::LDA_ABS, _) => {
                self.tick_load_absolute(&mut |me, value| me.set_reg_a(value))?;
            }
            SequenceState::Opcode(opcodes::LDX_ABS, _) => {
                self.tick_load_absolute(&mut |me, value| me.set_reg_x(value))?;
            }
            SequenceState::Opcode(opcodes::LDY_ABS, _) => {
                self.tick_load_absolute(&mut |me, value| me.set_reg_y(value))?;
            }

            SequenceState::Opcode(opcodes::LDA_ABS_X, _) => {
                self.tick_load_absolute_indexed(self.reg_x, &mut |me, value| me.set_reg_a(value))?;
            }
            SequenceState::Opcode(opcodes::LDA_ABS_Y, _) => {
                self.tick_load_absolute_indexed(self.reg_y, &mut |me, value| me.set_reg_a(value))?;
            }

            SequenceState::Opcode(opcodes::LDA_X_INDIR, _) => {
                self.tick_load_x_indirect(&mut |me, value| me.set_reg_a(value))?;
            }
            SequenceState::Opcode(opcodes::LDA_INDIR_Y, _) => {
                self.tick_load_indirect_y(&mut |me, value| me.set_reg_a(value))?;
            }

            SequenceState::Opcode(opcodes::STA_ZP, _) => {
                self.tick_store_zero_page(self.reg_a)?;
            }
            SequenceState::Opcode(opcodes::STX_ZP, _) => {
                self.tick_store_zero_page(self.reg_x)?;
            }
            SequenceState::Opcode(opcodes::STY_ZP, _) => {
                self.tick_store_zero_page(self.reg_y)?;
            }

            SequenceState::Opcode(opcodes::STA_ZP_X, _) => {
                self.tick_store_zero_page_x(self.reg_a)?;
            }
            SequenceState::Opcode(opcodes::STY_ZP_X, _) => {
                self.tick_store_zero_page_x(self.reg_y)?;
            }

            SequenceState::Opcode(opcodes::STA_ABS, _) => {
                self.tick_store_abs(self.reg_a)?;
            }
            SequenceState::Opcode(opcodes::STX_ABS, _) => {
                self.tick_store_abs(self.reg_x)?;
            }
            SequenceState::Opcode(opcodes::STY_ABS, _) => {
                self.tick_store_abs(self.reg_y)?;
            }

            SequenceState::Opcode(opcodes::STA_ABS_X, _) => {
                self.tick_store_abs_indexed(self.reg_x, self.reg_a)?;
            }
            SequenceState::Opcode(opcodes::STA_ABS_Y, _) => {
                self.tick_store_abs_indexed(self.reg_y, self.reg_a)?;
            }

            SequenceState::Opcode(opcodes::STA_X_INDIR, _) => {
                self.tick_store_x_indirect(self.reg_a)?;
            }
            SequenceState::Opcode(opcodes::STA_INDIR_Y, _) => {
                self.tick_store_indirect_y(self.reg_a)?;
            }

            SequenceState::Opcode(opcodes::AND_IMM, _) => {
                self.tick_load_immediate(&mut |me, value| me.set_reg_a(me.reg_a & value))?;
            }
            SequenceState::Opcode(opcodes::AND_ZP, _) => {
                self.tick_load_zero_page(&mut |me, value| me.set_reg_a(me.reg_a & value))?;
            }
            SequenceState::Opcode(opcodes::AND_ZP_X, _) => {
                self.tick_load_zero_page_x(&mut |me, value| me.set_reg_a(me.reg_a & value))?;
            }
            SequenceState::Opcode(opcodes::AND_ABS, _) => {
                self.tick_load_absolute(&mut |me, value| me.set_reg_a(me.reg_a & value))?;
            }
            SequenceState::Opcode(opcodes::AND_ABS_X, _) => {
                self.tick_load_absolute_indexed(self.reg_x, &mut |me, value| {
                    me.set_reg_a(me.reg_a & value)
                })?;
            }
            SequenceState::Opcode(opcodes::AND_ABS_Y, _) => {
                self.tick_load_absolute_indexed(self.reg_y, &mut |me, value| {
                    me.set_reg_a(me.reg_a & value)
                })?;
            }
            SequenceState::Opcode(opcodes::AND_X_INDIR, _) => {
                self.tick_load_x_indirect(&mut |me, value| me.set_reg_a(me.reg_a & value))?;
            }
            SequenceState::Opcode(opcodes::AND_INDIR_Y, _) => {
                self.tick_load_indirect_y(&mut |me, value| me.set_reg_a(me.reg_a & value))?;
            }

            SequenceState::Opcode(opcodes::ORA_IMM, _) => {
                self.tick_load_immediate(&mut |me, value| me.set_reg_a(me.reg_a | value))?;
            }
            SequenceState::Opcode(opcodes::ORA_ZP, _) => {
                self.tick_load_zero_page(&mut |me, value| me.set_reg_a(me.reg_a | value))?;
            }
            SequenceState::Opcode(opcodes::ORA_ZP_X, _) => {
                self.tick_load_zero_page_x(&mut |me, value| me.set_reg_a(me.reg_a | value))?;
            }
            SequenceState::Opcode(opcodes::ORA_ABS, _) => {
                self.tick_load_absolute(&mut |me, value| me.set_reg_a(me.reg_a | value))?;
            }
            SequenceState::Opcode(opcodes::ORA_ABS_X, _) => {
                self.tick_load_absolute_indexed(self.reg_x, &mut |me, value| {
                    me.set_reg_a(me.reg_a | value)
                })?;
            }
            SequenceState::Opcode(opcodes::ORA_ABS_Y, _) => {
                self.tick_load_absolute_indexed(self.reg_y, &mut |me, value| {
                    me.set_reg_a(me.reg_a | value)
                })?;
            }
            SequenceState::Opcode(opcodes::ORA_X_INDIR, _) => {
                self.tick_load_x_indirect(&mut |me, value| me.set_reg_a(me.reg_a | value))?;
            }
            SequenceState::Opcode(opcodes::ORA_INDIR_Y, _) => {
                self.tick_load_indirect_y(&mut |me, value| me.set_reg_a(me.reg_a | value))?;
            }

            SequenceState::Opcode(opcodes::EOR_IMM, _) => {
                self.tick_load_immediate(&mut |me, value| me.set_reg_a(me.reg_a ^ value))?;
            }
            SequenceState::Opcode(opcodes::EOR_ZP, _) => {
                self.tick_load_zero_page(&mut |me, value| me.set_reg_a(me.reg_a ^ value))?;
            }

            SequenceState::Opcode(opcodes::ASL_A, _) => {
                self.tick_simple_internal_operation(&mut |me| {
                    let shifted = me.shift_left(me.reg_a);
                    me.set_reg_a(shifted);
                })?;
            }
            SequenceState::Opcode(opcodes::ASL_ZP, _) => {
                self.tick_load_modify_store_zero_page(&mut |me, value| me.shift_left(value))?;
            }
            SequenceState::Opcode(opcodes::ASL_ZP_X, _) => {
                self.tick_load_modify_store_zero_page_x(&mut |me, value| me.shift_left(value))?;
            }
            SequenceState::Opcode(opcodes::ASL_ABS, _) => {
                self.tick_load_modify_store_absolute(&mut |me, value| me.shift_left(value))?;
            }

            SequenceState::Opcode(opcodes::LSR_A, _) => {
                self.tick_simple_internal_operation(&mut |me| {
                    let shifted = me.shift_right(me.reg_a);
                    me.set_reg_a(shifted);
                })?;
            }
            SequenceState::Opcode(opcodes::LSR_ZP, _) => {
                self.tick_load_modify_store_zero_page(&mut |me, value| me.shift_right(value))?;
            }
            SequenceState::Opcode(opcodes::LSR_ZP_X, _) => {
                self.tick_load_modify_store_zero_page_x(&mut |me, value| me.shift_right(value))?;
            }
            SequenceState::Opcode(opcodes::LSR_ABS, _) => {
                self.tick_load_modify_store_absolute(&mut |me, value| me.shift_right(value))?;
            }

            SequenceState::Opcode(opcodes::ROL_A, _) => {
                self.tick_simple_internal_operation(&mut |me| {
                    let rotated = me.rotate_left(me.reg_a);
                    me.set_reg_a(rotated);
                })?;
            }
            SequenceState::Opcode(opcodes::ROL_ZP, _) => {
                self.tick_load_modify_store_zero_page(&mut |me, value| me.rotate_left(value))?;
            }
            SequenceState::Opcode(opcodes::ROL_ZP_X, _) => {
                self.tick_load_modify_store_zero_page_x(&mut |me, value| me.rotate_left(value))?;
            }
            SequenceState::Opcode(opcodes::ROL_ABS, _) => {
                self.tick_load_modify_store_absolute(&mut |me, value| me.rotate_left(value))?;
            }

            SequenceState::Opcode(opcodes::ROR_A, _) => {
                self.tick_simple_internal_operation(&mut |me| {
                    let rotated = me.rotate_right(me.reg_a);
                    me.set_reg_a(rotated);
                })?;
            }
            SequenceState::Opcode(opcodes::ROR_ZP, _) => {
                self.tick_load_modify_store_zero_page(&mut |me, value| me.rotate_right(value))?;
            }
            SequenceState::Opcode(opcodes::ROR_ZP_X, _) => {
                self.tick_load_modify_store_zero_page_x(&mut |me, value| me.rotate_right(value))?;
            }
            SequenceState::Opcode(opcodes::ROR_ABS, _) => {
                self.tick_load_modify_store_absolute(&mut |me, value| me.rotate_right(value))?;
            }

            SequenceState::Opcode(opcodes::CMP_IMM, _) => {
                self.tick_compare_immediate(self.reg_a)?;
            }
            SequenceState::Opcode(opcodes::CMP_ZP, _) => {
                self.tick_compare_zero_page(self.reg_a)?;
            }
            SequenceState::Opcode(opcodes::CMP_ZP_X, _) => {
                self.tick_compare_zero_page_x(self.reg_a)?;
            }

            SequenceState::Opcode(opcodes::CPX_IMM, _) => {
                self.tick_compare_immediate(self.reg_x)?;
            }
            SequenceState::Opcode(opcodes::CPX_ZP, _) => {
                self.tick_compare_zero_page(self.reg_x)?;
            }

            SequenceState::Opcode(opcodes::CPY_IMM, _) => {
                self.tick_compare_immediate(self.reg_y)?;
            }
            SequenceState::Opcode(opcodes::CPY_ZP, _) => {
                self.tick_compare_zero_page(self.reg_y)?;
            }

            SequenceState::Opcode(opcodes::BIT_ZP, _) => {
                self.tick_load_zero_page(&mut |me, value| me.test_bits(value))?;
            }
            SequenceState::Opcode(opcodes::BIT_ABS, _) => {
                self.tick_load_absolute(&mut |me, value| me.test_bits(value))?;
            }

            SequenceState::Opcode(opcodes::ADC_IMM, _) => {
                self.tick_load_immediate(&mut |me, value| {
                    let sum = me.add_with_carry(me.reg_a, value);
                    me.set_reg_a(sum);
                })?;
            }
            SequenceState::Opcode(opcodes::ADC_ZP, _) => {
                self.tick_load_zero_page(&mut |me, value| {
                    let sum = me.add_with_carry(me.reg_a, value);
                    me.set_reg_a(sum);
                })?;
            }
            SequenceState::Opcode(opcodes::ADC_ZP_X, _) => {
                self.tick_load_zero_page_x(&mut |me, value| {
                    let sum = me.add_with_carry(me.reg_a, value);
                    me.set_reg_a(sum);
                })?;
            }
            SequenceState::Opcode(opcodes::ADC_ABS, _) => {
                self.tick_load_absolute(&mut |me, value| {
                    let sum = me.add_with_carry(me.reg_a, value);
                    me.set_reg_a(sum);
                })?;
            }
            SequenceState::Opcode(opcodes::ADC_ABS_X, _) => {
                self.tick_load_absolute_indexed(self.reg_x, &mut |me, value| {
                    let sum = me.add_with_carry(me.reg_a, value);
                    me.set_reg_a(sum);
                })?;
            }
            SequenceState::Opcode(opcodes::ADC_ABS_Y, _) => {
                self.tick_load_absolute_indexed(self.reg_y, &mut |me, value| {
                    let sum = me.add_with_carry(me.reg_a, value);
                    me.set_reg_a(sum);
                })?;
            }

            SequenceState::Opcode(opcodes::SBC_IMM, _) => {
                self.tick_load_immediate(&mut |me, value| {
                    let diff = me.sub_with_carry(me.reg_a, value);
                    me.set_reg_a(diff);
                })?;
            }
            SequenceState::Opcode(opcodes::SBC_ZP, _) => {
                self.tick_load_zero_page(&mut |me, value| {
                    let diff = me.sub_with_carry(me.reg_a, value);
                    me.set_reg_a(diff);
                })?;
            }
            SequenceState::Opcode(opcodes::SBC_ZP_X, _) => {
                self.tick_load_zero_page_x(&mut |me, value| {
                    let diff = me.sub_with_carry(me.reg_a, value);
                    me.set_reg_a(diff);
                })?;
            }
            SequenceState::Opcode(opcodes::SBC_ABS, _) => {
                self.tick_load_absolute(&mut |me, value| {
                    let diff = me.sub_with_carry(me.reg_a, value);
                    me.set_reg_a(diff);
                })?;
            }
            SequenceState::Opcode(opcodes::SBC_ABS_X, _) => {
                self.tick_load_absolute_indexed(self.reg_x, &mut |me, value| {
                    let diff = me.sub_with_carry(me.reg_a, value);
                    me.set_reg_a(diff);
                })?;
            }
            SequenceState::Opcode(opcodes::SBC_ABS_Y, _) => {
                self.tick_load_absolute_indexed(self.reg_y, &mut |me, value| {
                    let diff = me.sub_with_carry(me.reg_a, value);
                    me.set_reg_a(diff);
                })?;
            }

            SequenceState::Opcode(opcodes::INC_ZP, _) => {
                self.tick_load_modify_store_zero_page(&mut |me, val| me.inc(val))?;
            }
            SequenceState::Opcode(opcodes::INC_ZP_X, _) => {
                self.tick_load_modify_store_zero_page_x(&mut |me, val| me.inc(val))?;
            }

            SequenceState::Opcode(opcodes::DEC_ZP, _) => {
                self.tick_load_modify_store_zero_page(&mut |me, val| me.dec(val))?;
            }
            SequenceState::Opcode(opcodes::DEC_ZP_X, _) => {
                self.tick_load_modify_store_zero_page_x(&mut |me, val| me.dec(val))?;
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

            SequenceState::Opcode(opcodes::TAX, _) => {
                self.tick_simple_internal_operation(&mut |me| me.set_reg_x(me.reg_a))?;
            }
            SequenceState::Opcode(opcodes::TAY, _) => {
                self.tick_simple_internal_operation(&mut |me| me.set_reg_y(me.reg_a))?;
            }
            SequenceState::Opcode(opcodes::TXA, _) => {
                self.tick_simple_internal_operation(&mut |me| me.set_reg_a(me.reg_x))?;
            }
            SequenceState::Opcode(opcodes::TYA, _) => {
                self.tick_simple_internal_operation(&mut |me| me.set_reg_a(me.reg_y))?;
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
            SequenceState::Opcode(opcodes::SED, _) => {
                self.tick_simple_internal_operation(&mut |me| me.flags |= flags::D)?;
            }
            SequenceState::Opcode(opcodes::CLD, _) => {
                self.tick_simple_internal_operation(&mut |me| me.flags &= !flags::D)?;
            }
            SequenceState::Opcode(opcodes::SEC, _) => {
                self.tick_simple_internal_operation(&mut |me| me.flags |= flags::C)?;
            }
            SequenceState::Opcode(opcodes::CLC, _) => {
                self.tick_simple_internal_operation(&mut |me| me.flags &= !flags::C)?;
            }
            SequenceState::Opcode(opcodes::CLV, _) => {
                self.tick_simple_internal_operation(&mut |me| me.flags &= !flags::V)?;
            }

            SequenceState::Opcode(opcodes::BEQ, _) => {
                self.tick_branch_if_flag(flags::Z, flags::Z)?;
            }
            SequenceState::Opcode(opcodes::BNE, _) => {
                self.tick_branch_if_flag(flags::Z, 0)?;
            }
            SequenceState::Opcode(opcodes::BCC, _) => {
                self.tick_branch_if_flag(flags::C, 0)?;
            }
            SequenceState::Opcode(opcodes::BCS, _) => {
                self.tick_branch_if_flag(flags::C, flags::C)?;
            }
            SequenceState::Opcode(opcodes::BPL, _) => {
                self.tick_branch_if_flag(flags::N, 0)?;
            }
            SequenceState::Opcode(opcodes::BMI, _) => {
                self.tick_branch_if_flag(flags::N, flags::N)?;
            }
            SequenceState::Opcode(opcodes::BVS, _) => {
                self.tick_branch_if_flag(flags::V, flags::C)?;
            }
            SequenceState::Opcode(opcodes::BVC, _) => {
                self.tick_branch_if_flag(flags::V, 0)?;
            }

            SequenceState::Opcode(opcodes::JMP_ABS, subcycle) => match subcycle {
                1 => self.adl = self.consume_program_byte()?,
                _ => {
                    self.adh = self.memory.read(self.reg_pc)?;
                    self.reg_pc = self.address();
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::JSR, subcycle) => match subcycle {
                1 => self.adl = self.consume_program_byte()?,
                2 => {
                    self.phantom_read(self.stack_pointer());
                }
                3 => {
                    self.memory
                        .write(self.stack_pointer(), (self.reg_pc >> 8) as u8)?;
                    self.reg_sp = self.reg_sp.wrapping_sub(1);
                }
                4 => {
                    self.memory.write(self.stack_pointer(), self.reg_pc as u8)?;
                    self.reg_sp = self.reg_sp.wrapping_sub(1);
                }
                _ => {
                    self.adh = self.memory.read(self.reg_pc)?;
                    self.reg_pc = self.address();
                    self.sequence_state = SequenceState::Ready;
                }
            },
            SequenceState::Opcode(opcodes::RTS, subcycle) => match subcycle {
                1 => {
                    let _ = self.consume_program_byte();
                }
                2 => {
                    self.phantom_read(self.stack_pointer());
                    self.reg_sp = self.reg_sp.wrapping_add(1);
                }
                3 => {
                    self.reg_pc =
                        self.reg_pc & 0xFF00 | self.memory.read(self.stack_pointer())? as u16;
                    self.reg_sp = self.reg_sp.wrapping_add(1);
                }
                4 => {
                    self.reg_pc =
                        self.reg_pc & 0xFF | ((self.memory.read(self.stack_pointer())? as u16) << 8)
                }
                _ => {
                    let _ = self.consume_program_byte();
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
                // OMG, there's a bug in the state machine!
                debug_assert!(subcycle < 7, "Runaway instruction: ${:02X}", opcode);
                self.sequence_state = SequenceState::Opcode(opcode, subcycle + 1)
            }
            SequenceState::Reset(subcycle) => {
                self.sequence_state = SequenceState::Reset(subcycle + 1)
            }
            _ => {}
        };
        Ok(())
    }

    fn tick_simple_internal_operation(
        &mut self,
        operation: &mut dyn FnMut(&mut Self),
    ) -> Result<(), ReadError> {
        self.phantom_read(self.reg_pc);
        operation(self);
        self.sequence_state = SequenceState::Ready;
        Ok(())
    }

    fn tick_load_immediate(
        &mut self,
        load: &mut dyn FnMut(&mut Self, u8),
    ) -> Result<(), ReadError> {
        let value = self.consume_program_byte()?;
        load(self, value);
        self.sequence_state = SequenceState::Ready;
        Ok(())
    }

    fn tick_load_zero_page(
        &mut self,
        load: &mut dyn FnMut(&mut Self, u8),
    ) -> Result<(), ReadError> {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.adl = self.consume_program_byte()?,
            _ => {
                load(self, self.memory.read(self.adl as u16)?);
                self.sequence_state = SequenceState::Ready;
            }
        };
        Ok(())
    }

    fn tick_load_zero_page_x(
        &mut self,
        load: &mut dyn FnMut(&mut Self, u8),
    ) -> Result<(), ReadError> {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.bal = self.consume_program_byte()?,
            SequenceState::Opcode(_, 2) => self.phantom_read(self.bal as u16),
            _ => {
                load(
                    self,
                    self.memory.read(self.bal.wrapping_add(self.reg_x) as u16)?,
                );
                self.sequence_state = SequenceState::Ready;
            }
        };
        Ok(())
    }

    fn tick_load_absolute(&mut self, load: &mut dyn FnMut(&mut Self, u8)) -> Result<(), ReadError> {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.adl = self.consume_program_byte()?,
            SequenceState::Opcode(_, 2) => self.adh = self.consume_program_byte()?,
            _ => {
                load(self, self.memory.read(self.address())?);
                self.sequence_state = SequenceState::Ready;
            }
        };
        Ok(())
    }

    fn tick_load_absolute_indexed(
        &mut self,
        index: u8,
        load: &mut dyn FnMut(&mut Self, u8),
    ) -> Result<(), ReadError> {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.bal = self.consume_program_byte()?,
            SequenceState::Opcode(_, 2) => self.bah = self.consume_program_byte()?,
            SequenceState::Opcode(_, 3) => {
                let (adl, carry) = self.bal.overflowing_add(index);
                let address = u16::from_le_bytes([adl, self.bah]);
                if carry {
                    self.phantom_read(address);
                } else {
                    load(self, self.memory.read(address)?);
                    self.sequence_state = SequenceState::Ready;
                }
            }
            _ => {
                load(
                    self,
                    self.memory
                        .read(self.base_address().wrapping_add(index as u16))?,
                );
                self.sequence_state = SequenceState::Ready;
            }
        };
        Ok(())
    }

    fn tick_load_x_indirect(
        &mut self,
        load: &mut dyn FnMut(&mut Self, u8),
    ) -> Result<(), ReadError> {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.bal = self.consume_program_byte()?,
            SequenceState::Opcode(_, 2) => self.phantom_read(self.bal as u16),
            SequenceState::Opcode(_, 3) => {
                self.adl = self.memory.read(self.bal.wrapping_add(self.reg_x) as u16)?;
            }
            SequenceState::Opcode(_, 4) => {
                self.adh = self
                    .memory
                    .read(self.bal.wrapping_add(self.reg_x).wrapping_add(1) as u16)?;
            }
            _ => {
                load(self, self.memory.read(self.address())?);
                self.sequence_state = SequenceState::Ready;
            }
        }
        Ok(())
    }

    fn tick_load_indirect_y(
        &mut self,
        load: &mut dyn FnMut(&mut Self, u8),
    ) -> Result<(), ReadError> {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.ial = self.consume_program_byte()?,
            SequenceState::Opcode(_, 2) => self.bal = self.memory.read(self.ial as u16)?,
            SequenceState::Opcode(_, 3) => {
                self.bah = self.memory.read(self.ial.wrapping_add(1) as u16)?
            }
            SequenceState::Opcode(_, 4) => {
                let (adl, carry) = self.bal.overflowing_add(self.reg_y);
                let address = u16::from_le_bytes([adl, self.bah]);
                if carry {
                    self.phantom_read(address);
                } else {
                    load(self, self.memory.read(address)?);
                    self.sequence_state = SequenceState::Ready;
                }
            }
            _ => {
                load(
                    self,
                    self.memory
                        .read(self.base_address().wrapping_add(self.reg_y as u16))?,
                );
                self.sequence_state = SequenceState::Ready;
            }
        }
        Ok(())
    }

    fn tick_store_zero_page(&mut self, value: u8) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.adl = self.consume_program_byte()?,
            _ => {
                self.memory.write(self.adl as u16, value)?;
                self.sequence_state = SequenceState::Ready;
            }
        };
        Ok(())
    }

    fn tick_store_zero_page_x(&mut self, value: u8) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.bal = self.consume_program_byte()?,
            SequenceState::Opcode(_, 2) => self.phantom_read(self.bal as u16),
            _ => {
                self.memory
                    .write((self.bal.wrapping_add(self.reg_x)) as u16, value)?;
                self.sequence_state = SequenceState::Ready;
            }
        };
        Ok(())
    }

    fn tick_store_abs(&mut self, value: u8) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.adl = self.consume_program_byte()?,
            SequenceState::Opcode(_, 2) => self.adh = self.consume_program_byte()?,
            _ => {
                self.memory.write(self.address(), value)?;
                self.sequence_state = SequenceState::Ready;
            }
        }
        Ok(())
    }

    fn tick_store_abs_indexed(&mut self, index: u8, value: u8) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.bal = self.consume_program_byte()?,
            SequenceState::Opcode(_, 2) => self.bah = self.consume_program_byte()?,
            SequenceState::Opcode(_, 3) => {
                self.phantom_read(u16::from_le_bytes([self.bal.wrapping_add(index), self.bah]));
            }
            _ => {
                self.memory
                    .write(self.base_address().wrapping_add(index as u16), value)?;
                self.sequence_state = SequenceState::Ready;
            }
        }
        Ok(())
    }

    fn tick_store_x_indirect(&mut self, value: u8) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.bal = self.consume_program_byte()?,
            SequenceState::Opcode(_, 2) => self.phantom_read(self.bal as u16),
            SequenceState::Opcode(_, 3) => {
                self.adl = self.memory.read(self.bal.wrapping_add(self.reg_x) as u16)?;
            }
            SequenceState::Opcode(_, 4) => {
                self.adh = self
                    .memory
                    .read(self.bal.wrapping_add(self.reg_x).wrapping_add(1) as u16)?;
            }
            _ => {
                self.memory.write(self.address(), value)?;
                self.sequence_state = SequenceState::Ready;
            }
        }
        Ok(())
    }

    fn tick_store_indirect_y(&mut self, value: u8) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.ial = self.consume_program_byte()?,
            SequenceState::Opcode(_, 2) => self.bal = self.memory.read(self.ial as u16)?,
            SequenceState::Opcode(_, 3) => {
                self.bah = self.memory.read(self.ial.wrapping_add(1) as u16)?
            }
            SequenceState::Opcode(_, 4) => {
                self.phantom_read(u16::from_le_bytes([
                    self.bal.wrapping_add(self.reg_y),
                    self.bah,
                ]));
            }
            _ => {
                self.memory
                    .write(self.base_address().wrapping_add(self.reg_y as u16), value)?;
                self.sequence_state = SequenceState::Ready;
            }
        }
        Ok(())
    }

    fn tick_load_modify_store_zero_page(
        &mut self,
        operation: &mut dyn FnMut(&mut Self, u8) -> u8,
    ) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.adl = self.consume_program_byte()?,
            SequenceState::Opcode(_, 2) => self.tmp_data = self.memory.read(self.adl as u16)?,
            SequenceState::Opcode(_, 3) => {
                // A rare case of a "phantom write". Since we write the same
                // data, it doesn't really matter (that much), but we need to
                // simulate it anyway.
                self.memory.write(self.adl as u16, self.tmp_data)?;
            }
            _ => {
                let result = operation(self, self.tmp_data);
                self.memory.write(self.adl as u16, result)?;
                self.sequence_state = SequenceState::Ready;
            }
        }
        Ok(())
    }

    fn tick_load_modify_store_zero_page_x(
        &mut self,
        operation: &mut dyn FnMut(&mut Self, u8) -> u8,
    ) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.bal = self.consume_program_byte()?,
            SequenceState::Opcode(_, 2) => self.phantom_read(self.bal as u16),
            SequenceState::Opcode(_, 3) => {
                self.adl = self.bal.wrapping_add(self.reg_x);
                self.tmp_data = self.memory.read(self.adl as u16)?;
            }
            SequenceState::Opcode(_, 4) => {
                // Phantom write.
                self.memory.write(self.adl as u16, self.tmp_data)?;
            }
            _ => {
                let result = operation(self, self.tmp_data);
                self.memory.write(self.adl as u16, result)?;
                self.sequence_state = SequenceState::Ready;
            }
        }
        Ok(())
    }

    fn tick_load_modify_store_absolute(
        &mut self,
        operation: &mut dyn FnMut(&mut Self, u8) -> u8,
    ) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.adl = self.consume_program_byte()?,
            SequenceState::Opcode(_, 2) => self.adh = self.consume_program_byte()?,
            SequenceState::Opcode(_, 3) => {
                self.tmp_data = self.memory.read(self.address())?;
            }
            SequenceState::Opcode(_, 4) => {
                // Phantom write.
                self.memory.write(self.address(), self.tmp_data)?;
            }
            _ => {
                let result = operation(self, self.tmp_data);
                self.memory.write(self.address(), result)?;
                self.sequence_state = SequenceState::Ready;
            }
        }
        Ok(())
    }

    fn tick_compare_immediate(&mut self, register: u8) -> Result<(), ReadError> {
        self.tick_load_immediate(&mut |me, value| me.compare(register, value))
    }

    fn tick_compare_zero_page(&mut self, register: u8) -> Result<(), ReadError> {
        self.tick_load_zero_page(&mut |me, value| me.compare(register, value))
    }

    fn tick_compare_zero_page_x(&mut self, register: u8) -> Result<(), ReadError> {
        self.tick_load_zero_page_x(&mut |me, value| me.compare(register, value))
    }

    fn tick_push(&mut self, value: u8) -> TickResult {
        match self.sequence_state {
            SequenceState::Opcode(_, 1) => self.phantom_read(self.reg_pc),
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
            SequenceState::Opcode(_, 1) => self.phantom_read(self.reg_pc),
            SequenceState::Opcode(_, 2) => {
                self.phantom_read(self.stack_pointer());
                self.reg_sp = self.reg_sp.wrapping_add(1);
            }
            _ => {
                load(self, self.memory.read(self.stack_pointer())?);
                self.sequence_state = SequenceState::Ready;
            }
        };
        Ok(())
    }

    fn tick_branch_if_flag(&mut self, flag: u8, value: u8) -> Result<(), ReadError> {
        match self.sequence_state {
            // TODO: handle additional cycle when crossing page boundaries
            SequenceState::Opcode(_, 1) => {
                self.adl = self.consume_program_byte()?;
                if self.flags & flag != value {
                    // Condition not met; don't branch.
                    self.sequence_state = SequenceState::Ready;
                }
            }
            SequenceState::Opcode(_, 2) => {
                let new_pc = self.reg_pc.wrapping_add(self.adl as i8 as u16);
                if new_pc & 0xFF00 == self.reg_pc & 0xFF00 {
                    // No page boundary crossed. Do a phantom read of the
                    // computed address and skip the next cycle.
                    self.phantom_read(self.reg_pc);
                    self.sequence_state = SequenceState::Ready;
                } else {
                    self.phantom_read((new_pc & 0x00FF) | (self.reg_pc & 0xFF00));
                    // Page boundary crossed. Do a phantom read of a
                    // partially computed address and continue to the next
                    // cycle.
                }
                self.reg_pc = new_pc;
            }
            _ => {
                self.phantom_read(self.reg_pc);
                self.sequence_state = SequenceState::Ready;
            }
        };
        Ok(())
    }

    /// Reads one byte from the program and advances the program counter.
    fn consume_program_byte(&mut self) -> ReadResult {
        let result = self.memory.read(self.reg_pc)?;
        self.reg_pc = self.reg_pc.wrapping_add(1);
        return Ok(result);
    }

    /// Performs a "phantom read", a side effect that usually doesn't matter,
    /// but may matter to some devices that react to reading its pins. Because
    /// we don't use the result value, we don't even care if it was a read
    /// error.
    fn phantom_read(&self, address: u16) {
        let _ = self.memory.read(address);
    }

    fn set_reg_a(&mut self, value: u8) {
        self.reg_a = value;
        self.update_flags_nz(value);
    }

    fn set_reg_x(&mut self, value: u8) {
        self.reg_x = value;
        self.update_flags_nz(value);
    }

    fn set_reg_y(&mut self, value: u8) {
        self.reg_y = value;
        self.update_flags_nz(value);
    }

    /// Updates the N and Z flags to reflect the given value.
    fn update_flags_nz(&mut self, value: u8) {
        let flag_z = if value == 0 { flags::Z } else { 0 };
        let flag_n = if value & 0b1000_0000 != 0 {
            flags::N
        } else {
            0
        };
        self.flags = (self.flags & !(flags::Z | flags::N)) | flag_z | flag_n;
    }

    fn test_bits(&mut self, value: u8) {
        // Clear N, V, and Z. Then load N and V (bits 7 and 6) directly from the
        // value, and update Z by performing an AND with the accumolator.
        self.flags = self.flags & !(flags::N | flags::V | flags::Z)
            | (value & (flags::N | flags::V))
            | if value & self.reg_a == 0 { flags::Z } else { 0 };
    }

    /// Calculates lhs+rhs+C, updates the C and V flags, and returns the result.
    /// The V flag is not set in BCD mode, which is not how the real CPU works,
    /// but it's undefined anyway.
    fn add_with_carry(&mut self, lhs: u8, rhs: u8) -> u8 {
        if self.flags & flags::D != 0 {
            let (result, carry) = bcd::bcd_add(lhs, rhs, self.flags & flags::C != 0);
            self.flags = if carry {
                self.flags | flags::C
            } else {
                self.flags & !flags::C
            };
            return result;
        }

        let (mut unsigned_sum, mut unsigned_overflow) = lhs.overflowing_add(rhs);
        if self.flags & flags::C != 0 {
            let (unsigned_sum_2, unsigned_overflow_2) = unsigned_sum.overflowing_add(1);
            unsigned_sum = unsigned_sum_2;
            unsigned_overflow |= unsigned_overflow_2;
        }
        let signed_lhs = lhs as i8;
        let signed_rhs = rhs as i8;
        let (mut signed_sum, mut signed_overflow) = signed_lhs.overflowing_add(signed_rhs);
        if self.flags & flags::C != 0 {
            let (signed_sum_2, signed_overflow_2) = signed_sum.overflowing_add(1);
            signed_sum = signed_sum_2;
            signed_overflow |= signed_overflow_2;
        }
        debug_assert_eq!(unsigned_sum, signed_sum as u8); // sanity check
        self.flags = (self.flags & !(flags::C | flags::V))
            | if unsigned_overflow { flags::C } else { 0 }
            | if signed_overflow { flags::V } else { 0 };
        return unsigned_sum;
    }

    /// Calculates lhs-rhs-(1-C), updates the C and V flags, and returns the
    /// result.
    fn sub_with_carry(&mut self, lhs: u8, rhs: u8) -> u8 {
        if self.flags & flags::D != 0 {
            let (result, borrow) = bcd::bcd_sub(lhs, rhs, self.flags & flags::C == 0);
            self.flags = if borrow {
                self.flags & !flags::C
            } else {
                self.flags | flags::C
            };
            return result;
        }

        let (mut unsigned_diff, mut unsigned_overflow) = lhs.overflowing_sub(rhs);
        if self.flags & flags::C == 0 {
            let (unsigned_diff_2, unsigned_overflow_2) = unsigned_diff.overflowing_sub(1);
            unsigned_diff = unsigned_diff_2;
            unsigned_overflow |= unsigned_overflow_2;
        }
        let signed_lhs = lhs as i8;
        let signed_rhs = rhs as i8;
        let (mut signed_diff, mut signed_overflow) = signed_lhs.overflowing_sub(signed_rhs);
        if self.flags & flags::C == 0 {
            let (signed_diff_2, signed_overflow_2) = signed_diff.overflowing_sub(1);
            signed_diff = signed_diff_2;
            signed_overflow |= signed_overflow_2;
        }
        debug_assert_eq!(unsigned_diff, signed_diff as u8); // sanity check
        self.flags = (self.flags & !(flags::C | flags::V))
            | if unsigned_overflow { 0 } else { flags::C }
            | if signed_overflow { flags::V } else { 0 };
        return unsigned_diff;
    }

    fn shift_left(&mut self, value: u8) -> u8 {
        let carry = (value & (1 << 7)) >> 7;
        self.flags = (self.flags & !flags::C) | carry;
        return value << 1;
    }

    fn shift_right(&mut self, value: u8) -> u8 {
        let carry = value & 1;
        self.flags = (self.flags & !flags::C) | carry;
        return value >> 1;
    }

    fn rotate_left(&mut self, value: u8) -> u8 {
        let prev_carry = self.flags & flags::C;
        let carry = (value & (1 << 7)) >> 7;
        self.flags = (self.flags & !flags::C) | carry;
        return (value << 1) | prev_carry;
    }

    fn rotate_right(&mut self, value: u8) -> u8 {
        let prev_carry = self.flags & flags::C;
        let carry = value & 1;
        self.flags = (self.flags & !flags::C) | carry;
        return (value >> 1) | (prev_carry << 7);
    }

    fn compare(&mut self, register: u8, value: u8) {
        let (difference, borrow) = register.overflowing_sub(value);
        self.update_flags_nz(difference);
        self.flags = self.flags & !flags::C | if borrow { 0 } else { flags::C };
    }

    fn inc(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        self.update_flags_nz(result);
        result
    }

    fn dec(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        self.update_flags_nz(result);
        result
    }

    fn stack_pointer(&self) -> u16 {
        0x100 | self.reg_sp as u16
    }

    /// Returns a 16-bit address stored in (`adh`, `adl`).
    fn address(&self) -> u16 {
        u16::from_le_bytes([self.adl, self.adh])
    }

    /// Returns a 16-bit address stored in (`bah`, `bal`).
    fn base_address(&self) -> u16 {
        u16::from_le_bytes([self.bal, self.bah])
    }

    #[cfg(test)]
    fn ticks(&mut self, n_ticks: u32) -> TickResult {
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
            flags::flags_to_string(self.flags)
        )
    }
}
