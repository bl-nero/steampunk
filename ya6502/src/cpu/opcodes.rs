pub const NOP: u8 = 0xEA;
pub const LDA_IMM: u8 = 0xA9;
pub const LDX_IMM: u8 = 0xA2;
pub const LDY_IMM: u8 = 0xA0;
pub const LDA_ZP: u8 = 0xA5;
pub const LDX_ZP: u8 = 0xA6;
pub const LDY_ZP: u8 = 0xA4;
pub const LDA_ZP_X: u8 = 0xB5;
pub const LDA_ABS: u8 = 0xAD;
pub const LDX_ABS: u8 = 0xAE;
pub const LDY_ABS: u8 = 0xAC;
pub const LDA_ABS_X: u8 = 0xBD;
pub const LDA_ABS_Y: u8 = 0xB9;
pub const LDA_X_INDIR: u8 = 0xA1;
pub const LDA_INDIR_Y: u8 = 0xB1;

pub const STA_ZP: u8 = 0x85;
pub const STX_ZP: u8 = 0x86;
pub const STY_ZP: u8 = 0x84;
pub const STA_ZP_X: u8 = 0x95;
pub const STY_ZP_X: u8 = 0x94;
pub const STA_ABS: u8 = 0x8D;
pub const STX_ABS: u8 = 0x8E;
pub const STY_ABS: u8 = 0x8C;
pub const STA_ABS_X: u8 = 0x9D;
pub const STA_ABS_Y: u8 = 0x99;
pub const STA_X_INDIR: u8 = 0x81;
pub const STA_INDIR_Y: u8 = 0x91;

pub const AND_IMM: u8 = 0x29;
pub const AND_ZP: u8 = 0x25;
pub const AND_ZP_X: u8 = 0x35;
pub const AND_ABS: u8 = 0x2D;
pub const AND_ABS_X: u8 = 0x3D;
pub const AND_ABS_Y: u8 = 0x39;
pub const AND_X_INDIR: u8 = 0x21;
pub const AND_INDIR_Y: u8 = 0x31;

pub const ORA_IMM: u8 = 0x09;
pub const ORA_ZP: u8 = 0x05;
pub const ORA_ZP_X: u8 = 0x15;
pub const ORA_ABS: u8 = 0x0D;
pub const ORA_ABS_X: u8 = 0x1D;
pub const ORA_ABS_Y: u8 = 0x19;
pub const ORA_X_INDIR: u8 = 0x01;
pub const ORA_INDIR_Y: u8 = 0x11;

pub const EOR_IMM: u8 = 0x49;
pub const EOR_ZP: u8 = 0x45;

pub const ASL_A: u8 = 0x0A;
pub const ASL_ZP: u8 = 0x06;
pub const ASL_ZP_X: u8 = 0x16;
pub const ASL_ABS: u8 = 0x0E;
pub const LSR_A: u8 = 0x4A;
pub const LSR_ZP: u8 = 0x46;
pub const LSR_ZP_X: u8 = 0x56;
pub const LSR_ABS: u8 = 0x4E;
pub const ROL_A: u8 = 0x2A;
pub const ROL_ZP: u8 = 0x26;
pub const ROL_ZP_X: u8 = 0x36;
pub const ROL_ABS: u8 = 0x2E;
pub const ROR_A: u8 = 0x6A;
pub const ROR_ZP: u8 = 0x66;
pub const ROR_ZP_X: u8 = 0x76;
pub const ROR_ABS: u8 = 0x6E;

pub const CMP_IMM: u8 = 0xC9;
pub const CMP_ZP: u8 = 0xC5;
pub const CMP_ZP_X: u8 = 0xD5;
pub const CMP_ABS: u8 = 0xCD;
pub const CMP_ABS_X: u8 = 0xDD;
pub const CMP_ABS_Y: u8 = 0xD9;
pub const CMP_X_INDIR: u8 = 0xC1;
pub const CMP_INDIR_Y: u8 = 0xD1;

pub const CPX_IMM: u8 = 0xE0;
pub const CPX_ZP: u8 = 0xE4;
pub const CPY_IMM: u8 = 0xC0;
pub const CPY_ZP: u8 = 0xC4;

pub const BIT_ZP: u8 = 0x24;
pub const BIT_ABS: u8 = 0x2C;

pub const ADC_IMM: u8 = 0x69;
pub const ADC_ZP: u8 = 0x65;
pub const ADC_ZP_X: u8 = 0x75;
pub const ADC_ABS: u8 = 0x6D;
pub const ADC_ABS_X: u8 = 0x7D;
pub const ADC_ABS_Y: u8 = 0x79;

pub const SBC_IMM: u8 = 0xE9;
pub const SBC_ZP: u8 = 0xE5;
pub const SBC_ZP_X: u8 = 0xF5;
pub const SBC_ABS: u8 = 0xED;
pub const SBC_ABS_X: u8 = 0xFD;
pub const SBC_ABS_Y: u8 = 0xF9;

pub const INC_ZP: u8 = 0xE6;
pub const INC_ZP_X: u8 = 0xF6;
pub const DEC_ZP: u8 = 0xC6;
pub const DEC_ZP_X: u8 = 0xD6;

pub const INX: u8 = 0xE8;
pub const INY: u8 = 0xC8;
pub const DEX: u8 = 0xCA;
pub const DEY: u8 = 0x88;

pub const TAX: u8 = 0xAA;
pub const TAY: u8 = 0xA8;
pub const TXA: u8 = 0x8A;
pub const TYA: u8 = 0x98;
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
pub const CLV: u8 = 0xB8;

pub const BEQ: u8 = 0xF0;
pub const BNE: u8 = 0xD0;
pub const BCC: u8 = 0x90;
pub const BCS: u8 = 0xB0;
pub const BPL: u8 = 0x10;
pub const BMI: u8 = 0x30;
pub const BVS: u8 = 0x70;
pub const BVC: u8 = 0x50;

pub const JMP_ABS: u8 = 0x4C;
pub const JSR: u8 = 0x20;
pub const RTS: u8 = 0x60;

pub const HLT1: u8 = 0x02;
