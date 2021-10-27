#![cfg(test)]

use super::*;
use crate::test_utils::decode_video_outputs;
use crate::test_utils::encode_audio;
use crate::test_utils::encode_video_outputs;

/// A utility that produces a sequence of TIA video outputs. Useful for
/// comparing with expected sequences in tests.
struct VideoOutputIterator<'a> {
    tia: &'a mut Tia,
}

impl<'a> Iterator for VideoOutputIterator<'a> {
    type Item = VideoOutput;

    fn next(&mut self) -> Option<VideoOutput> {
        return Some(self.tia.tick().video);
    }
}

fn wait_ticks(tia: &mut Tia, n: u32) {
    for _ in 0..n {
        tia.tick();
    }
}

fn scan_video(tia: &mut Tia, n_pixels: u32) -> Vec<VideoOutput> {
    (0..n_pixels).map(|_| tia.tick().video).collect()
}

fn scan_audio<'a>(tia: &'a mut Tia, n_samples: usize) -> impl Iterator<Item = AudioOutput> + 'a {
    std::iter::from_fn(move || Some(tia.tick().audio))
        // Only those who are not None.
        .filter_map(std::convert::identity)
        .take(n_samples)
}

fn scan_audio_ticks<'a>(tia: &'a mut Tia, n_ticks: u32) -> impl Iterator<Item = AudioOutput> + 'a {
    (0..n_ticks)
        .map(move |_| tia.tick().audio)
        // Only those who are not None.
        .filter_map(std::convert::identity)
}

#[test]
fn draws_background_pixels() {
    let mut tia = Tia::new();
    wait_ticks(&mut tia, HBLANK_WIDTH);

    tia.write(registers::COLUBK, 0x02).unwrap();
    assert_eq!(tia.tick().video, VideoOutput::pixel(0x02));

    tia.write(registers::COLUBK, 0xfe).unwrap();
    assert_eq!(tia.tick().video, VideoOutput::pixel(0xfe));
}

#[test]
fn draws_scanlines() {
    let expected_output = decode_video_outputs(
        "................||||||||||||||||....................................\
         88888888888888888888888888888888888888888888888888888888888888888888888888888888\
         88888888888888888888888888888888888888888888888888888888888888888888888888888888\
         ................||||||||||||||||....................................\
         88888888888888888888888888888888888888888888888888888888888888888888888888888888\
         88888888888888888888888888888888888888888888888888888888888888888888888888888888",
    );

    let mut tia = Tia::new();
    tia.write(registers::COLUBK, 0x08).unwrap();
    // Generate two scanlines (2 * TOTAL_WIDTH clock cycles).
    let output = VideoOutputIterator { tia: &mut tia }.take(2 * TOTAL_WIDTH as usize);
    itertools::assert_equal(output, expected_output);
}

#[test]
fn emits_vsync() {
    let expected_output = decode_video_outputs(
        "----------------++++++++++++++++------------------------------------\
         ================================================================================\
         ================================================================================",
    );

    let mut tia = Tia::new();
    tia.write(registers::COLUBK, 0x00).unwrap();
    tia.write(registers::VSYNC, flags::VSYNC_ON).unwrap();
    let output = VideoOutputIterator { tia: &mut tia }.take(TOTAL_WIDTH as usize);
    itertools::assert_equal(output, expected_output);

    // Note: we turn off VSYNC not by writing 0, but by setting all bits but
    // bit 1. This is to make sure that all other bits are ignored.
    tia.write(registers::VSYNC, !flags::VSYNC_ON).unwrap();
    assert_eq!(tia.tick().video, VideoOutput::blank());
}

#[test]
fn emits_vblank() {
    let expected_output = decode_video_outputs(
        "................||||||||||||||||....................................\
         ................................................................................\
         ................................................................................",
    );

    let mut tia = Tia::new();
    tia.write(registers::COLUBK, 0x32).unwrap();
    tia.write(registers::VBLANK, flags::VBLANK_ON).unwrap();
    let output = VideoOutputIterator { tia: &mut tia }.take(TOTAL_WIDTH as usize);
    itertools::assert_equal(output, expected_output);

    // Make sure that only bit 1 of VBLANK counts.
    tia.write(registers::VBLANK, !flags::VBLANK_ON).unwrap();
    wait_ticks(&mut tia, HBLANK_WIDTH);
    assert_eq!(tia.tick().video, VideoOutput::pixel(0x32));
}

#[test]
fn emits_vblank_with_vsync() {
    let expected_output = decode_video_outputs(
        "----------------++++++++++++++++------------------------------------\
         --------------------------------------------------------------------------------\
         --------------------------------------------------------------------------------",
    );

    let mut tia = Tia::new();
    tia.write(registers::VSYNC, flags::VSYNC_ON).unwrap();
    tia.write(registers::VBLANK, flags::VBLANK_ON).unwrap();
    let output = VideoOutputIterator { tia: &mut tia }.take(TOTAL_WIDTH as usize);
    itertools::assert_equal(output, expected_output);
}

#[test]
fn tells_to_tick_cpu_every_three_cycles() {
    let mut tia = Tia::new();
    assert_eq!(tia.tick().cpu_tick, true);
    assert_eq!(tia.tick().cpu_tick, false);
    assert_eq!(tia.tick().cpu_tick, false);
    assert_eq!(tia.tick().cpu_tick, true);
    assert_eq!(tia.tick().cpu_tick, false);
    assert_eq!(tia.tick().cpu_tick, false);
    assert_eq!(tia.tick().cpu_tick, true);
}

#[test]
fn freezes_cpu_until_wsync() {
    let mut tia = Tia::new();
    tia.tick();
    tia.write(registers::WSYNC, 0x00).unwrap();
    for i in 1..TOTAL_WIDTH {
        assert_eq!(tia.tick().cpu_tick, false, "for index {}", i);
    }
    assert_eq!(tia.tick().cpu_tick, true);
    assert_eq!(tia.tick().cpu_tick, false);
    assert_eq!(tia.tick().cpu_tick, false);
    assert_eq!(tia.tick().cpu_tick, true);
}

#[test]
fn tells_riot_to_tick_every_three_cycles() {
    let mut tia = Tia::new();
    assert_eq!(tia.tick().riot_tick, true);
    assert_eq!(tia.tick().riot_tick, false);
    assert_eq!(tia.tick().riot_tick, false);
    assert_eq!(tia.tick().riot_tick, true);
    //Even if WSYNC is turned on!
    tia.write(registers::WSYNC, 0x00).unwrap();
    assert_eq!(tia.tick().riot_tick, false);
    assert_eq!(tia.tick().riot_tick, false);
    assert_eq!(tia.tick().riot_tick, true);
}

#[test]
fn draws_playfield() {
    let mut tia = Tia::new();
    tia.write(registers::COLUBK, 0).unwrap();
    tia.write(registers::COLUPF, 2).unwrap();
    tia.write(registers::PF0, 0b11010000).unwrap();
    tia.write(registers::PF1, 0b10011101).unwrap();
    tia.write(registers::PF2, 0b10110101).unwrap();
    tia.write(
        registers::CTRLPF,
        0xff & !flags::CTRLPF_REFLECT & !flags::CTRLPF_SCORE,
    )
    .unwrap();

    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         22220000222222222222000000002222222222220000222222220000222200002222222200002222\
         22220000222222222222000000002222222222220000222222220000222200002222222200002222",
    );
}

#[test]
fn draws_reflected_playfield() {
    let expected_output = decode_video_outputs(
        "................||||||||||||||||....................................\
         66662222666666666666222222226666666666662222666666662222666622226666666622226666\
         66662222666666662222666622226666666622226666666666662222222266666666666622226666",
    );

    let mut tia = Tia::new();
    tia.write(registers::COLUBK, 2).unwrap();
    tia.write(registers::COLUPF, 6).unwrap();
    tia.write(registers::PF0, 0b11010000).unwrap();
    tia.write(registers::PF1, 0b10011101).unwrap();
    tia.write(registers::PF2, 0b10110101).unwrap();
    tia.write(registers::CTRLPF, flags::CTRLPF_REFLECT).unwrap();
    // Generate two scanlines (2 * TOTAL_WIDTH clock cycles).
    let output = VideoOutputIterator { tia: &mut tia }.take(TOTAL_WIDTH as usize);
    itertools::assert_equal(output, expected_output);
}

#[test]
fn rsync() {
    let expected_output_1 = decode_video_outputs(
        "................||||||||||||||||....................................\
         888888888888",
    );
    let expected_output_2 = decode_video_outputs(
        "888\
         ................||||||||||||||||....................................\
         88888888888888888888888888888888888888888888888888888888888888888888888888888888\
         88888888888888888888888888888888888888888888888888888888888888888888888888888888",
    );

    let mut tia = Tia::new();
    tia.write(registers::COLUBK, 0x08).unwrap();
    // Generate two scanlines (2 * TOTAL_WIDTH clock cycles).
    let output = VideoOutputIterator { tia: &mut tia }.take(HBLANK_WIDTH as usize + 12);
    itertools::assert_equal(output, expected_output_1);
    tia.write(registers::RSYNC, 0x00).unwrap();
    let output = VideoOutputIterator { tia: &mut tia }.take(TOTAL_WIDTH as usize + 3);
    itertools::assert_equal(output, expected_output_2);
}

#[test]
fn draws_sprites() {
    let mut tia = Tia::new();
    tia.write(registers::COLUBK, 0x02).unwrap();
    tia.write(registers::COLUP0, 0x04).unwrap();
    tia.write(registers::COLUP1, 0x06).unwrap();
    tia.write(registers::GRP0, 0b1010_0101).unwrap();
    tia.write(registers::GRP1, 0b1100_0011).unwrap();
    tia.write(registers::ENAM0, flags::ENAXX_ENABLE).unwrap();
    tia.write(registers::ENAM1, flags::ENAXX_ENABLE).unwrap();

    let p0_delay = 30 * 3;
    let p1_delay = 3 * 3;
    let m0_delay = 4 * 3;
    let m1_delay = 2 * 3;
    wait_ticks(&mut tia, p0_delay);
    tia.write(registers::RESP0, 0).unwrap();
    wait_ticks(&mut tia, p1_delay);
    tia.write(registers::RESP1, 0).unwrap();
    wait_ticks(&mut tia, m0_delay);
    tia.write(registers::RESM0, 0).unwrap();
    wait_ticks(&mut tia, m1_delay);
    tia.write(registers::RESM1, 0).unwrap();
    wait_ticks(
        &mut tia,
        TOTAL_WIDTH - p0_delay - p1_delay - m0_delay - m1_delay,
    );

    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         22222222222222222222222222222424224242662222662224222226222222222222222222222222\
         22222222222222222222222222222222222222222222222222222222222222222222222222222222",
    );

    tia.write(registers::COLUP0, 0x08).unwrap();
    tia.write(registers::COLUP1, 0x0A).unwrap();
    tia.write(registers::GRP0, 0b1111_0101).unwrap();
    tia.write(registers::GRP1, 0b1010_1111).unwrap();

    let p0_delay = 36 * 3;
    let p1_delay = 6 * 3;
    let m0_delay = 8 * 3;
    let m1_delay = 1 * 3;
    wait_ticks(&mut tia, p0_delay);
    tia.write(registers::RESP0, 0).unwrap();
    wait_ticks(&mut tia, p1_delay);
    tia.write(registers::RESP1, 0).unwrap();
    wait_ticks(&mut tia, m0_delay);
    tia.write(registers::RESM0, 0).unwrap();
    wait_ticks(&mut tia, m1_delay);
    tia.write(registers::RESM1, 0).unwrap();
    wait_ticks(
        &mut tia,
        TOTAL_WIDTH - p0_delay - p1_delay - m0_delay - m1_delay,
    );

    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         22222222222222222222222222222222222222222222222888828282222222222A2A2AAAA2222222\
         22222222822A22222222222222222222222222222222222222222222222222222222222222222222",
    );
}

#[test]
fn moves_sprites() {
    let mut tia = Tia::new();
    tia.write(registers::COLUBK, 0x00).unwrap();
    tia.write(registers::COLUP0, 0x02).unwrap();
    tia.write(registers::COLUP1, 0x04).unwrap();
    tia.write(registers::GRP0, 0b1100_0011).unwrap();
    tia.write(registers::GRP1, 0b1100_0011).unwrap();
    tia.write(registers::ENAM0, flags::ENAXX_ENABLE).unwrap();
    tia.write(registers::ENAM1, flags::ENAXX_ENABLE).unwrap();
    tia.write(registers::HMP0, 3 << 4).unwrap();
    tia.write(registers::HMP1, (-5i8 << 4) as u8).unwrap();
    tia.write(registers::HMM0, (-6i8 << 4) as u8).unwrap();
    tia.write(registers::HMM1, 4 << 4 as u8).unwrap();

    let p0_delay = 32 * 3;
    let p1_delay = 6 * 3;
    let m0_delay = 9 * 3;
    let m1_delay = 2 * 3;
    wait_ticks(&mut tia, p0_delay);
    tia.write(registers::RESP0, 0).unwrap();
    wait_ticks(&mut tia, p1_delay);
    tia.write(registers::RESP1, 0).unwrap();
    wait_ticks(&mut tia, m0_delay);
    tia.write(registers::RESM0, 0).unwrap();
    wait_ticks(&mut tia, m1_delay);
    tia.write(registers::RESM1, 0).unwrap();
    wait_ticks(
        &mut tia,
        TOTAL_WIDTH - p0_delay - p1_delay - m0_delay - m1_delay,
    );

    // Pretend we're doing an STA: wait for 2 CPU cycles, write to register
    // on the 3rd one.
    let mut scanline = scan_video(&mut tia, 2 * 3 + 1);
    tia.write(registers::HMOVE, 0).unwrap();
    scanline.append(&mut scan_video(&mut tia, TOTAL_WIDTH - (2 * 3 + 1)));

    assert_eq!(
        encode_video_outputs(scanline),
        "................||||||||||||||||....................................\
         ........000000000000000000000000220000220000000000000000004400004400000000000000\
         04000200000000000000000000000000000000000000000000000000000000000000000000000000",
    );

    // Do the same once again, and then clear the movement registers before
    // HMOVE on the 3rd line. The 3rd line should look exactly as the 2nd
    // one.
    let mut scanline = scan_video(&mut tia, 2 * 3 + 1);
    tia.write(registers::HMOVE, 0).unwrap();
    scanline.append(&mut scan_video(&mut tia, TOTAL_WIDTH - (2 * 3 + 1)));
    tia.write(registers::HMCLR, 0).unwrap();
    scanline.append(&mut scan_video(&mut tia, 2 * 3 + 1));
    tia.write(registers::HMOVE, 0).unwrap();
    scanline.append(&mut scan_video(&mut tia, TOTAL_WIDTH - (2 * 3 + 1)));

    assert_eq!(
        encode_video_outputs(scanline),
        "................||||||||||||||||....................................\
         ........000000000000000000000220000220000000000000000000000000044000044000000400\
         00000000000200000000000000000000000000000000000000000000000000000000000000000000\
         ................||||||||||||||||....................................\
         ........000000000000000000000220000220000000000000000000000000044000044000000400\
         00000000000200000000000000000000000000000000000000000000000000000000000000000000",
    );

    // Test RESMPx: make sure the missiles move along with players and stop
    // following them once they are freed.
    tia.write(registers::RESMP0, flags::RESMPX_RESET).unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000000000000000000000000000220000220000000000000000000000000044000044000000400\
         00000000000000000000000000000000000000000000000000000000000000000000000000000000",
    );

    tia.write(registers::RESMP1, flags::RESMPX_RESET).unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000000000000000000000000000220000220000000000000000000000000044000044000000000\
         00000000000000000000000000000000000000000000000000000000000000000000000000000000",
    );

    tia.write(registers::RESMP0, 0).unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000000000000000000000000000220020220000000000000000000000000044000044000000000\
         00000000000000000000000000000000000000000000000000000000000000000000000000000000",
    );
    tia.write(registers::RESMP1, 0).unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000000000000000000000000000220020220000000000000000000000000044004044000000000\
         00000000000000000000000000000000000000000000000000000000000000000000000000000000",
    );
}

#[test]
fn sprite_delay() {
    let mut tia = Tia::new();
    tia.write(registers::COLUP0, 0x02).unwrap();
    tia.write(registers::COLUP1, 0x04).unwrap();
    tia.write(registers::VDELP0, flags::VDELXX_ON).unwrap();
    tia.write(registers::VDELP1, flags::VDELXX_ON).unwrap();
    // Reset both new and old values.
    tia.write(registers::GRP0, 0b0000_0001).unwrap();
    tia.write(registers::GRP1, 0b0000_0001).unwrap();
    tia.write(registers::GRP0, 0b0000_0001).unwrap();
    tia.write(registers::GRP1, 0b0000_0001).unwrap();

    let p0_delay = 30 * 3;
    let p1_delay = 3 * 3;
    wait_ticks(&mut tia, p0_delay);
    tia.write(registers::RESP0, 0).unwrap();
    wait_ticks(&mut tia, p1_delay);
    tia.write(registers::RESP1, 0).unwrap();
    wait_ticks(&mut tia, TOTAL_WIDTH - p0_delay - p1_delay);
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000000000000000000000000000000000020000000040000000000000000000000000000000000\
         00000000000000000000000000000000000000000000000000000000000000000000000000000000",
    );

    // Write a new value to GRP0, update old value of GRP1. Since old value of
    // GRP1 is the same as the new value of GRP1, no change is expected.
    tia.write(registers::GRP0, 0b0000_0011).unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000000000000000000000000000000000020000000040000000000000000000000000000000000\
         00000000000000000000000000000000000000000000000000000000000000000000000000000000",
    );

    // Write a new value to GRP1, update old value of GRP0.
    tia.write(registers::GRP1, 0b0000_0011).unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000000000000000000000000000000000220000000040000000000000000000000000000000000\
         00000000000000000000000000000000000000000000000000000000000000000000000000000000",
    );

    // Write a new value to GRP0, update old value of GRP1.
    tia.write(registers::GRP0, 0b0000_0111).unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000000000000000000000000000000000220000000440000000000000000000000000000000000\
         00000000000000000000000000000000000000000000000000000000000000000000000000000000",
    );
}

#[test]
fn player_reflection() {
    let mut tia = Tia::new();
    tia.write(registers::COLUP0, 0x0A).unwrap();
    tia.write(registers::COLUP1, 0x0E).unwrap();
    tia.write(registers::GRP0, 0b1011_0001).unwrap();
    tia.write(registers::GRP1, 0b1101_0001).unwrap();

    let p0_delay = 30 * 3;
    let p1_delay = 4 * 3;
    wait_ticks(&mut tia, p0_delay);
    tia.write(registers::RESP0, 0).unwrap();
    wait_ticks(&mut tia, p1_delay);
    tia.write(registers::RESP1, 0).unwrap();
    wait_ticks(&mut tia, TOTAL_WIDTH - p0_delay - p1_delay);

    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000000000000000000000000000A0AA000A0000EE0E000E0000000000000000000000000000000\
         00000000000000000000000000000000000000000000000000000000000000000000000000000000",
    );

    tia.write(registers::REFP0, flags::REFPX_REFLECT).unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000000000000000000000000000A000AA0A0000EE0E000E0000000000000000000000000000000\
         00000000000000000000000000000000000000000000000000000000000000000000000000000000",
    );

    tia.write(registers::REFP1, flags::REFPX_REFLECT).unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000000000000000000000000000A000AA0A0000E000E0EE0000000000000000000000000000000\
         00000000000000000000000000000000000000000000000000000000000000000000000000000000",
    );
}

#[test]
fn sprite_copies() {
    use flags::*;
    use registers::{NUSIZ0, NUSIZ1};

    let mut tia = Tia::new();
    tia.write(registers::COLUP0, 0x0A).unwrap();
    tia.write(registers::COLUP1, 0x0C).unwrap();
    tia.write(registers::GRP0, 0b1010_0101).unwrap();
    tia.write(registers::GRP1, 0b1010_0101).unwrap();
    tia.write(registers::ENAM0, flags::ENAXX_ENABLE).unwrap();
    tia.write(registers::ENAM1, flags::ENAXX_ENABLE).unwrap();

    let p0_delay = 21 * 3;
    let m0_delay = 3 * 3;
    let p1_delay = 23 * 3;
    let m1_delay = 4 * 3;
    wait_ticks(&mut tia, p0_delay);
    tia.write(registers::RESP0, 0).unwrap();
    wait_ticks(&mut tia, m0_delay);
    tia.write(registers::RESM0, 0).unwrap();
    wait_ticks(&mut tia, p1_delay);
    tia.write(registers::RESP1, 0).unwrap();
    wait_ticks(&mut tia, m1_delay);
    tia.write(registers::RESM1, 0).unwrap();
    wait_ticks(
        &mut tia,
        TOTAL_WIDTH - p0_delay - p1_delay - m0_delay - m1_delay,
    );
    tia.write(registers::HMP0, 3 << 4).unwrap();
    tia.write(registers::HMOVE, 0).unwrap();
    wait_ticks(&mut tia, TOTAL_WIDTH);
    tia.write(registers::HMP0, 0).unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         A0A00A0A00A000000000000000000000000000000000000000000000000000000000000000000000\
         C0C00C0C000C00000000000000000000000000000000000000000000000000000000000000000000",
    );

    tia.write(NUSIZ0, NUSIZX_TWO_COPIES_CLOSE | NUSIZX_MISSILE_WIDTH_1)
        .unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         A0A00A0A00A00000A0A00A0A00A00000000000000000000000000000000000000000000000000000\
         C0C00C0C000C00000000000000000000000000000000000000000000000000000000000000000000",
    );

    tia.write(NUSIZ0, NUSIZX_TWO_COPIES_MEDIUM | NUSIZX_MISSILE_WIDTH_1)
        .unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         A0A00A0A00A000000000000000000000A0A00A0A00A0000000000000000000000000000000000000\
         C0C00C0C000C00000000000000000000000000000000000000000000000000000000000000000000",
    );

    tia.write(NUSIZ0, NUSIZX_TWO_COPIES_WIDE | NUSIZX_MISSILE_WIDTH_1)
        .unwrap();
    tia.write(NUSIZ1, NUSIZX_TWO_COPIES_CLOSE | NUSIZX_MISSILE_WIDTH_1)
        .unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         A0A00A0A00A00000000000000000000000000000000000000000000000000000A0A00A0A00A00000\
         C0C00C0C000C0000C0C00C0C000C0000000000000000000000000000000000000000000000000000",
    );

    tia.write(NUSIZ0, NUSIZX_THREE_COPIES_CLOSE | NUSIZX_MISSILE_WIDTH_1)
        .unwrap();
    tia.write(NUSIZ1, NUSIZX_ONE_COPY | NUSIZX_MISSILE_WIDTH_1)
        .unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         A0A00A0A00A00000A0A00A0A00A00000A0A00A0A00A0000000000000000000000000000000000000\
         C0C00C0C000C00000000000000000000000000000000000000000000000000000000000000000000",
    );

    tia.write(NUSIZ0, NUSIZX_THREE_COPIES_MEDIUM | NUSIZX_MISSILE_WIDTH_1)
        .unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         A0A00A0A00A000000000000000000000A0A00A0A00A000000000000000000000A0A00A0A00A00000\
         C0C00C0C000C00000000000000000000000000000000000000000000000000000000000000000000",
    );
}

#[test]
fn sprite_scaling() {
    use flags::*;
    use registers::{NUSIZ0, NUSIZ1};

    let mut tia = Tia::new();
    tia.write(registers::COLUP0, 0x0A).unwrap();
    tia.write(registers::COLUP1, 0x0C).unwrap();
    tia.write(registers::GRP0, 0b1010_0101).unwrap();
    tia.write(registers::GRP1, 0b1010_0101).unwrap();
    tia.write(registers::ENAM0, flags::ENAXX_ENABLE).unwrap();
    tia.write(registers::ENAM1, flags::ENAXX_ENABLE).unwrap();

    let p0_delay = 22 * 3;
    let m0_delay = 20 * 3;
    let p1_delay = 7 * 3;
    let m1_delay = 20 * 3;
    wait_ticks(&mut tia, p0_delay);
    tia.write(registers::RESP0, 0).unwrap();
    wait_ticks(&mut tia, m0_delay);
    tia.write(registers::RESM0, 0).unwrap();
    wait_ticks(&mut tia, p1_delay);
    tia.write(registers::RESP1, 0).unwrap();
    wait_ticks(&mut tia, m1_delay);
    tia.write(registers::RESM1, 0).unwrap();
    wait_ticks(
        &mut tia,
        TOTAL_WIDTH - p0_delay - p1_delay - m0_delay - m1_delay,
    );
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000A0A00A0A000000000000000000000000000000000000000000000000000A000000000000000\
         000000C0C00C0C000000000000000000000000000000000000000000000000000C00000000000000",
    );

    tia.write(NUSIZ0, NUSIZX_DOUBLE_SIZED_PLAYER | NUSIZX_MISSILE_WIDTH_1)
        .unwrap();
    // Damn, I don't know if this should be necessary. But right now it is, or
    // the sprite would become "warped".
    wait_ticks(&mut tia, TOTAL_WIDTH);
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         000000AA00AA0000AA00AA000000000000000000000000000000000000000000A000000000000000\
         000000C0C00C0C000000000000000000000000000000000000000000000000000C00000000000000",
    );

    tia.write(registers::HMOVE, 0).unwrap();
    wait_ticks(&mut tia, TOTAL_WIDTH);
    tia.write(registers::HMOVE, 0).unwrap();
    wait_ticks(&mut tia, TOTAL_WIDTH);
    tia.write(registers::HMM0, 0).unwrap();
    tia.write(NUSIZ0, NUSIZX_QUAD_SIZED_PLAYER | NUSIZX_MISSILE_WIDTH_1)
        .unwrap();
    tia.write(NUSIZ1, NUSIZX_DOUBLE_SIZED_PLAYER | NUSIZX_MISSILE_WIDTH_1)
        .unwrap();
    wait_ticks(&mut tia, TOTAL_WIDTH);
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         000000AAAA0000AAAA00000000AAAA0000AAAA00000000000000000000000000A000000000000000\
         0000000CC00CC0000CC00CC000000000000000000000000000000000000000000C00000000000000",
    );
}

#[test]
fn missile_scaling() {
    use flags::*;
    use registers::{NUSIZ0, NUSIZ1};

    let mut tia = Tia::new();
    tia.write(registers::COLUP0, 0x0A).unwrap();
    tia.write(registers::COLUP1, 0x0C).unwrap();
    tia.write(registers::GRP0, 0b1010_0101).unwrap();
    tia.write(registers::GRP1, 0b1010_0101).unwrap();
    tia.write(registers::ENAM0, flags::ENAXX_ENABLE).unwrap();
    tia.write(registers::ENAM1, flags::ENAXX_ENABLE).unwrap();

    let p0_delay = 22 * 3;
    let m0_delay = 20 * 3;
    let p1_delay = 7 * 3;
    let m1_delay = 20 * 3;
    wait_ticks(&mut tia, p0_delay);
    tia.write(registers::RESP0, 0).unwrap();
    wait_ticks(&mut tia, m0_delay);
    tia.write(registers::RESM0, 0).unwrap();
    wait_ticks(&mut tia, p1_delay);
    tia.write(registers::RESP1, 0).unwrap();
    wait_ticks(&mut tia, m1_delay);
    tia.write(registers::RESM1, 0).unwrap();
    wait_ticks(
        &mut tia,
        TOTAL_WIDTH - p0_delay - p1_delay - m0_delay - m1_delay,
    );
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000A0A00A0A000000000000000000000000000000000000000000000000000A000000000000000\
         000000C0C00C0C000000000000000000000000000000000000000000000000000C00000000000000",
    );

    tia.write(NUSIZ0, NUSIZX_ONE_COPY | NUSIZX_MISSILE_WIDTH_2)
        .unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000A0A00A0A000000000000000000000000000000000000000000000000000AA00000000000000\
         000000C0C00C0C000000000000000000000000000000000000000000000000000C00000000000000",
    );

    tia.write(NUSIZ0, NUSIZX_ONE_COPY | NUSIZX_MISSILE_WIDTH_4)
        .unwrap();
    tia.write(NUSIZ1, NUSIZX_DOUBLE_SIZED_PLAYER | NUSIZX_MISSILE_WIDTH_2)
        .unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000A0A00A0A000000000000000000000000000000000000000000000000000AAAA000000000000\
         0000000CC00CC0000CC00CC000000000000000000000000000000000000000000CC0000000000000",
    );

    tia.write(NUSIZ0, NUSIZX_ONE_COPY | NUSIZX_MISSILE_WIDTH_8)
        .unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000A0A00A0A000000000000000000000000000000000000000000000000000AAAAAAAA00000000\
         0000000CC00CC0000CC00CC000000000000000000000000000000000000000000CC0000000000000",
    );
}

#[test]
fn graphics_priorities() {
    let mut tia = Tia::new();
    tia.write(registers::COLUBK, 0x00).unwrap();
    tia.write(registers::COLUPF, 0x02).unwrap();
    tia.write(registers::COLUP0, 0x04).unwrap();
    tia.write(registers::COLUP1, 0x06).unwrap();
    tia.write(registers::PF1, 0b1111_0011).unwrap();
    tia.write(registers::GRP0, 0b1010_1010).unwrap();
    tia.write(registers::GRP1, 0b1111_1111).unwrap();
    tia.write(registers::ENAM0, flags::ENAXX_ENABLE).unwrap();
    tia.write(registers::ENAM1, flags::ENAXX_ENABLE).unwrap();

    let player_delay = 30 * 3;
    let missile_delay = 4 * 3;
    wait_ticks(&mut tia, player_delay);
    tia.write(registers::RESP0, 0).unwrap();
    tia.write(registers::RESP1, 0).unwrap();
    wait_ticks(&mut tia, missile_delay);
    tia.write(registers::RESM0, 0).unwrap();
    tia.write(registers::RESM1, 0).unwrap();
    wait_ticks(&mut tia, TOTAL_WIDTH - player_delay - missile_delay);
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000000000000002222222222222464646460004222222200000000000000000000000000000000\
         00000000000000002222222222222222000000002222222200000000000000000000000000000000",
    );

    tia.write(registers::CTRLPF, flags::CTRLPF_PRIORITY)
        .unwrap();
    assert_eq!(
        encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
        "................||||||||||||||||....................................\
         00000000000000002222222222222222646460002222222200000000000000000000000000000000\
         00000000000000002222222222222222000000002222222200000000000000000000000000000000",
    );
}

#[test]
fn sprite_collisions() {
    let mut tia = Tia::new();
    tia.write(registers::PF1, 0b0000_0100).unwrap();
    tia.write(registers::ENAM0, flags::ENAXX_ENABLE).unwrap();
    tia.write(registers::ENAM1, flags::ENAXX_ENABLE).unwrap();
    tia.write(registers::GRP0, 0b1000_0000).unwrap();
    tia.write(registers::GRP1, 0b1000_0000).unwrap();
    tia.write(registers::VBLANK, flags::VBLANK_ON).unwrap();

    // Position all graphics objects in a way where everything is separated,
    // in order: M0, P0, M1, P1, PF.
    let sprite_delay = 32 * 3;
    wait_ticks(&mut tia, sprite_delay);
    tia.write(registers::RESP0, 0).unwrap();
    tia.write(registers::RESP1, 0).unwrap();
    tia.write(registers::RESM0, 0).unwrap();
    tia.write(registers::RESM1, 0).unwrap();
    wait_ticks(&mut tia, TOTAL_WIDTH - sprite_delay);
    tia.write(registers::HMP0, 2 << 4).unwrap();
    tia.write(registers::HMM0, 2 << 4).unwrap();
    tia.write(registers::HMOVE, 0).unwrap();
    wait_ticks(&mut tia, TOTAL_WIDTH);
    tia.write(registers::VBLANK, 0).unwrap();
    assert_collision_latches(&tia, [0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b00]);

    // M0 goes right, colliding with P0.
    tia.write(registers::HMCLR, 0).unwrap();
    tia.write(registers::HMM0, (-1i8 << 4) as u8).unwrap();
    tia.write(registers::HMOVE, 0).unwrap();
    wait_ticks(&mut tia, TOTAL_WIDTH);
    assert_collision_latches(&tia, [0b01, 0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b00]);

    // M0 and P0 go right, colliding with M1.
    tia.write(registers::HMP0, (-1i8 << 4) as u8).unwrap();
    tia.write(registers::HMOVE, 0).unwrap();
    wait_ticks(&mut tia, TOTAL_WIDTH);
    assert_collision_latches(&tia, [0b01, 0b10, 0b00, 0b00, 0b00, 0b00, 0b00, 0b01]);

    // M0+P0+M1+P1.
    tia.write(registers::HMM1, (-1i8 << 4) as u8).unwrap();
    tia.write(registers::HMOVE, 0).unwrap();
    wait_ticks(&mut tia, TOTAL_WIDTH);
    assert_collision_latches(&tia, [0b11, 0b11, 0b00, 0b00, 0b00, 0b00, 0b00, 0b11]);

    // M0+P0+M1+P1+PF.
    tia.write(registers::HMP1, (-1i8 << 4) as u8).unwrap();
    tia.write(registers::HMOVE, 0).unwrap();
    wait_ticks(&mut tia, TOTAL_WIDTH);
    assert_collision_latches(&tia, [0b11, 0b11, 0b10, 0b10, 0b10, 0b10, 0b00, 0b11]);

    tia.write(registers::CXCLR, 0).unwrap();
    assert_collision_latches(&tia, [0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b00, 0b00]);
}

/// Performs an assertion on the collision registers (0x00-0x07), comparing
/// them to the expected values. For better call site readability, the
/// values are shifted 6 bits left, so the collision bit values are given in
/// lowest 2 bits, and not the highest ones.
fn assert_collision_latches(tia: &Tia, expected: [u8; 8]) {
    let expected = expected.iter().copied().map(|x| x << 6);
    let actual = (0..8).map(|i| tia.read(i).unwrap());
    itertools::assert_equal(actual, expected);
}

#[test]
fn write_address_mirroring() {
    let mut tia = Tia::new();
    wait_ticks(&mut tia, HBLANK_WIDTH);

    tia.write(registers::COLUBK, 0x08).unwrap();
    let output = tia.tick().video;
    assert_eq!(output.pixel.unwrap(), 0x08);

    tia.write(0x6F40 + registers::COLUBK, 0x0A).unwrap();
    let output = tia.tick().video;
    assert_eq!(output.pixel.unwrap(), 0x0A);
}

#[test]
fn read_address_mirroring() {
    let mut tia = Tia::new();
    tia.write(registers::VBLANK, 0).unwrap(); // Disable latching.

    tia.set_port(Port::Input4, true);
    assert_eq!(tia.read(registers::INPT4).unwrap(), flags::INPUT_HIGH);
    assert_eq!(
        tia.read(0x2640 + registers::INPT4).unwrap(),
        flags::INPUT_HIGH
    );
    assert_eq!(
        tia.read(0x2650 + registers::INPT4).unwrap(),
        flags::INPUT_HIGH
    );
}

#[test]
fn unlatched_input_ports() {
    let mut tia = Tia::new();
    tia.write(registers::VBLANK, 0).unwrap(); // Disable latching.

    tia.set_port(Port::Input4, true);
    assert_eq!(tia.read(registers::INPT4).unwrap(), flags::INPUT_HIGH);
    tia.set_port(Port::Input4, false);
    assert_eq!(tia.read(registers::INPT4).unwrap(), 0);
    tia.set_port(Port::Input4, true);
    assert_eq!(tia.read(registers::INPT4).unwrap(), flags::INPUT_HIGH);

    tia.set_port(Port::Input5, true);
    assert_eq!(tia.read(registers::INPT5).unwrap(), flags::INPUT_HIGH);
    tia.set_port(Port::Input5, false);
    assert_eq!(tia.read(registers::INPT5).unwrap(), 0);
    tia.set_port(Port::Input5, true);
    assert_eq!(tia.read(registers::INPT5).unwrap(), flags::INPUT_HIGH);
}

#[test]
fn latched_input_ports() {
    let mut tia = Tia::new();
    tia.set_port(Port::Input4, true);
    tia.write(registers::VBLANK, flags::VBLANK_INPUT_LATCH)
        .unwrap();
    assert_eq!(tia.read(registers::INPT4).unwrap(), flags::INPUT_HIGH);

    // Setting the port to low should latch the value and ignore setting it
    // back to high.
    tia.set_port(Port::Input4, false);
    assert_eq!(tia.read(registers::INPT4).unwrap(), 0);
    tia.set_port(Port::Input4, true);
    assert_eq!(tia.read(registers::INPT4).unwrap(), 0);

    // Unlatching should immediately restore the current value.
    tia.write(registers::VBLANK, 0).unwrap();
    assert_eq!(tia.read(registers::INPT4).unwrap(), flags::INPUT_HIGH);

    // Unlatching should immediately restore the current value.
    tia.write(registers::VBLANK, flags::VBLANK_INPUT_LATCH)
        .unwrap();
    tia.set_port(Port::Input4, false);
    tia.write(registers::VBLANK, 0).unwrap();
    assert_eq!(tia.read(registers::INPT4).unwrap(), 0);
}

#[test]
fn generates_audio() {
    let mut tia = Tia::new();
    tia.write(registers::AUDV0, 15).unwrap();
    tia.write(registers::AUDF0, 0).unwrap();
    tia.write(registers::AUDC0, 4).unwrap();
    assert_eq!(
        encode_audio(scan_audio(&mut tia, 7).map(|a| a.au0)),
        "0F0F0F0",
    );
}

#[test]
fn audio_base_frequency() {
    let mut tia = Tia::new();
    tia.write(registers::AUDV0, 15).unwrap();
    let n_samples = scan_audio_ticks(&mut tia, 5 * TOTAL_WIDTH).count();
    assert_eq!(n_samples, 10);
    let n_samples = scan_audio_ticks(&mut tia, 3 * TOTAL_WIDTH).count();
    assert_eq!(n_samples, 6);
}

#[test]
fn audio_volume() {
    let mut tia = Tia::new();
    tia.write(registers::AUDF0, 0).unwrap();
    tia.write(registers::AUDC0, 4).unwrap();
    tia.write(registers::AUDF1, 0).unwrap();
    tia.write(registers::AUDC1, 4).unwrap();

    tia.write(registers::AUDV0, 6).unwrap();
    tia.write(registers::AUDV1, 10).unwrap();
    let audio: Vec<AudioOutput> = scan_audio(&mut tia, 4).collect();
    assert_eq!(encode_audio(audio.iter().map(|a| a.au0)), "0606");
    assert_eq!(encode_audio(audio.iter().map(|a| a.au1)), "0A0A");

    tia.write(registers::AUDV0, 7).unwrap();
    tia.write(registers::AUDV1, 9).unwrap();
    let audio: Vec<AudioOutput> = scan_audio(&mut tia, 4).collect();
    assert_eq!(encode_audio(audio.iter().map(|a| a.au0)), "0707");
    assert_eq!(encode_audio(audio.iter().map(|a| a.au1)), "0909");

    tia.write(registers::AUDV0, 0).unwrap();
    tia.write(registers::AUDV1, 0).unwrap();
    let audio: Vec<AudioOutput> = scan_audio(&mut tia, 4).collect();
    assert_eq!(encode_audio(audio.iter().map(|a| a.au0)), "0000");
    assert_eq!(encode_audio(audio.iter().map(|a| a.au1)), "0000");
}

#[test]
fn audio_voulume_outside_range() {
    let mut tia = Tia::new();
    tia.write(registers::AUDF0, 0).unwrap();
    tia.write(registers::AUDC0, 4).unwrap();
    tia.write(registers::AUDF1, 0).unwrap();
    tia.write(registers::AUDC1, 4).unwrap();

    tia.write(registers::AUDV0, 0xf7).unwrap();
    tia.write(registers::AUDV1, 0x48).unwrap();
    let audio: Vec<AudioOutput> = scan_audio(&mut tia, 4).collect();
    assert_eq!(encode_audio(audio.iter().map(|a| a.au0)), "0707");
    assert_eq!(encode_audio(audio.iter().map(|a| a.au1)), "0808");
}

#[test]
fn audio_frequency() {
    let mut tia = Tia::new();
    tia.write(registers::AUDC0, 4).unwrap();
    tia.write(registers::AUDC1, 4).unwrap();
    tia.write(registers::AUDV0, 1).unwrap();
    tia.write(registers::AUDV1, 1).unwrap();

    tia.write(registers::AUDF0, 0).unwrap();
    tia.write(registers::AUDF1, 0).unwrap();
    let audio: Vec<AudioOutput> = scan_audio(&mut tia, 4).collect();
    assert_eq!(encode_audio(audio.iter().map(|a| a.au0)), "0101");
    assert_eq!(encode_audio(audio.iter().map(|a| a.au1)), "0101");

    tia.write(registers::AUDF0, 2).unwrap();
    tia.write(registers::AUDF1, 4).unwrap();
    let audio: Vec<AudioOutput> = scan_audio(&mut tia, 12).collect();
    assert_eq!(encode_audio(audio.iter().map(|a| a.au0)), "000111000111");
    assert_eq!(encode_audio(audio.iter().map(|a| a.au1)), "000001111100");
}
