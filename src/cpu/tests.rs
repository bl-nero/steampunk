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

macro_rules! cpu_with_code {
    ($($tokens:tt)*) => {
        cpu_with_program(&assemble6502!({
            start: 0xF000,
            code: {$($tokens)*}
        }))
    };
}

fn reversed_stack(cpu: &Cpu<SimpleRam>) -> Vec<u8> {
    cpu.memory.bytes[(cpu.stack_pointer() as usize + 1)..=0x1FF]
        .iter()
        .copied()
        .rev()
        .collect()
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
fn nop() {
    let mut cpu = cpu_with_code! {
        lda #0xFF
        nop
        sta 1
    };
    cpu.ticks(4).unwrap();
    assert_eq!(cpu.memory.bytes[1], 0);
    cpu.ticks(3).unwrap();
    assert_eq!(cpu.memory.bytes[1], 0xFF);
}

#[test]
fn lda_sta() {
    let mut cpu = cpu_with_code! {
            lda #65
            sta 4
            lda #73
            sta 4
            lda #12
            sta 5
            lda 4
            sta 6
            lda abs 0xF002  // should load the STA opcode
            sta abs 0xABCD
    };
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    cpu.ticks(14).unwrap();
    assert_eq!(cpu.memory.bytes[4..7], [73, 12, 73]);
    assert_eq!(cpu.memory.bytes[0xABCD], opcodes::STA_ZP);
}

#[test]
fn ldx_stx() {
    let mut cpu = cpu_with_code! {
            ldx #65
            stx 4
            ldx #73
            stx 4
            ldx #12
            stx 5
            ldx 4
            stx 6
            ldx abs 0xF002  // should load the STX opcode
            stx abs 0xABCD
    };
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    cpu.ticks(14).unwrap();
    assert_eq!(cpu.memory.bytes[4..7], [73, 12, 73]);
    assert_eq!(cpu.memory.bytes[0xABCD], opcodes::STX_ZP);
}

#[test]
fn ldy_sty() {
    let mut cpu = cpu_with_code! {
            ldy #65
            sty 4
            ldy #73
            sty 4
            ldy #12
            sty 5
            ldy 4
            sty 6
            ldy abs 0xF002  // should load the STY opcode
            sty abs 0xABCD
    };
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    cpu.ticks(14).unwrap();
    assert_eq!(cpu.memory.bytes[4..7], [73, 12, 73]);
    assert_eq!(cpu.memory.bytes[0xABCD], opcodes::STY_ZP);
}

#[test]
fn multiple_registers() {
    let mut cpu = cpu_with_code! {
            lda #10
            ldx #20
            sta 0
            stx 1
    };
    cpu.ticks(10).unwrap();
    assert_eq!(cpu.memory.bytes[0..2], [10, 20]);
}

#[test]
fn loading_addressing_modes() {
    let mut cpu = cpu_with_code! {
            ldx #0xFF
            txs
            lda abs 0xF002
            pha
    };
    cpu.ticks(11).unwrap();
    assert_eq!(reversed_stack(&cpu), [opcodes::TXS]);
}

#[test]
fn storing_addressing_modes() {
    let mut cpu = cpu_with_code! {
            ldx #5
            lda #42
            ldy #100
        loop:
            sta 0xFC,x
            sty 0x02,x
            dex
            bne loop
            sta abs 0xABCD
    };
    cpu.ticks(6 + 5 * 13 + 4).unwrap();
    assert_eq!(cpu.memory.bytes[0xFC..0x100], [0, 42, 42, 42]);
    assert_eq!(
        cpu.memory.bytes[0x00..0x09],
        [42, 42, 0, 100, 100, 100, 100, 100, 0]
    );
    assert_eq!(cpu.memory.bytes[0xABCD], 42);
}

#[test]
fn cmp() {
    let mut program = assemble6502! ({
        start: 0xF000,
        code: {
                lda #7

                cmp #6
                beq fail
                bcc fail
                bmi fail
                sta 30

                cmp #7
                bne fail
                bcc fail
                bmi fail
                sta 31

                cmp #8
                beq fail
                bcs fail
                bpl fail
                sta 32

                cmp #(-7i8 as u8)
                beq fail
                bcs fail
                bmi fail

                sta 33
                nop  // to be replaced
            fail:
                jmp fail
        }
    });
    // Deliberately inject HLT1 instead of NOP to make sure we never reach that
    // place and test timing.
    program[program.len() - 4] = opcodes::HLT1;
    let mut cpu = cpu_with_program(&program);
    cpu.ticks(2 + 4 * 11).unwrap();
    assert_eq!(cpu.memory.bytes[30..=33], [7, 7, 7, 7]);
}

#[test]
fn cpx_cpy() {
    let mut cpu = cpu_with_code! {
            ldx #0xFF
            txs
            ldy #10
            cpx #6
            php
            cpy #25
            php
    };
    cpu.ticks(16).unwrap();
    let mask = flags::C | flags::Z | flags::N;
    assert_eq!(cpu.memory.bytes[0x1FF] & mask, flags::N | flags::C);
    assert_eq!(cpu.memory.bytes[0x1FE] & mask, flags::N);
}

#[test]
fn bit() {
    let mut cpu = cpu_with_code! {
            ldx #0xFE
            txs
            plp
            lda #0b1000_0001
            sta 0x01
            lda #0b0100_0001
            sta 0x02
            lda #0b0011_1110
            sta 0x03
            lda #0b1111_1110
            sta abs 0x1234
            lda #0b0000_0001
            bit 0x01
            php
            bit 0x02
            php
            bit 0x03
            php
            bit abs 0x1234
            php
    };
    cpu.ticks(56).unwrap();
    assert_eq!(
        reversed_stack(&cpu),
        &[
            flags::UNUSED | flags::N,
            flags::UNUSED | flags::V,
            flags::UNUSED | flags::Z,
            flags::UNUSED | flags::N | flags::V | flags::Z,
        ]
    );
}

#[test]
fn adc_sbc() {
    let mut cpu = cpu_with_code! {
            ldx #0xFE
            txs
            plp
            lda #0x45

            adc #0x2A
            pha
            php

            adc #0x20
            pha
            php

            adc #0xAC
            pha
            php

            adc #0x01
            pha
            php

            sbc #0x45
            pha
            php

            sbc #0x7F
            pha
            php

            sbc #0xBF
            pha
            php
    };
    cpu.ticks(10 + 7 * 8).unwrap();

    assert_eq!(
        reversed_stack(&cpu),
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
fn adc_sbc_decimal_mode() {
    let mut cpu = cpu_with_code! {
            ldx #0xFE
            txs
            plp
            sed
            lda #0x45

            adc #0x68
            pha
            php

            adc #0x16
            pha
            php

            sbc #0x25
            pha
            php

            sbc #0x56
            pha
            php
    };
    cpu.ticks(12 + 4 * 8).unwrap();

    assert_eq!(
        reversed_stack(&cpu),
        [
            0x13,
            flags::UNUSED | flags::D | flags::C,
            0x30,
            flags::UNUSED | flags::D,
            0x04,
            flags::UNUSED | flags::D | flags::C,
            0x48,
            flags::UNUSED | flags::D,
        ]
    );
}

#[test]
fn adc_sbc_addressing_modes() {
    let mut cpu = cpu_with_code! {
            ldx #0xFE
            txs
            plp
            ldx #15
            stx 5
            inx
            stx 6

            lda #20
            adc 5
            pha
            sec
            sbc 6
            pha
    };
    cpu.ticks(18 + 16).unwrap();
    assert_eq!(reversed_stack(&cpu), [35, 19]);
}

#[test]
fn logical_operations() {
    let mut cpu = cpu_with_code! {
            ldx #0xFF
            txs
            lda #0b0000_1111
            and #0b1100_1100
            pha
    };
    cpu.ticks(11).unwrap();
    assert_eq!(reversed_stack(&cpu), [0b0000_1100])
}

#[test]
fn shifting() {
    let mut cpu = cpu_with_code! {
            sec
            lda #0b0101_0000

            asl a
        stop1:
            bcs stop1
            sta 0x01

            asl 0x01
        stop2:
            bcc stop2
            sta 0x02

            ldx #1
            asl 0x01,x
    };
    cpu.ticks(4 + 7 + 10 + 8).unwrap();
    assert_eq!(cpu.memory.bytes[1..=2], [0b0100_0000, 0b0100_0000]);
}

#[test]
fn inc_dec() {
    let mut cpu = cpu_with_code! {
            inc 10
            inc 10
            dec 11
            dec 11
    };
    cpu.ticks(20).unwrap();
    assert_eq!(cpu.memory.bytes[10..=11], [2, -2 as i8 as u8]);
}

#[test]
fn inx_dex() {
    let mut cpu = cpu_with_code! {
            ldx #0xFE
            inx
            stx 5
            inx
            stx 6
            inx
            stx 7
            dex
            stx 8
            dex
            stx 9
    };
    cpu.ticks(27).unwrap();
    assert_eq!(cpu.memory.bytes[5..10], [0xFF, 0x00, 0x01, 0x00, 0xFF]);
}

#[test]
fn iny_dey() {
    let mut cpu = cpu_with_code! {
            ldy #0xFE
            iny
            sty 5
            iny
            sty 6
            iny
            sty 7
            dey
            sty 8
            dey
            sty 9
    };
    cpu.ticks(27).unwrap();
    assert_eq!(cpu.memory.bytes[5..10], [0xFF, 0x00, 0x01, 0x00, 0xFF]);
}

#[test]
fn tya() {
    let mut cpu = cpu_with_code! {
            ldy #15
            tya
            sta 1
    };
    cpu.ticks(7).unwrap();
    assert_eq!(cpu.memory.bytes[0x01], 15);
}

#[test]
fn tax() {
    let mut cpu = cpu_with_code! {
            lda #13
            tax
            stx 0x01
    };
    cpu.ticks(7).unwrap();
    assert_eq!(cpu.memory.bytes[0x01], 13);
}

#[test]
fn txa() {
    let mut cpu = cpu_with_code! {
            ldx #43
            txa
            sta 0x01
    };
    cpu.ticks(7).unwrap();
    assert_eq!(cpu.memory.bytes[0x01], 43);
}

#[test]
fn flag_manipulation() {
    let mut cpu = cpu_with_code! {
            ldx #0xFE
            txs
            plp

            sei
            sec
            lda #0
            php

            ldx #0xFF
            php

            cli
            ldy #0x01
            php

            clc
            php
    };
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
    let mut cpu = cpu_with_code! {
            ldx #5
            lda #5
        loop:
            sta 9,x
            dex
            bne loop
            stx 12
    };
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
    let mut cpu = cpu_with_code! {
            ldx #1
        loop:
            stx 9
            inx
            jmp loop
    };

    cpu.ticks(13).unwrap();
    assert_eq!(cpu.memory.bytes[9], 2);
    cpu.ticks(8).unwrap();
    assert_eq!(cpu.memory.bytes[9], 3);
}

#[test]
fn subroutines_and_stack() {
    let mut cpu = cpu_with_code! {
        // Main program. Call subroutine A to store 6 at 25. Then call
        // subroutine B to store 7 at 28 and 6 at 26. Finally, store the 10
        // loaded to A in the beginning at 30. Duration: 25 cycles.
            ldx #0xFF
            txs
            lda #10
            ldx #5
            jsr sub_a
            inx
            jsr sub_b
            sta 30
            nop  // to be replaced

        // Subroutine A: store 6 at 20+X. Duration: 19 cycles.
        sub_a:
            pha
            lda #6
            sta 20,x
            pla
            rts
            nop  // to be replaced

        // Subroutine B: store 6 at 20+X and 7 at 22+X. Duration: 25 cycles.
        sub_b:
            pha
            lda #7
            jsr sub_a
            sta 22,x
            pla
            rts
            nop  // to be replaced
    };
    cpu.mut_memory().bytes[0xF010] = opcodes::HLT1;
    cpu.mut_memory().bytes[0xF018] = opcodes::HLT1;
    cpu.mut_memory().bytes[0xF023] = opcodes::HLT1;

    cpu.ticks(25 + 19 + 25 + 19).unwrap();
    assert_eq!(cpu.memory.bytes[24..32], [0, 6, 6, 0, 7, 0, 10, 0]);
}

#[test]
fn stack_wrapping() {
    let mut cpu = cpu_with_code! {
            ldx #1
            txs

            txa
            pha
            tsx
            txa
            pha
            tsx
            txa
            pha
            tsx

            txa
            pla
            pla
            pla
            sta 5
    };
    cpu.ticks(4 + 3 * 7 + 17).unwrap();
    assert_eq!(cpu.memory.bytes[0x1FF], 0xFF);
    assert_eq!(cpu.memory.bytes[0x100..0x102], [0, 1]);
    assert_eq!(cpu.memory.bytes[5], 1);
}

#[test]
fn stack_wrapping_with_subroutines() {
    let mut cpu = cpu_with_code! {
            ldx #0x00
            txs
            jsr subroutine
            sta 20
            nop  // to be replaced
        subroutine:
            lda #34
            rts
    };
    cpu.mut_memory().bytes[0xF008] = opcodes::HLT1;
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
    let mut cpu = cpu_with_code! {
            clc
            cld
            ldx #1
            lda #42
        loop:
            sta 0,x
            adc #64
            inx
            jmp loop
    };
    b.iter(|| {
        reset(&mut cpu);
        cpu.ticks(1000).unwrap();
    });
}