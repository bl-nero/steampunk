use crate::memory::{Memory, ReadError, ReadResult, WriteError, WriteResult};

/// TIA is responsible for generating the video signal, sound (not yet
/// implemented) and for synchronizing CPU with the screen's electron beam.
#[derive(Debug)]
pub struct Tia {
    // *** REGISTERS ***
    /// If bit 1 (`flags::VSYNC_ON`) is set, TIA emits a VSYNC signal.
    reg_vsync: u8,
    /// If bit 1 (`flags::VBLANK_ON`) is set, TIA doesn't emit pixels.
    reg_vblank: u8,
    /// Color and luminance of playfield. See
    /// [`VideoOutput::pixel`](struct.VideoOutput.html#structfield.pixel) for details.
    reg_colupf: u8,
    /// Color and luminance of background. See
    /// [`VideoOutput::pixel`](struct.VideoOutput.html#structfield.pixel) for details.
    reg_colubk: u8,
    /// Playfield control register. Responsible for reflecting playfield,
    /// playfield score mode, playfield priority, and ball size.
    reg_ctrlpf: u8,
    /// Playfield register 0 (leftmost 4 bits, mirrored).
    reg_pf0: u8,
    /// Playfield register 1 (middle 8 bits).
    reg_pf1: u8,
    /// Playfield register 2 (rightmost 8 bits, mirrored).
    reg_pf2: u8,

    /// Each frame has 228 cycles, including 160 cycles that actually emit
    /// pixels.
    column_counter: u32,
    /// Indicates whether a horizontal blank signal is being generated.
    hblank_on: bool,
    /// Indicates whether a horizontal sync signal is being generated.
    hsync_on: bool,
    /// Holds CPU ticks until we reach the end of a scanline.
    wait_for_sync: bool,

    // player_0_pos: u32,
    // player_1_pos: u32,
    // missile_0_pos: u32,
    // missile_1_pos: u32,
    // ball_pos: u32,
    // A temporary hack to allow one-time initialization before complaining each
    // time a register is written to.
    initialized_registers: [bool; 0x100],
}

impl Tia {
    pub fn new() -> Tia {
        Tia {
            reg_vsync: 0,
            reg_vblank: 0,
            reg_colupf: 0,
            reg_colubk: 0,
            reg_ctrlpf: 0,
            reg_pf0: 0,
            reg_pf1: 0,
            reg_pf2: 0,

            column_counter: 0,
            hsync_on: false,
            hblank_on: false,
            wait_for_sync: false,
            // player_0_pos: 0,
            // player_1_pos: 0,
            // missile_0_pos: 0,
            // missile_1_pos: 0,
            // ball_pos: 0,
            initialized_registers: [false; 0x100],
        }
    }

    /// Processes a single TIA clock cycle. Returns a TIA output structure. A
    /// single cycle is the time needed to render a single pixel.
    pub fn tick(&mut self) -> TiaOutput {
        match self.column_counter {
            0 => {
                self.hblank_on = true;
                self.wait_for_sync = false;
            }
            HSYNC_START => self.hsync_on = true,
            HSYNC_END => self.hsync_on = false,
            HBLANK_WIDTH => self.hblank_on = false,
            _ => {}
        }

        let vsync_on = self.reg_vsync & flags::VSYNC_ON != 0;
        let vblank_on = self.reg_vblank & flags::VBLANK_ON != 0;
        let output = TiaOutput {
            video: VideoOutput {
                hsync: self.hsync_on,
                vsync: vsync_on,
                pixel: if self.hblank_on || vblank_on {
                    None
                } else {
                    Some(self.color_at(self.column_counter - HBLANK_WIDTH))
                },
            },
            riot_tick: self.column_counter % 3 == 0,
            cpu_tick: !self.wait_for_sync && self.column_counter % 3 == 0,
        };

        self.column_counter = (self.column_counter + 1) % TOTAL_WIDTH;
        return output;
    }

    fn color_at(&self, x: u32) -> u8 {
        let x_playfield = self.playfield_x(x);
        let mask = match x_playfield {
            0..=3 => 0b0001_0000 << x_playfield,
            4..=11 => 0b1000_0000 >> (x_playfield - 4),
            12..=19 => 0b0000_0001 << (x_playfield - 12),
            _ => 0,
        };
        let playfield_register_value = match x_playfield {
            0..=3 => self.reg_pf0,
            4..=11 => self.reg_pf1,
            12..=19 => self.reg_pf2,
            _ => 0,
        };

        return if mask & playfield_register_value != 0 {
            self.reg_colupf
        } else {
            self.reg_colubk
        };
    }

    /// Returns a playfield pixel X coordinate from a [0, 20) range for a
    /// full-resolution X coordinate (starting from the left edge of the visible
    /// screen). The resulting value can be directly used to access the playfield
    /// registers, because it takes into consideration playfield reflection.
    fn playfield_x(&self, x: u32) -> u32 {
        // Playfield has 4 times lower resolution than other stuff.
        let x = x / 4;
        return if x < 20 {
            x // Left half of the screen.
        } else {
            // Right half of the screen.
            if self.reg_ctrlpf & flags::CTRLPF_REFLECT == 0 {
                x - 20 // Normal mode (repeat the left half).
            } else {
                39 - x // Reflected mode (reflect the left half).
            }
        };
    }
}

impl Memory for Tia {
    fn read(&self, address: u16) -> ReadResult {
        Err(ReadError { address })
    }

    fn write(&mut self, address: u16, value: u8) -> WriteResult {
        match address & 0b0001_1111 {
            registers::VSYNC => self.reg_vsync = value,
            registers::VBLANK => self.reg_vblank = value,
            registers::WSYNC => self.wait_for_sync = true,
            registers::RSYNC => self.column_counter = TOTAL_WIDTH - 3,
            registers::COLUPF => self.reg_colupf = value,
            registers::COLUBK => self.reg_colubk = value,
            registers::CTRLPF => self.reg_ctrlpf = value,
            registers::PF0 => self.reg_pf0 = value,
            registers::PF1 => self.reg_pf1 = value,
            registers::PF2 => self.reg_pf2 = value,

            // Audio. Skip that thing for now, since it's complex and not
            // essential.
            registers::AUDC0
            | registers::AUDC1
            | registers::AUDV0
            | registers::AUDV1
            | registers::AUDF0
            | registers::AUDF1 => {}

            // Not (yet) supported. Allow one initialization pass, but that's it.
            _ => {
                if self.initialized_registers[address as usize] || value != 0 {
                    return Err(WriteError { address, value });
                }
                self.initialized_registers[address as usize] = true;
            }
        }
        Ok(())
    }
}

/// TIA output structure. It indicates how a single TIA clock tick influences
/// other parts of the system.
pub struct TiaOutput {
    pub video: VideoOutput,
    /// If `true`, TIA allows CPU to perform a tick. Otherwise, the CPU is put on
    /// hold.
    pub cpu_tick: bool,
    /// If `true`, TIA tells RIOT to perform a tick.
    pub riot_tick: bool,
}

/// TIA video output. The TIA chip actually produces a composite sync signal, but
/// it doesn't make sense to encode it only to decode it downstream in the
/// emulation process.
///
/// Note: We need to derive `PartialEq` to easily perform assertions in tests.
#[derive(PartialEq, Clone, Debug)]
pub struct VideoOutput {
    /// If set to `true`, the vertical synchronization signal is being emitted.
    pub vsync: bool,
    /// If set to `true`, the horizontal synchronization signal is being emitted.
    pub hsync: bool,
    /// If outside horizontal and vertical blanking area, this field contains a
    /// currently emitted pixel. Bits 7-4 denote color, bits 3-1 are the
    /// luminance. Bit 0 is unused.
    pub pixel: Option<u8>,
}

#[cfg(test)]
impl VideoOutput {
    /// Creates a new `VideoOutput` instance that contains pixel with a given
    /// color. See [`pixel`](#structfield.pixel) for details.
    pub fn pixel(pixel: u8) -> Self {
        VideoOutput {
            vsync: false,
            hsync: false,
            pixel: Some(pixel),
        }
    }

    /// Creates a new blank `VideoOutput` that doesn't contain any signals or
    /// pixel color.
    pub fn blank() -> Self {
        VideoOutput {
            vsync: false,
            hsync: false,
            pixel: None,
        }
    }

    /// Sets the HSYNC flag on an existing `VideoOutput` instance.
    pub fn with_hsync(mut self) -> Self {
        self.hsync = true;
        self
    }

    /// Sets the VSYNC flag on an existing `VideoOutput` instance.
    pub fn with_vsync(mut self) -> Self {
        self.vsync = true;
        self
    }
}

// Some constants that describe the scanline geometry.
pub const HSYNC_START: u32 = 16;
pub const HSYNC_END: u32 = 32; // 1 cycle after, to make it easy to construct a range.
pub const HBLANK_WIDTH: u32 = 68;
pub const FRAME_WIDTH: u32 = 160;
pub const TOTAL_WIDTH: u32 = FRAME_WIDTH + HBLANK_WIDTH;

// On the second thought, these constants will probably be more needed
// elsewhere...
// const FRAME_HEIGHT: i32 = 192;
// const VSYNC_HEIGHT: i32 = 3;
// const V_BLANK_HEIGHT: i32 = 37;
// const OVERSCAN_HEIGHT: i32 = 30;
// const TOTAL_HEIGHT: i32 = FRAME_HEIGHT + VSYNC_HEIGHT + V_BLANK_HEIGHT;

/// Constants in this module represent addresses of TIA registers. To be used
/// with the `TIA::read()` and `TIA::write()` methods.
pub mod registers {
    pub const VSYNC: u16 = 0x00;
    pub const VBLANK: u16 = 0x01;
    pub const WSYNC: u16 = 0x02;
    pub const RSYNC: u16 = 0x03;
    // pub const NUSIZ0: u16 = 0x04;
    // pub const NUSIZ1: u16 = 0x05;
    // pub const COLUP0: u16 = 0x06;
    // pub const COLUP1: u16 = 0x07;
    pub const COLUPF: u16 = 0x08;
    pub const COLUBK: u16 = 0x09;
    pub const CTRLPF: u16 = 0x0A;
    // pub const REFP0: u16 = 0x0B;
    // pub const REFP1: u16 = 0x0C;
    pub const PF0: u16 = 0x0D;
    pub const PF1: u16 = 0x0E;
    pub const PF2: u16 = 0x0F;
    // pub const RESP0: u16 = 0x10;
    // pub const RESP1: u16 = 0x11;
    // pub const RESM0: u16 = 0x12;
    // pub const RESM1: u16 = 0x13;
    // pub const RESBL: u16 = 0x14;
    pub const AUDC0: u16 = 0x15;
    pub const AUDC1: u16 = 0x16;
    pub const AUDF0: u16 = 0x17;
    pub const AUDF1: u16 = 0x18;
    pub const AUDV0: u16 = 0x19;
    pub const AUDV1: u16 = 0x1A;
    // pub const GRP0: u16 = 0x1B;
    // pub const GRP1: u16 = 0x1C;
    // pub const ENAM0: u16 = 0x1D;
    // pub const ENAM1: u16 = 0x1E;
    // pub const ENABL: u16 = 0x1F;
    // pub const HMP0: u16 = 0x20;
    // pub const HMP1: u16 = 0x21;
    // pub const HMM0: u16 = 0x22;
    // pub const HMM1: u16 = 0x23;
    // pub const HMBL: u16 = 0x24;
    // pub const VDELP0: u16 = 0x25;
    // pub const VDELP1: u16 = 0x26;
    // pub const VDELBL: u16 = 0x27;
    // pub const RESMP0: u16 = 0x28;
    // pub const RESMP1: u16 = 0x29;
    // pub const HMOVE: u16 = 0x2A;
    // pub const HMCLR: u16 = 0x2B;
    // pub const CXCLR: u16 = 0x2C;
}

/// Constants in this module are bit masks for setting and testing register
/// values.
pub mod flags {
    /// Bit mask for turning on VSYNC signal using `VSYNC` registry.
    pub const VSYNC_ON: u8 = 0b0000_0010;
    /// Bit mask for turning on vertical blanking using `VBLANK` registry.
    pub const VBLANK_ON: u8 = 0b0000_0010;
    /// Bit mask for turning on reflected playfield using `CTRLPF` registry.
    pub const CTRLPF_REFLECT: u8 = 0b0000_0001;
    /// Bit mask for turning on the playfield score mode using `CTRLPF` registry.
    #[cfg(test)]
    pub const CTRLPF_SCORE: u8 = 0b0000_0010;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::decode_video_outputs;

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

    #[test]
    fn draws_background_pixels() {
        let mut tia = Tia::new();
        for _ in 0..HBLANK_WIDTH {
            tia.tick();
        }

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
        for _ in 0..HBLANK_WIDTH {
            tia.tick();
        }
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
        let expected_output = decode_video_outputs(
            "................||||||||||||||||....................................\
             22220000222222222222000000002222222222220000222222220000222200002222222200002222\
             22220000222222222222000000002222222222220000222222220000222200002222222200002222",
        );

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
        // Generate two scanlines (2 * TOTAL_WIDTH clock cycles).
        let output = VideoOutputIterator { tia: &mut tia }.take(TOTAL_WIDTH as usize);
        itertools::assert_equal(output, expected_output);
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
    fn address_mirroring() {
        let mut tia = Tia::new();
        for _ in 0..HBLANK_WIDTH {
            tia.tick();
        }

        tia.write(registers::COLUBK, 0x08).unwrap();
        let output = tia.tick().video;
        assert_eq!(output.pixel.unwrap(), 0x08);

        tia.write(0x6F40 + registers::COLUBK, 0x0A).unwrap();
        let output = tia.tick().video;
        assert_eq!(output.pixel.unwrap(), 0x0A);
    }
}
