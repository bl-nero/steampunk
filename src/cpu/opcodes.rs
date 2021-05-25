pub const LDA_IMM: u8 = 0xA9;
pub const LDX_IMM: u8 = 0xA2;
pub const LDY_IMM: u8 = 0xA0;
pub const LDX_ZP: u8 = 0xA6;
pub const LDA_ABS: u8 = 0xAD;

pub const STA_ZP: u8 = 0x85;
pub const STX_ZP: u8 = 0x86;
pub const STY_ZP: u8 = 0x84;
pub const STA_ZP_X: u8 = 0x95;
pub const STY_ZP_X: u8 = 0x94;
pub const STA_ABS: u8 = 0x8D;

pub const CMP_IMM: u8 = 0xC9;
pub const CPX_IMM: u8 = 0xE0;
pub const CPY_IMM: u8 = 0xC0;

pub const ADC_IMM: u8 = 0x69;
pub const ADC_ZP: u8 = 0x65;
pub const SBC_IMM: u8 = 0xE9;
pub const SBC_ZP: u8 = 0xE5;

pub const INC_ZP: u8 = 0xE6;
pub const DEC_ZP: u8 = 0xC6;
pub const INX: u8 = 0xE8;
pub const INY: u8 = 0xC8;
pub const DEX: u8 = 0xCA;
pub const DEY: u8 = 0x88;

pub const TYA: u8 = 0x98;
pub const TAX: u8 = 0xAA;
pub const TXA: u8 = 0x8A;
pub const TXS: u8 = 0x9A;
pub const TSX: u8 = 0xBA;

pub const PHP: u8 = 0x08;
pub const PHA: u8 = 0x48;
pub const PLP: u8 = 0x28;
pub const PLA: u8 = 0x68;

pub const SEI: u8 = 0x78;
pub const CLI: u8 = 0x58;
pub const SED: u8 = 0xF8;
pub const CLD: u8 = 0xD8;
pub const SEC: u8 = 0x38;
pub const CLC: u8 = 0x18;

pub const BEQ: u8 = 0xF0;
pub const BNE: u8 = 0xD0;
pub const BCC: u8 = 0x90;
pub const BCS: u8 = 0xB0;
pub const BPL: u8 = 0x10;
pub const BMI: u8 = 0x30;

pub const JMP_ABS: u8 = 0x4C;
pub const JSR: u8 = 0x20;
pub const RTS: u8 = 0x60;

pub const HLT1: u8 = 0x02;
