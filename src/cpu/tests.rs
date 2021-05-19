#![cfg(test)]
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
fn storing_addressing_modes() {
    let mut cpu = cpu_with_program(&[
        opcodes::LDX_IMM,
        5,
        opcodes::LDA_IMM,
        42,
        opcodes::LDY_IMM,
        100,
        // ----
        opcodes::STA_ZP_X,
        0xFC,
        opcodes::STY_ZP_X,
        0x02,
        opcodes::DEX,
        opcodes::BNE,
        -7i8 as u8,
        // ----
        opcodes::STA_ABS,
        0xCD,
        0xAB,
    ]);
    cpu.ticks(6 + 5 * 13 + 4).unwrap();
    assert_eq!(cpu.memory.bytes[0xFC..0x100], [0, 42, 42, 42]);
    assert_eq!(
        cpu.memory.bytes[0x00..0x09],
        [42, 42, 0, 100, 100, 100, 100, 100, 0]
    );
    assert_eq!(cpu.memory.bytes[0xABCD], 42);
}

#[test]
fn inc_dec() {
    let mut cpu = cpu_with_program(&[
        opcodes::INC_ZP,
        10,
        opcodes::INC_ZP,
        10,
        opcodes::DEC_ZP,
        11,
        opcodes::DEC_ZP,
        11,
    ]);
    cpu.ticks(20).unwrap();
    assert_eq!(cpu.memory.bytes[10..=11], [2, -2 as i8 as u8]);
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
fn cmp() {
    let mut cpu = cpu_with_program(&[
        opcodes::LDA_IMM,
        7,
        // ----
        opcodes::CMP_IMM,
        6,
        opcodes::BEQ,
        37,
        opcodes::BCC,
        35,
        opcodes::BMI,
        33,
        opcodes::STA_ZP,
        30,
        // ----
        opcodes::CMP_IMM,
        7,
        opcodes::BNE,
        27,
        opcodes::BCC,
        25,
        opcodes::BMI,
        23,
        opcodes::STA_ZP,
        31,
        // ----
        opcodes::CMP_IMM,
        8,
        opcodes::BEQ,
        17,
        opcodes::BCS,
        15,
        opcodes::BPL,
        13,
        opcodes::STA_ZP,
        32,
        // ----
        opcodes::CMP_IMM,
        -7i8 as u8,
        opcodes::BEQ,
        7,
        opcodes::BCS,
        5,
        opcodes::BMI,
        3,
        opcodes::STA_ZP,
        33,
        opcodes::HLT1, // This makes sure that we don't use too many cycles.
        // If the test fails, just loop and wait.
        opcodes::JMP_ABS,
        0x2B,
        0xF0,
    ]);
    cpu.ticks(2 + 4 * 11).unwrap();
    assert_eq!(cpu.memory.bytes[30..=33], [7, 7, 7, 7]);
}

#[test]
fn cpx_cpy() {
    let mut cpu = cpu_with_program(&[
        opcodes::LDX_IMM,
        0xFF,
        opcodes::TXS,
        opcodes::LDY_IMM,
        10,
        opcodes::CPX_IMM,
        6,
        opcodes::PHP,
        opcodes::CPY_IMM,
        25,
        opcodes::PHP,
    ]);
    cpu.ticks(16).unwrap();
    let mask = flags::C | flags::Z | flags::N;
    assert_eq!(cpu.memory.bytes[0x1FF] & mask, flags::N | flags::C);
    assert_eq!(cpu.memory.bytes[0x1FE] & mask, flags::N);
}

#[test]
fn adc_sbc() {
    let mut cpu = cpu_with_program(&[
        opcodes::LDX_IMM,
        0xFE,
        opcodes::TXS,
        opcodes::PLP,
        opcodes::LDA_IMM,
        0x45,
        opcodes::ADC_IMM,
        0x2A,
        opcodes::PHA,
        opcodes::PHP,
        opcodes::ADC_IMM,
        0x20,
        opcodes::PHA,
        opcodes::PHP,
        opcodes::ADC_IMM,
        0xAC,
        opcodes::PHA,
        opcodes::PHP,
        opcodes::ADC_IMM,
        0x01,
        opcodes::PHA,
        opcodes::PHP,
        opcodes::SBC_IMM,
        0x45,
        opcodes::PHA,
        opcodes::PHP,
        opcodes::SBC_IMM,
        0x7F,
        opcodes::PHA,
        opcodes::PHP,
        opcodes::SBC_IMM,
        0xBF,
        opcodes::PHA,
        opcodes::PHP,
    ]);
    cpu.ticks(10 + 7 * 8).unwrap();

    let reversed_stack: Vec<u8> = cpu.memory.bytes[0x1F2..=0x1FF]
        .iter()
        .copied()
        .rev()
        .collect();
    assert_eq!(
        reversed_stack,
        [
            0x6F,
            flags::UNUSED,
            0x8F,
            flags::UNUSED | flags::V | flags::N,
            0x3B,
            flags::UNUSED | flags::C | flags::V,
            0x3D,
            flags::UNUSED,
            0xF7,
            flags::UNUSED | flags::N,
            0x77,
            flags::UNUSED | flags::C | flags::V,
            0xB8,
            flags::UNUSED | flags::V | flags::N,
        ]
    );
}

#[test]
fn adc_sbc_addressing_modes() {
    let mut cpu = cpu_with_program(&[
        opcodes::LDX_IMM,
        0xFE,
        opcodes::TXS,
        opcodes::PLP,
        opcodes::LDX_IMM,
        15,
        opcodes::STX_ZP,
        0x05,
        opcodes::INX,
        opcodes::STX_ZP,
        0x06,
        // ----
        opcodes::LDA_IMM,
        20,
        opcodes::ADC_ZP,
        0x05,
        opcodes::PHA,
        opcodes::SEC,
        opcodes::SBC_ZP,
        0x06,
        opcodes::PHA,
    ]);
    cpu.ticks(18 + 16).unwrap();

    let reversed_stack: Vec<u8> = cpu.memory.bytes[0x1FE..=0x1FF]
        .iter()
        .copied()
        .rev()
        .collect();
    assert_eq!(reversed_stack, [35, 19]);
}

#[test]
fn tya() {
    let mut cpu = cpu_with_program(&[opcodes::LDY_IMM, 15, opcodes::TYA, opcodes::STA_ZP, 0x01]);
    cpu.ticks(7).unwrap();
    assert_eq!(cpu.memory.bytes[0x01], 15);
}

#[test]
fn tax() {
    let mut cpu = cpu_with_program(&[opcodes::LDA_IMM, 13, opcodes::TAX, opcodes::STX_ZP, 0x01]);
    cpu.ticks(7).unwrap();
    assert_eq!(cpu.memory.bytes[0x01], 13);
}

#[test]
fn txa() {
    let mut cpu = cpu_with_program(&[opcodes::LDX_IMM, 43, opcodes::TXA, opcodes::STA_ZP, 0x01]);
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
        opcodes::SEC,
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
        opcodes::CLC,
        opcodes::PHP,
    ]);
    cpu.ticks(34).unwrap();
    assert_eq!(
        cpu.memory.bytes[0x1FC..0x200],
        [
            flags::UNUSED,
            flags::C | flags::UNUSED,
            flags::C | flags::I | flags::N | flags::UNUSED,
            flags::C | flags::I | flags::Z | flags::UNUSED,
        ]
    );
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
    cpu.ticks(1).unwrap();
    assert_eq!(cpu.memory.bytes[20], 10);
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
fn stack_wrapping_with_subroutines() {
    let mut cpu = cpu_with_program(&[
        opcodes::LDX_IMM,
        0x00,
        opcodes::TXS,
        opcodes::JSR,
        0x09,
        0xF0,
        opcodes::STA_ZP,
        20,
        opcodes::HLT1,
        // Subroutine. Address: $F009.
        opcodes::LDA_IMM,
        34,
        opcodes::RTS,
    ]);
    cpu.ticks(21).unwrap();
    assert_eq!(cpu.memory.bytes[20], 34);
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
        opcodes::CLC,
        opcodes::LDX_IMM,
        1,
        opcodes::LDA_IMM,
        42,
        opcodes::STA_ZP_X,
        0x00,
        opcodes::ADC_IMM,
        64,
        opcodes::INX,
        opcodes::JMP_ABS,
        0x05,
        0xf0,
    ]));
    let mut cpu = Cpu::new(memory);
    b.iter(|| {
        reset(&mut cpu);
        cpu.ticks(1000).unwrap();
    });
}
