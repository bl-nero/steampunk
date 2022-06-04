#![cfg(test)]

extern crate test;

use super::*;
use crate::cpu_with_code;
use crate::memory::Ram;
use crate::test_utils::cpu_with_program;
use crate::test_utils::reset;
use test::Bencher;

fn reversed_stack(cpu: &Cpu<Ram>) -> Vec<u8> {
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
            // (15 cycles)

            lda 4
            clc
            cld
            adc #1
            sta 6
            // (12 cycles)

            ldx #2
        loop1:
            lda 4,x
            sta 7,x
            dex
            bpl loop1
            // (2 + 10 * 3 + 3 * 2 + 2 cycles)

            // Copy arguments of first three instructions from this program.
            lda abs 0xF001
            sta abs 0xABC0
            lda abs 0xF003
            sta abs 0xABC1
            lda abs 0xF005
            sta abs 0xABC2
            // (8 * 3 cycles)

            ldx #2
        loop2:
            lda abs 0xABC0,x
            sta abs 0xABC3,x
            dex
            bpl loop2
            // (2 + 11 * 3 + 3 * 2 + 2 cycles)

            ldy #2
        loop3:
            lda abs 0xABC0,y
            sta abs 0xABC6,y
            dey
            bpl loop3
            // (2 + 11 * 3 + 3 * 2 + 2 cycles)

            ldx #4
        loop4:
            lda (10,x)
            sta (20,x)
            dex
            dex
            bpl loop4
            // (2 + 16 * 3 + 3 * 2 + 2 cycles)

            ldy #2
        loop5:
            lda (12),y
            sta (26),y
            dey
            bpl loop5
            // (2 + 13 * 3 + 3 * 2 + 2 cycles)
    };
    // Prepare address vectors for the (X, indirect) addressing.
    cpu.mut_memory().bytes[10..=15].copy_from_slice(&[0xC1, 0xAB, 0xC2, 0xAB, 0xC3, 0xAB]);
    cpu.mut_memory().bytes[20..=27]
        .copy_from_slice(&[0xCB, 0xAB, 0xCA, 0xAB, 0xC9, 0xAB, 0xCC, 0xAB]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    cpu.ticks(
        12 + (2 + 10 * 3 + 3 * 2 + 2)
            + (8 * 3)
            + 2 * (2 + 11 * 3 + 3 * 2 + 2)
            + (2 + 16 * 3 + 3 * 2 + 2)
            + (2 + 13 * 3 + 3 * 2 + 2),
    )
    .unwrap();
    assert_eq!(cpu.memory.bytes[4..=9], [73, 12, 74, 73, 12, 74]);
    assert_eq!(
        cpu.memory.bytes[0xABC0..=0xABCE],
        [65, 4, 73, 65, 4, 73, 65, 4, 73, 65, 73, 4, 73, 65, 4]
    );
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

            ldx abs 0x3456
            stx abs 0xABCD

            ldy #2
            ldx 14,y
            stx 24,y

            ldy #7
            ldx abs 0x3450,y
            stx 7
    };
    // Prepare test data.
    cpu.mut_memory().bytes[16..=16].copy_from_slice(&[34]);
    cpu.mut_memory().bytes[0x3456..=0x3457].copy_from_slice(&[17, 56]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    cpu.ticks(14).unwrap();
    assert_eq!(cpu.memory.bytes[4..7], [73, 12, 73]);
    assert_eq!(cpu.memory.bytes[0xABCD], 17);
    cpu.ticks(10).unwrap();
    assert_eq!(cpu.memory.bytes[26], 34);
    cpu.ticks(9).unwrap();
    assert_eq!(cpu.memory.bytes[7], 56);
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
            ldy abs 0x3456
            sty abs 0xABCD

            ldx #2
            ldy 14,x
            sty 24,x

            ldx #7
            ldy abs 0x3450,x
            sty 7
    };
    // Prepare test data.
    cpu.mut_memory().bytes[16..=16].copy_from_slice(&[34]);
    cpu.mut_memory().bytes[0x3456..=0x3457].copy_from_slice(&[17, 56]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [65, 0]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [73, 0]);
    cpu.ticks(5).unwrap();
    assert_eq!(cpu.memory.bytes[4..6], [73, 12]);
    cpu.ticks(14).unwrap();
    assert_eq!(cpu.memory.bytes[4..7], [73, 12, 73]);
    assert_eq!(cpu.memory.bytes[0xABCD], 17);
    cpu.ticks(10).unwrap();
    assert_eq!(cpu.memory.bytes[26], 34);
    cpu.ticks(9).unwrap();
    assert_eq!(cpu.memory.bytes[7], 56);
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
fn storing_addressing_mode_quirks() {
    let mut cpu = cpu_with_code! {
            ldx #5
            lda #42
            ldy #100
        loop:
            sta 0xFC,x
            sty 0x02,x
            dex
            bne loop
    };
    cpu.ticks(6 + 5 * 13).unwrap();
    assert_eq!(cpu.memory.bytes[0xFC..0x100], [0, 42, 42, 42]);
    assert_eq!(
        cpu.memory.bytes[0x00..0x09],
        [42, 42, 0, 100, 100, 100, 100, 100, 0]
    );
}

#[test]
fn loading_across_pages_timing() {
    let mut cpu = cpu_with_code! {
        lda #56
        sta abs 0x5714
        lda #0

        ldx #0x74
        lda abs 0x56A0,x
        sta 0x05

        ldy #0x73
        lda (10),y
        sta 0x06
    };
    cpu.mut_memory().bytes[10..=11].copy_from_slice(&[0xA1, 0x56]);
    cpu.ticks(8 + 9).unwrap();
    assert_eq!(cpu.memory.bytes[5..=6], [0, 0]);
    cpu.ticks(1).unwrap();
    assert_eq!(cpu.memory.bytes[5..=6], [56, 0]);
    cpu.ticks(10).unwrap();
    assert_eq!(cpu.memory.bytes[5..=6], [56, 0]);
    cpu.ticks(1).unwrap();
    assert_eq!(cpu.memory.bytes[5..=6], [56, 56]);
}

#[test]
fn cmp() {
    let mut program = assemble6502! ({
        start: 0xF000,
        code: {
                ldx #0xFE
                txs
                plp
                lda #7
                // 10 cycles

                cmp #6
                beq fail
                bcc fail
                bmi fail
                sta 30
                // 11 cycles

                cmp #7
                bne fail
                bcc fail
                bmi fail
                sta 31
                // 11 cycles

                cmp #8
                beq fail
                bcs fail
                bpl fail
                sta 32
                // 11 cycles

                cmp #(-7i8 as u8)
                beq fail
                bcs fail
                bmi fail
                sta 33
                // 11 cycles

                cmp 30
                php
                // 6 cycles

                ldx #5
                cmp 35,x
                php
                // 9 cycles

                cmp abs 0x2345
                php
                // 7 cycles

                cmp abs 0x2341,x
                php
                // 7 cycles

                ldy #4
                cmp abs 0x2343,y
                php
                // 9 cycles

                cmp (36,x)
                php
                // 9 cycles

                cmp (43),y
                php
                // 8 cycles

                nop  // to be replaced
            fail:
                jmp fail
        }
    });
    // Deliberately inject HLT1 instead of NOP to make sure we never reach that
    // place and test timing.
    program[program.len() - 4] = opcodes::HLT1;
    let mut cpu = cpu_with_program(&program);
    // Some test data.
    cpu.mut_memory().bytes[40..=44].copy_from_slice(&[8, 0x48, 0x23, 0x45, 0x23]);
    cpu.mut_memory().bytes[0x2345..=0x2349].copy_from_slice(&[6, 7, 8, 6, 7]);
    cpu.ticks(10 + 4 * 11 + 6 + 9 + 7 + 7 + 9 + 9 + 9).unwrap();
    assert_eq!(cpu.memory.bytes[30..=33], [7, 7, 7, 7]);
    assert_eq!(
        reversed_stack(&cpu),
        [
            flags::PUSHED | flags::Z | flags::C,
            flags::PUSHED | flags::N,
            flags::PUSHED | flags::C,
            flags::PUSHED | flags::Z | flags::C,
            flags::PUSHED | flags::N,
            flags::PUSHED | flags::C,
            flags::PUSHED | flags::Z | flags::C,
        ]
    );
}

#[test]
fn cpx_cpy() {
    let mut cpu = cpu_with_code! {
            ldx #0xFE
            txs
            plp

            cpx #6
            php

            ldy #10
            cpy #25
            php

            lda #10
            ldx #20
            sta 4
            cpx 4
            php

            cpy 4
            php

            lda #15
            sta abs 0x1234
            cpx abs 0x1234
            php

            cpy abs 0x1234
            php
    };
    cpu.ticks(8 + 5 + 7 + 13 + 6 + 13 + 7).unwrap();
    assert_eq!(
        reversed_stack(&cpu),
        [
            flags::PUSHED | flags::N | flags::C,
            flags::PUSHED | flags::N,
            flags::PUSHED | flags::C,
            flags::PUSHED | flags::Z | flags::C,
            flags::PUSHED | flags::C,
            flags::PUSHED | flags::N,
        ]
    );
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
            flags::PUSHED | flags::N,
            flags::PUSHED | flags::V,
            flags::PUSHED | flags::Z,
            flags::PUSHED | flags::N | flags::V | flags::Z,
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
            flags::PUSHED,
            0x8F,
            flags::PUSHED | flags::V | flags::N,
            0x3B,
            flags::PUSHED | flags::C | flags::V,
            0x3D,
            flags::PUSHED,
            0xF7,
            flags::PUSHED | flags::N,
            0x77,
            flags::PUSHED | flags::C | flags::V,
            0xB8,
            flags::PUSHED | flags::V | flags::N,
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
            flags::PUSHED | flags::D | flags::C,
            0x30,
            flags::PUSHED | flags::D,
            0x04,
            flags::PUSHED | flags::D | flags::C,
            0x48,
            flags::PUSHED | flags::D,
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
            clc
            adc 5
            pha
            sec
            sbc 6
            pha
            ldx #2
            clc
            adc 3,x
            pha
            sec
            sbc 4,x
            pha

            clc
            adc abs 0x72C4
            pha
            sec
            sbc abs 0x72C5
            pha

            ldx #4
            clc
            adc abs 0x72C0,x
            pha
            sec
            sbc abs 0x72C1,x
            pha

            ldy #3
            clc
            adc abs 0x72C1,y
            pha
            sec
            sbc abs 0x72C2,y
            pha

            clc
            adc (0x4C,x)
            pha
            sec
            sbc (0x4E,x)
            pha

            clc
            adc (0x54),y
            pha
            sec
            sbc (0x56),y
            pha
    };
    cpu.mut_memory().bytes[0x50..=0x57]
        .copy_from_slice(&[0xC6, 0x72, 0xC7, 0x72, 0xC5, 0x72, 0xC6, 0x72]);
    cpu.mut_memory().bytes[0x72C4..=0x72C9].copy_from_slice(&[7, 6, 5, 9, 30, 3]);
    cpu.ticks(18 + 18 + 20 + 18 + 20 + 20 + 22 + 20).unwrap();
    assert_eq!(
        reversed_stack(&cpu),
        [35, 19, 34, 18, 25, 19, 26, 20, 27, 21, 26, 17, 47, 44]
    );
}

#[test]
fn overflow_flag() {
    let mut cpu = cpu_with_code! {
            ldx #0xFE
            txs
            plp
            // 8 cycles

            lda #0x40
            adc #0x40
            bvc fail
            adc #1
            bvs fail
            sbc #2
            bvc fail
            // 14 cycles

            php
            clv
            php
            // 8 cycles
        fail:
            jmp fail
    };
    cpu.ticks(8 + 14 + 8).unwrap();
    assert_eq!(
        reversed_stack(&cpu),
        [
            flags::PUSHED | flags::V | flags::C,
            flags::PUSHED | flags::C
        ]
    );
}

#[test]
fn and_ora() {
    let mut cpu = cpu_with_code! {
            ldx #0xFF
            txs
            lda #0b0000_1111
            and #0b1100_1100
            pha
            ora #0b1010_1010
            pha
            // 16 cycles

            and 44
            pha
            ora 45
            pha
            // 12 cycles

            ldx #2
            and 44,x
            pha
            inx
            ora 44,x
            pha
            // 18 cycles

            and abs 0x1234
            pha
            ora abs 0x1235
            pha
            // 14 cycles

            ldx #2
            and abs 0x1234,x
            inx
            pha
            ora abs 0x1234,x
            pha
            // 18 cycles

            ldy #3
            and abs 0x1235,y
            iny
            pha
            ora abs 0x1235,y
            pha
            // 18 cycles

            ldx #4
            and (44,x)
            pha
            ora (46,x)
            pha
            // 20 cycles

            ldy #8
            and (52),y
            pha
            iny
            ora (52),y
            pha
            // 20 cycles
    };
    cpu.mut_memory().bytes[44..=53].copy_from_slice(&[
        0b1111_0000,
        0b0101_0101,
        0b0100_0111,
        0b1100_0011,
        0x3A,
        0x12,
        0x3B,
        0x12,
        0x34,
        0x12,
    ]);
    cpu.mut_memory().bytes[0x1234..=0x123D].copy_from_slice(&[
        0b1010_1010,
        0b0011_1100,
        0b1111_0000,
        0b0101_0101,
        0b1100_1100,
        0b0000_1111,
        0b0110_0110,
        0b1001_1001,
        0b1010_1010,
        0b0011_0011,
    ]);

    cpu.ticks(16 + 12 + 18 + 14 + 18 + 18 + 20 + 20).unwrap();
    assert_eq!(
        reversed_stack(&cpu),
        [
            0b0000_1100,
            0b1010_1110,
            0b1010_0000,
            0b1111_0101,
            0b0100_0101,
            0b1100_0111,
            0b1000_0010,
            0b1011_1110,
            0b1011_0000,
            0b1111_0101,
            0b1100_0100,
            0b1100_1111,
            0b0100_0110,
            0b1101_1111,
            0b1000_1010,
            0b1011_1011,
        ]
    );
}

#[test]
fn eor() {
    let mut cpu = cpu_with_code! {
            ldx #0xFF
            txs
            lda #0b0000_1111
            eor #0b1100_1100
            pha
            // 11 cycles

            eor 20
            pha
            // 6 cycles

            ldx #10
            eor 11,x
            pha
            // 9 cycles

            eor abs 0x3210
            pha
            // 7 cycles

            eor abs 0x3207,x
            pha
            // 7 cycles

            ldy #2
            eor abs 0x3210,y
            pha
            // 9 cycles

            eor (12,x)
            pha
            // 9 cycles

            eor (24),y
            pha
            // 8 cycles
    };
    cpu.mut_memory().bytes[20..=25].copy_from_slice(&[
        0b1111_0000,
        0b1010_0101,
        0x13,
        0x32,
        0x12,
        0x32,
    ]);
    cpu.mut_memory().bytes[0x3210..=0x3214].copy_from_slice(&[
        0b0000_1111,
        0b1100_1100,
        0b1111_1111,
        0b0011_1100,
        0b0001_1000,
    ]);

    cpu.ticks(11 + 6 + 9 + 7 + 7 + 9 + 9 + 8).unwrap();
    assert_eq!(
        reversed_stack(&cpu),
        [
            0b1100_0011,
            0b0011_0011,
            0b1001_0110,
            0b1001_1001,
            0b0101_0101,
            0b1010_1010,
            0b1001_0110,
            0b1000_1110,
        ],
    );
}

#[test]
fn asl() {
    let mut cpu = cpu_with_code! {
            sec
            lda #0b0101_0000
            // 4 cycles

            asl a
        stop1:
            bcs stop1
            sta 0x01
            // 7 cycles

            asl 0x01
        stop2:
            bcc stop2
            sta 0x02
            // 10 cycles

            ldx #1
            asl 0x01,x
        stop3:
            bcc stop3
            // 10 cycles

            stx abs 0x0234
            asl abs 0x0234
            // 10 cycles

            inx
            stx abs 0x0235
            asl abs 0x0233,x
            // 13 cycles

            ldx #0x80
            asl abs 0x01B5,x // Test cross-page indexing
            // 9 cycles
    };
    cpu.ticks(4 + 7 + 10 + 10 + 10 + 13 + 9).unwrap();
    assert_eq!(cpu.memory.bytes[1..=2], [0b0100_0000, 0b0100_0000]);
    assert_eq!(cpu.memory.bytes[0x0234..=0x0235], [2, 8]);
}

#[test]
fn lsr() {
    let mut cpu = cpu_with_code! {
            sec
            lda #0b0000_1010
            // 4 cycles

            lsr a
        stop1:
            bcs stop1
            sta 0x0D
            // 7 cycles

            lsr 0x0D
        stop2:
            bcc stop2
            sta 0x0E
            // 10 cycles

            ldx #0x0A
            lsr 0x04,x
        stop3:
            bcc stop3
            // 10 cycles

            stx abs 0x0234
            lsr abs 0x0234
            // 10 cycles

            stx abs 0x0235
            lsr abs 0x022B,x
            // 11 cycles

            ldx #0x80
            lsr abs 0x01B5,x // Test cross-page indexing
            // 9 cycles
    };
    cpu.ticks(4 + 7 + 10 + 10 + 10 + 11 + 9).unwrap();
    assert_eq!(cpu.memory.bytes[0x0D..=0x0E], [0b0000_0010, 0b0000_0010]);
    assert_eq!(cpu.memory.bytes[0x0234..=0x0235], [0b0101, 0b0010]);
}

#[test]
fn rol() {
    let mut cpu = cpu_with_code! {
            clc
            lda #0b1010_0000
            // 4 cycles

            rol a
        stop1:
            bcc stop1
            sta 0x01
            // 7 cycles

            rol 0x01
        stop2:
            bcs stop2
            sta 0x02
            // 10 cycles

            ldx #1
            rol 0x01,x
        stop3:
            bcs stop3
            // 10 cycles

            stx abs 0x0234
            rol abs 0x0234
            // 10 cycles

            stx abs 0x0235
            rol abs 0x0234,x
            // 11 cycles
    };
    cpu.ticks(4 + 7 + 10 + 10 + 10 + 11).unwrap();
    assert_eq!(cpu.memory.bytes[1..=2], [0b1000_0001, 0b1000_0000]);
    assert_eq!(cpu.memory.bytes[0x0234..=0x0235], [2, 2]);
}

#[test]
fn ror() {
    let mut cpu = cpu_with_code! {
            clc
            lda #0b0000_0101
            // 4 cycles

            ror a
        stop1:
            bcc stop1
            sta 0x05
            // 7 cycles

            ror 0x05
        stop2:
            bcs stop2
            sta 0x06
            // 10 cycles

            ldx #2
            ror 0x04,x
        stop3:
            bcs stop3
            // 10 cycles

            stx abs 0x0234
            ror abs 0x0234
            // 10 cycles

            stx abs 0x0235
            ror abs 0x0233,x
            // 11 cycles
    };
    cpu.ticks(4 + 7 + 10 + 10 + 10 + 11).unwrap();
    assert_eq!(cpu.memory.bytes[5..=6], [0b1000_0001, 0b0000_0001]);
    assert_eq!(cpu.memory.bytes[0x0234..=0x0235], [1, 1]);
}

#[test]
fn inc_dec() {
    let mut cpu = cpu_with_code! {
            inc 10
            inc 10
            dec 11
            dec 11
            // 20 cycles

            ldx #1
            inc 11,x
            inx
            dec 11,x
            // 16 cycles

            inc abs 0x2345
            inc abs 0x2343,x
            dec abs 0x2346
            dec abs 0x2344,x
    };
    cpu.ticks(20 + 16 + 26).unwrap();
    assert_eq!(cpu.memory.bytes[10..=13], [2, -2i8 as u8, 1, -1i8 as u8]);
    assert_eq!(cpu.memory.bytes[0x2345..=0x2346], [2, -2i8 as u8]);
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
fn tay() {
    let mut cpu = cpu_with_code! {
            lda #76
            tay
            sty 0x01
    };
    cpu.ticks(7).unwrap();
    assert_eq!(cpu.memory.bytes[0x01], 76);
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
            flags::PUSHED,
            flags::C | flags::PUSHED,
            flags::C | flags::I | flags::N | flags::PUSHED,
            flags::C | flags::I | flags::Z | flags::PUSHED,
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
    let memory = Box::new(Ram::with_test_program_at(
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
fn jmp_indirect() {
    let mut cpu = cpu_with_code! {
            jmp start  // 0xF000
            // 3 cycles
            jmp store1 // 0xF003
            jmp store2 // 0xF006

        start:
            lda #0xFF
            jmp (0x1234) // Will point to 0xF003
        stop1:
            jmp stop1
        store1:
            sta 10
            // 13 cycles (incl. jump at 0xF003)

            jmp (0x12FF) // Will point to 0xF006
        stop2:
            jmp stop2
        store2:
            sta 11
            // 11 cycles (incl. jump at 0xF006)
    };
    cpu.mut_memory().bytes[0x1234..=0x1235].copy_from_slice(&[0x03, 0xF0]);
    // Cross-page indirect jump quirk
    cpu.mut_memory().bytes[0x1200] = 0xF0;
    cpu.mut_memory().bytes[0x12FF] = 0x06;

    cpu.ticks(3 + 13 + 11).unwrap();
    assert_eq!(cpu.memory.bytes[10..=11], [0xFF, 0xFF]);
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
    let mut memory = Box::new(Ram::with_test_program_at(
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
    let mut memory = Box::new(Ram::with_test_program_at(
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

#[test]
fn brk_rti() {
    let mut cpu = cpu_with_code! {
            jmp start     // 0xF000
            // 3 cycles
            jmp interrupt // 0xF003

        start:
            ldx #0xFE
            txs
            plp
            sed
            brk
            // 17 cycles

            cld // should be ignored

            php
            // 3 cycles
            nop  // 0xF00E (To be replaced with HLT)

        interrupt:
            stx 45
            // Manipulate the flags; these should be reverted once we return.
            sec
            cld
            sei
            rti
            // 18 cycles (including JMP in 0xF003)
    };
    cpu.mut_memory().bytes[0xFFFE..=0xFFFF].copy_from_slice(&[0x03, 0xF0]);
    cpu.mut_memory().bytes[0xF00E] = opcodes::HLT1;
    cpu.ticks(3 + 17 + 18 + 3).unwrap();
    assert_eq!(cpu.memory.bytes[45], 0xFE);
    assert_eq!(reversed_stack(&cpu), [flags::PUSHED | flags::D]);
}

fn cpu_with_interrupt_test_code() -> Cpu<Ram> {
    cpu_with_code! {
            jmp start
            // 3 cycles
            jmp interrupt // 0xF003

        start:
            ldx #0xFE
            txs
            plp
            lda #0
            sta 10
            ldx #0
            cli
            // 17 cycles

        loop:
            inc 10
            jmp loop
            // 8 cycles

        interrupt:
            lda 10
            sta 11,x
            inx
            rti
            // 18 cycles (including JMP in 0xF003) + 7 cycles of interrupt
            // sequence
    }
}

#[test]
fn irq() {
    let mut cpu = cpu_with_interrupt_test_code();
    cpu.mut_memory().bytes[0xFFFE..=0xFFFF].copy_from_slice(&[0x03, 0xF0]);
    cpu.set_irq_pin(false);
    cpu.ticks(3 + 17 + 2 * 8).unwrap();
    // At this moment, we should have counted to 2, and no interrupt should have
    // been triggered.
    assert_eq!(cpu.memory.bytes[10..=14], [2, 0, 0, 0, 0]);

    cpu.set_irq_pin(true);
    cpu.ticks(7 + 18).unwrap();
    // No B flag expected on the stack this time.
    assert_eq!(cpu.memory.bytes[0x1FD], flags::UNUSED);
    assert_eq!(cpu.memory.bytes[10..=14], [2, 2, 0, 0, 0]);

    // Turn off the IRQ line, expecting no interrupts.
    cpu.set_irq_pin(false);
    cpu.ticks(3 * 8).unwrap();
    assert_eq!(cpu.memory.bytes[10..=14], [5, 2, 0, 0, 0]);

    // Turn the IRQ line back on for twice as long as before, triggering two
    // consecutive interrupts. To make it more fun, trigger the interrupt in the
    // middle of processing the INC instruction. This means INC will be fully
    // processed, increasing cell 10 to 6!
    cpu.ticks(2).unwrap();
    cpu.set_irq_pin(true);
    cpu.ticks(3 + 2 * (7 + 18)).unwrap();
    assert_eq!(cpu.memory.bytes[10..=14], [6, 2, 6, 6, 0]);
}

#[test]
fn irq_right_after_init_pushes_b_flag_unset() {
    // This test assures that the B flag is never set to 1 internally.
    let mut cpu = cpu_with_code! {
            ldx #0xFF
            txs
            cli
        loop:
            jmp loop

        interrupt:        // 0xF007
            jmp interrupt
    };
    cpu.mut_memory().bytes[0xFFFE..=0xFFFF].copy_from_slice(&[0x07, 0xF0]);
    cpu.ticks(2 + 2 + 2).unwrap();
    cpu.set_irq_pin(true);
    cpu.ticks(7).unwrap();
    let flags = cpu.memory.bytes[0x01FD];
    assert_eq!(flags & flags::UNUSED, flags::UNUSED);
    assert_eq!(flags & flags::B, 0);
}

#[test]
fn nmi() {
    let mut cpu = cpu_with_interrupt_test_code();
    cpu.mut_memory().bytes[0xFFFA..=0xFFFB].copy_from_slice(&[0x03, 0xF0]);
    cpu.set_nmi_pin(false);
    cpu.ticks(3 + 17 + 2 * 8).unwrap();
    assert_eq!(cpu.memory.bytes[10..=15], [2, 0, 0, 0, 0, 0]);

    cpu.set_nmi_pin(true);
    cpu.ticks(7 + 18).unwrap();
    assert_eq!(cpu.memory.bytes[10..=15], [2, 2, 0, 0, 0, 0]);

    // Since NMI is edge-triggered, this shouldn't result in another interrupt.
    cpu.ticks(3 * 8).unwrap();
    assert_eq!(cpu.memory.bytes[10..=15], [5, 2, 0, 0, 0, 0]);

    // Release the NMI flag for a while.
    cpu.set_nmi_pin(false);
    cpu.ticks(2 * 8).unwrap();
    assert_eq!(cpu.memory.bytes[10..=15], [7, 2, 0, 0, 0, 0]);

    // Trigger another interrupt; this time with a very short signal, in the
    // middle of processing the INC instruction.
    cpu.ticks(1).unwrap();
    cpu.set_nmi_pin(true);
    cpu.ticks(1).unwrap();
    cpu.set_nmi_pin(false);
    cpu.ticks(7 + 18 - 2).unwrap();
    assert_eq!(cpu.memory.bytes[10..=15], [8, 2, 8, 0, 0, 0]);
}

#[test]
fn irq_masking() {
    let mut cpu = cpu_with_code! {
            sei // 2 cycles
        loop:
            jmp loop

        interrupt:  // 0xF004
            inc 5
            rti
            // 11 cycles + 7 cycles of interrupt sequence
    };
    cpu.mut_memory().bytes[0xFFFE..=0xFFFF].copy_from_slice(&[0x04, 0xF0]);
    cpu.ticks(2).unwrap();
    cpu.set_irq_pin(true);
    cpu.ticks(7 + 11).unwrap();
    assert_eq!(cpu.memory.bytes[5], 0);
}

#[test]
fn reports_instruction_start() {
    let mut cpu = cpu_with_code! {
            lda #1         // 0xF000
            nop            // 0xF002
            sta abs 0xABCD // 0xF003
            nop            // 0xF006
    };
    assert!(cpu.at_instruction_start());
    assert_eq!(cpu.reg_pc(), 0xF000);

    cpu.tick().unwrap();
    assert!(!cpu.at_instruction_start());
    cpu.tick().unwrap();
    assert!(cpu.at_instruction_start());
    assert_eq!(cpu.reg_pc(), 0xF002);

    cpu.tick().unwrap();
    assert!(!cpu.at_instruction_start());
    cpu.tick().unwrap();
    assert!(cpu.at_instruction_start());
    assert_eq!(cpu.reg_pc(), 0xF003);

    cpu.tick().unwrap();
    assert!(!cpu.at_instruction_start());
    cpu.tick().unwrap();
    assert!(!cpu.at_instruction_start());
    cpu.tick().unwrap();
    assert!(!cpu.at_instruction_start());
    cpu.tick().unwrap();
    assert!(cpu.at_instruction_start());
    assert_eq!(cpu.reg_pc(), 0xF006);
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
            asl 1
            lsr 2
            inx
            jmp loop
    };
    b.iter(|| {
        reset(&mut cpu);
        cpu.ticks(1000).unwrap();
    });
}
