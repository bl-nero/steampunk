use crate::debugger::dap_types::DisassembledInstruction;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::iter;
use ya6502::cpu::MachineInspector;

/// Disassembles a memory region. The region starts at `start_address`. First
/// `margin` instructions are ignored to allow for a "runway" in disassembling
/// the initial, potentially ambiguous, chain of instructions. The function
/// returns a vector of instructions of a given `length`. Additionally, a
/// concept of disassembly `origin` allows to have one place in the address
/// space that is known to be a valid start of an instruction (e.g. it's
/// currently a PC or belongs to a chain of already disassembled instructions).
/// This way, multiple disassembly requests for adjacent or overlapping memory
/// regions are guaranteed to produce a coherent output.
pub fn disassemble<I: MachineInspector>(
    inspector: &I,
    origin: u16,
    start_address: u16,
    margin: usize,
    length: usize,
) -> Vec<DisassembledInstruction> {
    let mut memory_stream = MemoryStream::new(inspector, start_address);
    return iter::from_fn(|| {
        let instruction_start = memory_stream.ptr;
        let instruction = read_instruction_unless_crosses_origin(&mut memory_stream, origin);

        use itertools::Itertools;
        let all_bytes = instruction.to_raw_bytes();
        let mnemonic = match instruction.descriptor {
            Some(descriptor) => descriptor.mnemonic,
            None => "",
        }
        .to_string();
        let argument = match instruction.argument {
            Some(argument) => format!("{}", argument),
            None => "".to_string(),
        };
        let instruction_parts = [mnemonic, argument];
        let non_empty_instruction_parts = instruction_parts.iter().filter(|s| s.len() > 0);
        return Some(DisassembledInstruction {
            address: format!("0x{:04X}", instruction_start),
            instruction_bytes: format!("{:02X}", all_bytes.iter().format(" ")),
            instruction: format!("{}", non_empty_instruction_parts.format(" ")),
        });
    })
    .skip(margin)
    .take(length)
    .collect();
}

fn read_instruction_unless_crosses_origin<'a, I>(
    stream: &mut MemoryStream<I>,
    origin: u16,
) -> Instruction<'a>
where
    I: MachineInspector,
{
    let instruction_start = stream.ptr;
    let instruction = stream.read_instruction();
    let crossed_origin = (instruction_start < origin && origin < stream.ptr)
        || (stream.ptr < instruction_start && instruction_start < origin)
        || (origin < stream.ptr && stream.ptr < instruction_start);

    if crossed_origin {
        stream.ptr = instruction_start.wrapping_add(1);
        return Instruction {
            opcode: instruction.opcode,
            descriptor: None,
            argument: None,
        };
    }

    return instruction;
}

/// Adds a given number of instructions (`offset`) to the `origin` address. If
/// the offset is positive, adding is analogous to the actual disassembly
/// process; if it's negative, we use a heuristic algorithm that minimizes the
/// number of unknown instructions.
pub fn seek_instruction<I: MachineInspector>(inspector: &I, origin: u16, offset: i64) -> u16 {
    let mut stream = MemoryStream::new(inspector, origin);

    if offset >= 0 {
        for _ in 0..offset {
            stream.read_instruction();
        }
        return stream.ptr;
    } else {
        // Initialize a vector of chain links. A chain link at index `i`
        // represents a (potential) instruction that starts at address `origin -
        // i`. The first chain link is obviously the origin instruction itself.
        let mut chain_links = vec![ChainLink {
            num_instructions: 0,
            num_unknown_instructions: 0,
        }];

        // This variable holds a vector of indices in the `chain_links` vector.
        // Each candidate link is guaranteed to be exactly `-offset`
        // instructions away from the origin.
        let mut candidate_link_indices = vec![];

        // Repeat the loop until at least 3 trailing chain links have at least
        // the required number of instructions. The magical number "3" stems
        // from the maximum number of bytes in a single 6502 instruction. Note
        // that perhaps it could be mathematically proven that we can finish
        // earlier, but that would be just a microoptimization that doesn't
        // change too much.
        while !seeking_backward_finished(&chain_links, offset) {
            // Go back one instruction and back up that pointer, since we'll
            // attempt to consume an instruction from here.
            let ptr = stream.ptr.wrapping_sub(1);
            stream.ptr = ptr;

            let instruction = stream.read_instruction();
            let is_unknown = instruction.descriptor.is_none();
            let instruction_length: usize = stream.ptr.wrapping_sub(ptr).into();

            // The target link offset denotes number of bytes until the next
            // link after current instruction. In a special case where after
            // consuming the instruction, we end up crossing the origin (in
            // other words, we skip more chain links than we have), we only move
            // by one instruction; in this case, the disassembly algorithm needs
            // to treat this instruction as a data byte, possibly an end of a
            // data segment right before the code block where the PC is
            // currently pointing at.
            let target_link_offset = if instruction_length <= chain_links.len() {
                instruction_length
            } else {
                1
            };
            let target_link_index = chain_links.len() - target_link_offset;

            // The current instruction, after being consumed, leads to this
            // target link.
            let target_link = &chain_links[target_link_index];

            // We can now create current instruction's chain link, deriving its
            // parameters from the target link.
            let link = ChainLink {
                num_instructions: target_link.num_instructions + 1,
                num_unknown_instructions: if is_unknown {
                    target_link.num_unknown_instructions + 1
                } else {
                    target_link.num_unknown_instructions
                },
            };

            // If we hit exactly the required number of instructions, we mark
            // the current instruction as a candidate link.
            if link.num_instructions == -offset {
                candidate_link_indices.push(chain_links.len());
            }
            chain_links.push(link);

            // Go back to where the current instruction started.
            stream.ptr = ptr;
        }

        // Once we finish computing the candidate links, we return the one with
        // the smallest number of unknown instructions.
        return origin.wrapping_sub(
            candidate_link_indices
                .into_iter()
                .min_by_key(|index| chain_links[*index].num_unknown_instructions)
                .expect("Unable to find matching candidate link") as u16,
        );
    }
}

/// An auxiliary structure used by the instruction seeking algorithm. It
/// represents a place in memory that is a start of a chain of a given number of
/// instructions, some of them unknown (i.e. uninterpretable bytes).
#[derive(Clone, Debug)]
struct ChainLink {
    /// Number of instructions until the origin.
    num_instructions: i64,
    /// Number of unknown instructions (i.e. uninterpretable bytes) in the chain.
    num_unknown_instructions: u16,
}

/// Checks whether there are at least 3 consecutive chain links at the end with
/// the minimal required number of instructions.
fn seeking_backward_finished(chain_links: &[ChainLink], offset: i64) -> bool {
    chain_links.len() >= 3
        && chain_links
            .iter()
            .rev()
            .take(3)
            .all(|link| link.num_instructions >= -offset)
}

#[derive(Clone, Copy, Debug)]
enum AddressingMode {
    Accumulator,
    Immediate,
    Implied,
    Relative,
    Absolute,
    ZeroPage,
    Indirect,
    AbsoluteIndexedX,
    AbsoluteIndexedY,
    ZeroPageIndexedX,
    ZeroPageIndexedY,
    ZeroPageXIndirect,
    ZeroPageIndirectY,
}

/// Encapsulates an instruction argument for a given addressing mode.
#[derive(Clone, Copy, Debug)]
enum Argument {
    Accumulator,
    Immediate(u8),
    Implied,
    Relative(u8),
    Absolute(u16),
    ZeroPage(u8),
    Indirect(u16),
    AbsoluteIndexedX(u16),
    AbsoluteIndexedY(u16),
    ZeroPageIndexedX(u8),
    ZeroPageIndexedY(u8),
    ZeroPageXIndirect(u8),
    ZeroPageIndirectY(u8),
}

impl Display for Argument {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use Argument::*;
        match self {
            Accumulator => write!(f, "A"),
            Immediate(arg) => write!(f, "#${:02X}", arg),
            Implied => Ok(()),
            Relative(arg) => write!(f, "{}", *arg as i8),
            Absolute(arg) => write!(f, "${:04X}", arg),
            ZeroPage(arg) => write!(f, "${:02X}", arg),
            Indirect(arg) => write!(f, "(${:04X})", arg),
            AbsoluteIndexedX(arg) => write!(f, "${:04X},X", arg),
            AbsoluteIndexedY(arg) => write!(f, "${:04X},Y", arg),
            ZeroPageIndexedX(arg) => write!(f, "${:02X},X", arg),
            ZeroPageIndexedY(arg) => write!(f, "${:02X},Y", arg),
            ZeroPageXIndirect(arg) => write!(f, "(${:02X},X)", arg),
            ZeroPageIndirectY(arg) => write!(f, "(${:02X}),Y", arg),
        }
    }
}

impl Argument {
    /// Returns instruction argument as a byte vector.
    fn to_raw_bytes(self) -> Vec<u8> {
        use Argument::*;
        match self {
            Accumulator | Implied => vec![],
            Immediate(arg)
            | Relative(arg)
            | ZeroPage(arg)
            | ZeroPageIndexedX(arg)
            | ZeroPageIndexedY(arg)
            | ZeroPageXIndirect(arg)
            | ZeroPageIndirectY(arg) => vec![arg],
            Absolute(arg) | Indirect(arg) | AbsoluteIndexedX(arg) | AbsoluteIndexedY(arg) => {
                arg.to_le_bytes().to_vec()
            }
        }
    }
}

impl AddressingMode {
    /// Reads an instruction argument from a memory stream.
    fn read_argument<'a, I>(self, stream: &mut MemoryStream<'a, I>) -> Argument
    where
        I: MachineInspector,
    {
        match self {
            AddressingMode::Accumulator => Argument::Accumulator,
            AddressingMode::Immediate => Argument::Immediate(stream.read_byte()),
            AddressingMode::Implied => Argument::Implied,
            AddressingMode::Relative => Argument::Relative(stream.read_byte()),
            AddressingMode::Absolute => Argument::Absolute(stream.read_word()),
            AddressingMode::ZeroPage => Argument::ZeroPage(stream.read_byte()),
            AddressingMode::Indirect => Argument::Indirect(stream.read_word()),
            AddressingMode::AbsoluteIndexedX => Argument::AbsoluteIndexedX(stream.read_word()),
            AddressingMode::AbsoluteIndexedY => Argument::AbsoluteIndexedY(stream.read_word()),
            AddressingMode::ZeroPageIndexedX => Argument::ZeroPageIndexedX(stream.read_byte()),
            AddressingMode::ZeroPageIndexedY => Argument::ZeroPageIndexedY(stream.read_byte()),
            AddressingMode::ZeroPageXIndirect => Argument::ZeroPageXIndirect(stream.read_byte()),
            AddressingMode::ZeroPageIndirectY => Argument::ZeroPageIndirectY(stream.read_byte()),
        }
    }
}

/// A reader that reads data from the machine inspector's address space.
struct MemoryStream<'a, I: MachineInspector> {
    inspector: &'a I,
    ptr: u16,
}

impl<'a, I: MachineInspector> MemoryStream<'a, I> {
    fn new(inspector: &'a I, ptr: u16) -> Self {
        Self { inspector, ptr }
    }
    fn read_byte(&mut self) -> u8 {
        let b = self.inspector.inspect_memory(self.ptr);
        self.ptr = self.ptr.wrapping_add(1);
        return b;
    }
    fn read_word(&mut self) -> u16 {
        let lsb = self.read_byte();
        let msb = self.read_byte();
        return u16::from_le_bytes([lsb, msb]);
    }
    // Note: it's neceessary to explicitly declare <'b> here, since otherwise,
    // the returned instruction implicitly borrows `self` mutably. Don't even
    // ask me how.
    fn read_instruction<'b>(&mut self) -> Instruction<'b> {
        let opcode = self.read_byte();
        let descriptor = INSTRUCTION_DESCRIPTORS.with(|descriptors| descriptors[opcode as usize]);
        let argument = descriptor.map(|d| d.addressing_mode.read_argument(self));
        return Instruction {
            opcode,
            argument,
            descriptor,
        };
    }
}

struct Instruction<'a> {
    opcode: u8,
    argument: Option<Argument>,
    descriptor: Option<InstructionDescriptor<'a>>,
}

impl<'a> Instruction<'a> {
    fn to_raw_bytes(&self) -> Vec<u8> {
        let arg_bytes = match self.argument {
            Some(arg) => arg.to_raw_bytes(),
            None => vec![],
        };
        return iter::once(self.opcode).chain(arg_bytes).collect();
    }
}

#[derive(Clone, Copy)]
struct InstructionDescriptor<'a> {
    mnemonic: &'a str,
    addressing_mode: AddressingMode,
}

type InstructionDescriptorMap<'a> = [Option<InstructionDescriptor<'a>>; 256];

thread_local! {
    /// A map that describes addressing modes of all possible opcodes.
    static INSTRUCTION_DESCRIPTORS: InstructionDescriptorMap<'static> = all_instruction_descriptors();
}

fn all_instruction_descriptors<'a>() -> InstructionDescriptorMap<'a> {
    use ya6502::cpu::opcodes::*;
    use AddressingMode::*;
    let mut descriptors = [None; 256];

    define_instruction(&mut descriptors, NOP, "NOP", Implied);

    define_instruction(&mut descriptors, LDA_IMM, "LDA", Immediate);
    define_instruction(&mut descriptors, LDA_ZP, "LDA", ZeroPage);
    define_instruction(&mut descriptors, LDA_ZP_X, "LDA", ZeroPageIndexedX);
    define_instruction(&mut descriptors, LDA_ABS, "LDA", Absolute);
    define_instruction(&mut descriptors, LDA_ABS_X, "LDA", AbsoluteIndexedX);
    define_instruction(&mut descriptors, LDA_ABS_Y, "LDA", AbsoluteIndexedY);
    define_instruction(&mut descriptors, LDA_X_INDIR, "LDA", ZeroPageXIndirect);
    define_instruction(&mut descriptors, LDA_INDIR_Y, "LDA", ZeroPageIndirectY);

    define_instruction(&mut descriptors, LDX_IMM, "LDX", Immediate);
    define_instruction(&mut descriptors, LDX_ZP, "LDX", ZeroPage);
    define_instruction(&mut descriptors, LDX_ZP_Y, "LDX", ZeroPageIndexedY);
    define_instruction(&mut descriptors, LDX_ABS, "LDX", Absolute);
    define_instruction(&mut descriptors, LDX_ABS_Y, "LDX", AbsoluteIndexedY);

    define_instruction(&mut descriptors, LDY_IMM, "LDY", Immediate);
    define_instruction(&mut descriptors, LDY_ZP, "LDY", ZeroPage);
    define_instruction(&mut descriptors, LDY_ZP_X, "LDY", ZeroPageIndexedX);
    define_instruction(&mut descriptors, LDY_ABS, "LDY", Absolute);
    define_instruction(&mut descriptors, LDY_ABS_X, "LDY", AbsoluteIndexedX);

    define_instruction(&mut descriptors, STA_ZP, "STA", ZeroPage);
    define_instruction(&mut descriptors, STA_ZP_X, "STA", ZeroPageIndexedX);
    define_instruction(&mut descriptors, STA_ABS, "STA", Absolute);
    define_instruction(&mut descriptors, STA_ABS_X, "STA", AbsoluteIndexedX);
    define_instruction(&mut descriptors, STA_ABS_Y, "STA", AbsoluteIndexedY);
    define_instruction(&mut descriptors, STA_X_INDIR, "STA", ZeroPageXIndirect);
    define_instruction(&mut descriptors, STA_INDIR_Y, "STA", ZeroPageIndirectY);

    define_instruction(&mut descriptors, STX_ZP, "STX", ZeroPage);
    define_instruction(&mut descriptors, STX_ZP_Y, "STX", ZeroPageIndexedY);
    define_instruction(&mut descriptors, STX_ABS, "STX", Absolute);

    define_instruction(&mut descriptors, STY_ZP, "STY", ZeroPage);
    define_instruction(&mut descriptors, STY_ZP_X, "STY", ZeroPageIndexedX);
    define_instruction(&mut descriptors, STY_ABS, "STY", Absolute);

    define_instruction(&mut descriptors, AND_IMM, "AND", Immediate);
    define_instruction(&mut descriptors, AND_ZP, "AND", ZeroPage);
    define_instruction(&mut descriptors, AND_ZP_X, "AND", ZeroPageIndexedX);
    define_instruction(&mut descriptors, AND_ABS, "AND", Absolute);
    define_instruction(&mut descriptors, AND_ABS_X, "AND", AbsoluteIndexedX);
    define_instruction(&mut descriptors, AND_ABS_Y, "AND", AbsoluteIndexedY);
    define_instruction(&mut descriptors, AND_X_INDIR, "AND", ZeroPageXIndirect);
    define_instruction(&mut descriptors, AND_INDIR_Y, "AND", ZeroPageIndirectY);

    define_instruction(&mut descriptors, ORA_IMM, "ORA", Immediate);
    define_instruction(&mut descriptors, ORA_ZP, "ORA", ZeroPage);
    define_instruction(&mut descriptors, ORA_ZP_X, "ORA", ZeroPageIndexedX);
    define_instruction(&mut descriptors, ORA_ABS, "ORA", Absolute);
    define_instruction(&mut descriptors, ORA_ABS_X, "ORA", AbsoluteIndexedX);
    define_instruction(&mut descriptors, ORA_ABS_Y, "ORA", AbsoluteIndexedY);
    define_instruction(&mut descriptors, ORA_X_INDIR, "ORA", ZeroPageXIndirect);
    define_instruction(&mut descriptors, ORA_INDIR_Y, "ORA", ZeroPageIndirectY);

    define_instruction(&mut descriptors, EOR_IMM, "EOR", Immediate);
    define_instruction(&mut descriptors, EOR_ZP, "EOR", ZeroPage);
    define_instruction(&mut descriptors, EOR_ZP_X, "EOR", ZeroPageIndexedX);
    define_instruction(&mut descriptors, EOR_ABS, "EOR", Absolute);
    define_instruction(&mut descriptors, EOR_ABS_X, "EOR", AbsoluteIndexedX);
    define_instruction(&mut descriptors, EOR_ABS_Y, "EOR", AbsoluteIndexedY);
    define_instruction(&mut descriptors, EOR_X_INDIR, "EOR", ZeroPageXIndirect);
    define_instruction(&mut descriptors, EOR_INDIR_Y, "EOR", ZeroPageIndirectY);

    define_instruction(&mut descriptors, ASL_A, "ASL", Accumulator);
    define_instruction(&mut descriptors, ASL_ZP, "ASL", ZeroPage);
    define_instruction(&mut descriptors, ASL_ZP_X, "ASL", ZeroPageIndexedX);
    define_instruction(&mut descriptors, ASL_ABS, "ASL", Absolute);
    define_instruction(&mut descriptors, ASL_ABS_X, "ASL", AbsoluteIndexedX);

    define_instruction(&mut descriptors, LSR_A, "LSR", Accumulator);
    define_instruction(&mut descriptors, LSR_ZP, "LSR", ZeroPage);
    define_instruction(&mut descriptors, LSR_ZP_X, "LSR", ZeroPageIndexedX);
    define_instruction(&mut descriptors, LSR_ABS, "LSR", Absolute);
    define_instruction(&mut descriptors, LSR_ABS_X, "LSR", AbsoluteIndexedX);

    define_instruction(&mut descriptors, ROL_A, "ROL", Accumulator);
    define_instruction(&mut descriptors, ROL_ZP, "ROL", ZeroPage);
    define_instruction(&mut descriptors, ROL_ZP_X, "ROL", ZeroPageIndexedX);
    define_instruction(&mut descriptors, ROL_ABS, "ROL", Absolute);
    define_instruction(&mut descriptors, ROL_ABS_X, "ROL", AbsoluteIndexedX);

    define_instruction(&mut descriptors, ROR_A, "ROR", Accumulator);
    define_instruction(&mut descriptors, ROR_ZP, "ROR", ZeroPage);
    define_instruction(&mut descriptors, ROR_ZP_X, "ROR", ZeroPageIndexedX);
    define_instruction(&mut descriptors, ROR_ABS, "ROR", Absolute);
    define_instruction(&mut descriptors, ROR_ABS_X, "ROR", AbsoluteIndexedX);

    define_instruction(&mut descriptors, CMP_IMM, "CMP", Immediate);
    define_instruction(&mut descriptors, CMP_ZP, "CMP", ZeroPage);
    define_instruction(&mut descriptors, CMP_ZP_X, "CMP", ZeroPageIndexedX);
    define_instruction(&mut descriptors, CMP_ABS, "CMP", Absolute);
    define_instruction(&mut descriptors, CMP_ABS_X, "CMP", AbsoluteIndexedX);
    define_instruction(&mut descriptors, CMP_ABS_Y, "CMP", AbsoluteIndexedY);
    define_instruction(&mut descriptors, CMP_X_INDIR, "CMP", ZeroPageXIndirect);
    define_instruction(&mut descriptors, CMP_INDIR_Y, "CMP", ZeroPageIndirectY);

    define_instruction(&mut descriptors, CPX_IMM, "CPX", Immediate);
    define_instruction(&mut descriptors, CPX_ZP, "CPX", ZeroPage);
    define_instruction(&mut descriptors, CPX_ABS, "CPX", Absolute);

    define_instruction(&mut descriptors, CPY_IMM, "CPY", Immediate);
    define_instruction(&mut descriptors, CPY_ZP, "CPY", ZeroPage);
    define_instruction(&mut descriptors, CPY_ABS, "CPY", Absolute);

    define_instruction(&mut descriptors, BIT_ZP, "BIT", ZeroPage);
    define_instruction(&mut descriptors, BIT_ABS, "BIT", Absolute);

    define_instruction(&mut descriptors, ADC_IMM, "ADC", Immediate);
    define_instruction(&mut descriptors, ADC_ZP, "ADC", ZeroPage);
    define_instruction(&mut descriptors, ADC_ZP_X, "ADC", ZeroPageIndexedX);
    define_instruction(&mut descriptors, ADC_ABS, "ADC", Absolute);
    define_instruction(&mut descriptors, ADC_ABS_X, "ADC", AbsoluteIndexedX);
    define_instruction(&mut descriptors, ADC_ABS_Y, "ADC", AbsoluteIndexedY);
    define_instruction(&mut descriptors, ADC_X_INDIR, "ADC", ZeroPageXIndirect);
    define_instruction(&mut descriptors, ADC_INDIR_Y, "ADC", ZeroPageIndirectY);

    define_instruction(&mut descriptors, SBC_IMM, "SBC", Immediate);
    define_instruction(&mut descriptors, SBC_ZP, "SBC", ZeroPage);
    define_instruction(&mut descriptors, SBC_ZP_X, "SBC", ZeroPageIndexedX);
    define_instruction(&mut descriptors, SBC_ABS, "SBC", Absolute);
    define_instruction(&mut descriptors, SBC_ABS_X, "SBC", AbsoluteIndexedX);
    define_instruction(&mut descriptors, SBC_ABS_Y, "SBC", AbsoluteIndexedY);
    define_instruction(&mut descriptors, SBC_X_INDIR, "SBC", ZeroPageXIndirect);
    define_instruction(&mut descriptors, SBC_INDIR_Y, "SBC", ZeroPageIndirectY);

    define_instruction(&mut descriptors, INC_ZP, "INC", ZeroPage);
    define_instruction(&mut descriptors, INC_ZP_X, "INC", ZeroPageIndexedX);
    define_instruction(&mut descriptors, INC_ABS, "INC", Absolute);
    define_instruction(&mut descriptors, INC_ABS_X, "INC", AbsoluteIndexedX);

    define_instruction(&mut descriptors, DEC_ZP, "DEC", ZeroPage);
    define_instruction(&mut descriptors, DEC_ZP_X, "DEC", ZeroPageIndexedX);
    define_instruction(&mut descriptors, DEC_ABS, "DEC", Absolute);
    define_instruction(&mut descriptors, DEC_ABS_X, "DEC", AbsoluteIndexedX);

    define_instruction(&mut descriptors, INX, "INX", Implied);
    define_instruction(&mut descriptors, INY, "INY", Implied);
    define_instruction(&mut descriptors, DEX, "DEX", Implied);
    define_instruction(&mut descriptors, DEY, "DEY", Implied);

    define_instruction(&mut descriptors, TAX, "TAX", Implied);
    define_instruction(&mut descriptors, TAY, "TAY", Implied);
    define_instruction(&mut descriptors, TXA, "TXA", Implied);
    define_instruction(&mut descriptors, TYA, "TYA", Implied);
    define_instruction(&mut descriptors, TXS, "TXS", Implied);
    define_instruction(&mut descriptors, TSX, "TSX", Implied);

    define_instruction(&mut descriptors, PHP, "PHP", Implied);
    define_instruction(&mut descriptors, PHA, "PHA", Implied);
    define_instruction(&mut descriptors, PLP, "PLP", Implied);
    define_instruction(&mut descriptors, PLA, "PLA", Implied);

    define_instruction(&mut descriptors, SEI, "SEI", Implied);
    define_instruction(&mut descriptors, CLI, "CLI", Implied);
    define_instruction(&mut descriptors, SED, "SED", Implied);
    define_instruction(&mut descriptors, CLD, "CLD", Implied);
    define_instruction(&mut descriptors, SEC, "SEC", Implied);
    define_instruction(&mut descriptors, CLC, "CLC", Implied);
    define_instruction(&mut descriptors, CLV, "CLV", Implied);

    define_instruction(&mut descriptors, BEQ, "BEQ", Relative);
    define_instruction(&mut descriptors, BNE, "BNE", Relative);
    define_instruction(&mut descriptors, BCC, "BCC", Relative);
    define_instruction(&mut descriptors, BCS, "BCS", Relative);
    define_instruction(&mut descriptors, BPL, "BPL", Relative);
    define_instruction(&mut descriptors, BMI, "BMI", Relative);
    define_instruction(&mut descriptors, BVS, "BVS", Relative);
    define_instruction(&mut descriptors, BVC, "BVC", Relative);

    define_instruction(&mut descriptors, JMP_ABS, "JMP", Absolute);
    define_instruction(&mut descriptors, JMP_INDIR, "JMP", Indirect);
    define_instruction(&mut descriptors, JSR, "JSR", Absolute);
    define_instruction(&mut descriptors, RTS, "RTS", Implied);
    define_instruction(&mut descriptors, BRK, "BRK", Implied);
    define_instruction(&mut descriptors, RTI, "RTI", Implied);

    return descriptors;
}

fn define_instruction<'a>(
    descriptors: &mut InstructionDescriptorMap<'a>,
    opcode: u8,
    mnemonic: &'a str,
    addressing_mode: AddressingMode,
) {
    descriptors[opcode as usize] = Some(InstructionDescriptor {
        mnemonic,
        addressing_mode,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use ya6502::cpu_with_code;
    use ya6502::test_utils::cpu_with_program;

    fn disassembled(
        address: &str,
        instruction_bytes: &str,
        instruction: &str,
    ) -> DisassembledInstruction {
        DisassembledInstruction {
            address: address.to_string(),
            instruction_bytes: instruction_bytes.to_string(),
            instruction: instruction.to_string(),
        }
    }

    #[test]
    fn memory_stream_reading_bytes() {
        let cpu = cpu_with_program(&[0x54, 0x45]);
        let mut ms = MemoryStream::new(&cpu, 0xF000);
        assert_eq!(ms.read_byte(), 0x54);
        assert_eq!(ms.read_byte(), 0x45);
    }

    #[test]
    fn memory_stream_reading_words() {
        let cpu = cpu_with_program(&[0x6A, 0xC9, 0x12, 0x67]);
        let mut ms = MemoryStream::new(&cpu, 0xF000);
        assert_eq!(ms.read_word(), 0xC96A);
        assert_eq!(ms.read_word(), 0x6712);
    }

    #[test]
    fn memory_stream_address_wrapping() {
        let mut cpu = cpu_with_program(&[]);
        cpu.mut_memory().bytes[0xFFFE..=0xFFFF].copy_from_slice(&[0x01, 0x02]);
        cpu.mut_memory().bytes[0x0000..=0x0002].copy_from_slice(&[0x03, 0x04, 0x05]);

        let mut ms = MemoryStream::new(&cpu, 0xFFFF);
        assert_eq!(ms.read_byte(), 0x02);
        assert_eq!(ms.read_byte(), 0x03);

        let mut ms = MemoryStream::new(&cpu, 0xFFFE);
        assert_eq!(ms.read_word(), 0x0201);
        assert_eq!(ms.read_word(), 0x0403);

        let mut ms = MemoryStream::new(&cpu, 0xFFFF);
        assert_eq!(ms.read_word(), 0x0302);
        assert_eq!(ms.read_word(), 0x0504);
    }

    #[test]
    fn seek_at_origin() {
        let cpu = cpu_with_program(&[]);
        assert_eq!(seek_instruction(&cpu, 0x483A, 0), 0x483A);
        assert_eq!(seek_instruction(&cpu, 0xA384, 0), 0xA384);
    }

    #[test]
    fn seek_forward() {
        let cpu = cpu_with_code! {
                inx
                lda #0x2B
                sta abs 0x1234
        };
        assert_eq!(seek_instruction(&cpu, 0xF000, 1), 0xF001);
        assert_eq!(seek_instruction(&cpu, 0xF000, 2), 0xF003);
        assert_eq!(seek_instruction(&cpu, 0xF000, 3), 0xF006);
    }

    #[test]
    fn seek_forward_unknown_instruction() {
        let mut cpu = cpu_with_code! {
                lda #111  // 0xF000
                nop       // 0xF002
                lda #222  // 0xF003
                nop       // 0xF005
        };
        cpu.mut_memory().bytes[0xF002] = 0x02;
        cpu.mut_memory().bytes[0xF005] = 0x02;

        assert_eq!(seek_instruction(&cpu, 0xF000, 2), 0xF003);
        assert_eq!(seek_instruction(&cpu, 0xF000, 4), 0xF006);
    }

    #[test]
    fn seek_backward() {
        let cpu = cpu_with_code! {
                inx
                lda #0x2B
                sta abs 0x1234
        };
        assert_eq!(seek_instruction(&cpu, 0xF006, -1), 0xF003);
        assert_eq!(seek_instruction(&cpu, 0xF006, -2), 0xF001);
        assert_eq!(seek_instruction(&cpu, 0xF006, -3), 0xF000);
    }

    #[test]
    fn seek_backward_ambiguous() {
        let cpu = cpu_with_code! {
                // 0xEA == NOP
                inx             // 0xF000
                lda 0xEA        // 0xF001
                lda 0xEA        // 0xF003
                lda abs 0xEAEA  // 0xF005
                                // 0xF008
        };

        // Interpret 1 instruction as NOP
        assert_eq!(seek_instruction(&cpu, 0xF005, -1), 0xF004);
        // Interpret 2 instructions as NOP, LDA $EA
        assert_eq!(seek_instruction(&cpu, 0xF005, -2), 0xF002);
        // Interpret 3 instructions as INX, LDA $EA, LDA $EA
        assert_eq!(seek_instruction(&cpu, 0xF005, -3), 0xF000);
        // Interpret 3 instructions as NOP, LDA $EA, LDA $EAEA
        assert_eq!(seek_instruction(&cpu, 0xF008, -3), 0xF002);
    }

    #[test]
    fn seek_backward_unknown_instruction() {
        let mut cpu = cpu_with_code! {
            nop
            nop
            stx abs 0x2B2B
        };
        cpu.mut_memory().bytes[0xF001] = 0x2B;

        // 0xF001 should be preferred to 0xF003, since it has 1 unknown
        // instruction less.
        assert_eq!(seek_instruction(&cpu, 0xF005, -2), 0xF001);
    }

    #[test]
    fn seek_backward_impossible_1() {
        let cpu = cpu_with_code! {
            nop
            stx 0x2B
        };

        // There's no way to land on 0xF003 (the last byte of the stx
        // instruction). In such case, we expect the stx instruction to be
        // interpreted entirely as data.
        assert_eq!(seek_instruction(&cpu, 0xF002, -2), 0xF000);
    }

    #[test]
    fn seek_backward_impossible_2() {
        let cpu = cpu_with_code! {
            nop
            stx abs 0x2B2B
        };
        assert_eq!(seek_instruction(&cpu, 0xF003, -3), 0xF000);
    }

    #[test]
    fn seek_backward_with_wrapping() {
        let mut cpu = cpu_with_program(&[]);
        cpu.mut_memory().bytes[0xFFFF] = 0xEA;
        assert_eq!(seek_instruction(&cpu, 0x0000, -1), 0xFFFF);

        let mut cpu = cpu_with_program(&[]);
        // LDA $12
        cpu.mut_memory().bytes[0xFFFF] = 0xA5;
        cpu.mut_memory().bytes[0x0000] = 0x12;
        assert_eq!(seek_instruction(&cpu, 0x0001, -1), 0xFFFF);
    }

    #[test]
    fn disassemble_no_offset() {
        let cpu = cpu_with_code! {
                lda 0x45
            loop:
                ldx #0x4
                sta abs 0xBEEF,x
                dex
                bne loop
        };

        assert_eq!(disassemble(&cpu, 0xF000, 0xF000, 0, 0), vec![]);
        assert_eq!(
            disassemble(&cpu, 0xF000, 0xF000, 0, 5),
            vec![
                disassembled("0xF000", "A5 45", "LDA $45"),
                disassembled("0xF002", "A2 04", "LDX #$04",),
                disassembled("0xF004", "9D EF BE", "STA $BEEF,X",),
                disassembled("0xF007", "CA", "DEX",),
                disassembled("0xF008", "D0 F8", "BNE -8",)
            ]
        );
        assert_eq!(
            disassemble(&cpu, 0xF002, 0xF002, 0, 2),
            vec![
                disassembled("0xF002", "A2 04", "LDX #$04",),
                disassembled("0xF004", "9D EF BE", "STA $BEEF,X",),
            ]
        );
    }

    #[test]
    fn disassemble_unknown_instruction() {
        let cpu = cpu_with_program(&[0xEA, 0x67, 0xEA]);
        assert_eq!(
            disassemble(&cpu, 0xF000, 0xF000, 0, 3),
            vec![
                disassembled("0xF000", "EA", "NOP",),
                disassembled("0xF001", "67", "",),
                disassembled("0xF002", "EA", "NOP",),
            ]
        );
    }

    #[test]
    fn disassemble_with_offset() {
        let cpu = cpu_with_code! {
                lda 0x45
                sta 0xEA
                sta 0xAE
        };

        assert_eq!(
            disassemble(&cpu, 0xF002, 0xF000, 0, 3),
            vec![
                disassembled("0xF000", "A5 45", "LDA $45",),
                disassembled("0xF002", "85 EA", "STA $EA",),
                disassembled("0xF004", "85 AE", "STA $AE",),
            ]
        );
        assert_eq!(
            disassemble(&cpu, 0xF003, 0xF000, 0, 4),
            vec![
                disassembled("0xF000", "A5 45", "LDA $45",),
                disassembled("0xF002", "85", "",),
                disassembled("0xF003", "EA", "NOP",),
                disassembled("0xF004", "85 AE", "STA $AE",),
            ]
        )
    }

    #[test]
    fn disassemble_with_margin() {
        let cpu = cpu_with_code! {
                ldx 0x45
                inx
                stx 0x46
        };
        assert_eq!(
            disassemble(&cpu, 0xF003, 0xF000, 1, 2),
            vec![
                disassembled("0xF002", "E8", "INX",),
                disassembled("0xF003", "86 46", "STX $46",),
            ]
        )
    }

    /// Tests some incredibly rare edge cases that occur when we perform
    /// wrapping arithmetic operations close to the wrapping point.
    #[test]
    fn disassemble_jumping_through_origin_with_address_wrapping() {
        let mut cpu = cpu_with_program(&[]);
        // STA $EA
        cpu.mut_memory().bytes[0xFFFE] = 0x85;
        cpu.mut_memory().bytes[0xFFFF] = 0xEA;
        assert_eq!(
            disassemble(&cpu, 0xFFFF, 0xFFFE, 0, 1),
            vec![disassembled("0xFFFE", "85", "")]
        );

        let mut cpu = cpu_with_program(&[]);
        cpu.mut_memory().bytes[0xFFFF] = 0x85;
        cpu.mut_memory().bytes[0x0000] = 0xEA;
        assert_eq!(
            disassemble(&cpu, 0x0000, 0xFFFF, 0, 1),
            vec![disassembled("0xFFFF", "85", "")]
        );
    }
}
