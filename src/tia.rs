pub struct TIA {
    // *** REGISTERS ***
    /// If bit 1 (`flags::VSYNC_ON`) is set, TIA emits a VSYNC signal.
    reg_vsync: u8,
    /// If bit 1 (`flags::VBLANK_ON`) is set, TIA doesn't emit pixels.
    reg_vblank: u8,
    /// Color and luminance of background. See
    /// [`VideoOutput::pixel`](struct.VideoOutput.html#structfield.pixel) for details.
    reg_colubk: u8,

    /// Each frame has 228 cycles, including 160 cycles that actually emit
    /// pixels.
    column_counter: u32,
    hblank_on: bool,
    hsync_on: bool,
}

impl TIA {
    pub fn new() -> TIA {
        TIA {
            reg_vsync: 0,
            reg_vblank: 0,
            reg_colubk: 0,
            column_counter: 0,
            hsync_on: false,
            hblank_on: false,
        }
    }

    /// Processes a single TIA clock cycle. Returns a TIA output structure. A
    /// single cycle is the time needed to render a single pixel.
    pub fn tick(&mut self) -> TIAOutput {
        match self.column_counter {
            0 => self.hblank_on = true,
            HSYNC_START => self.hsync_on = true,
            HSYNC_END => self.hsync_on = false,
            HBLANK_WIDTH => self.hblank_on = false,
            _ => {}
        }

        let vsync_on = self.reg_vsync & flags::VSYNC_ON != 0;
        let vblank_on = self.reg_vblank & flags::VBLANK_ON != 0;
        let output = TIAOutput {
            video: VideoOutput {
                hsync: self.hsync_on,
                vsync: vsync_on,
                pixel: if self.hblank_on || vblank_on {
                    None
                } else {
                    Some(self.reg_colubk)
                },
            },
        };

        self.column_counter = (self.column_counter + 1) % TOTAL_WIDTH;
        return output;
    }

    pub fn read(&self, address: u16) -> u8 {
        0
    }

    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            registers::VSYNC => self.reg_vsync = value,
            registers::VBLANK => self.reg_vblank = value,
            registers::COLUBK => self.reg_colubk = value,
            _ => {}
        }
    }
}

pub struct TIAOutput {
    pub video: VideoOutput,
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

impl VideoOutput {
    /// Creates a new `VideoOutput` instance that contains pixel with a given
    /// color. See [`pixel`](#structfield.pixel) for details.
    pub fn pixel(pixel: u8) -> VideoOutput {
        VideoOutput {
            vsync: false,
            hsync: false,
            pixel: Some(pixel),
        }
    }

    /// Creates a new blank `VideoOutput` that doesn't contain any signals or
    /// pixel color.
    pub fn blank() -> VideoOutput {
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
    pub const COLUBK: u16 = 0x09;
}

/// Constants in this module are bit masks for setting and testing register
/// values.
pub mod flags {
    /// Bit mask for turning on VSYNC signal using `VSYNC` registry.
    pub const VSYNC_ON: u8 = 0b0000_0010;
    /// Bit mask for turning on vertical blanking using `VBLANK` registry.
    pub const VBLANK_ON: u8 = 0b0000_0010;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;

    /// A utility that produces a sequence of TIA video outputs. Useful for
    /// comparing with expected sequences in tests.
    struct VideoOutputIterator<'a> {
        tia: &'a mut TIA,
    }

    impl<'a> Iterator for VideoOutputIterator<'a> {
        type Item = VideoOutput;

        fn next(&mut self) -> Option<VideoOutput> {
            return Some(self.tia.tick().video);
        }
    }

    #[test]
    fn draws_background_pixels() {
        let mut tia = TIA::new();
        for _ in 0..HBLANK_WIDTH {
            tia.tick();
        }

        tia.write(registers::COLUBK, 0x02);
        assert_eq!(tia.tick().video, VideoOutput::pixel(0x02));

        tia.write(registers::COLUBK, 0xfe);
        assert_eq!(tia.tick().video, VideoOutput::pixel(0xfe));
    }

    #[test]
    fn draws_scanlines() {
        let expected_output = test_utils::decode_video_outputs(
            "................||||||||||||||||....................................\
             88888888888888888888888888888888888888888888888888888888888888888888888888888888\
             88888888888888888888888888888888888888888888888888888888888888888888888888888888\
             ................||||||||||||||||....................................\
             88888888888888888888888888888888888888888888888888888888888888888888888888888888\
             88888888888888888888888888888888888888888888888888888888888888888888888888888888",
        );

        let mut tia = TIA::new();
        tia.write(registers::COLUBK, 0x08);
        // Generate two scanlines (2 * TOTAL_WIDTH clock cycles).
        let output = VideoOutputIterator { tia: &mut tia }.take(2 * TOTAL_WIDTH as usize);
        itertools::assert_equal(output, expected_output);
    }

    #[test]
    fn emits_vsync() {
        let expected_output = test_utils::decode_video_outputs(
            "----------------++++++++++++++++------------------------------------\
             ================================================================================\
             ================================================================================",
        );

        let mut tia = TIA::new();
        tia.write(registers::COLUBK, 0x00);
        tia.write(registers::VSYNC, flags::VSYNC_ON);
        let output = VideoOutputIterator { tia: &mut tia }.take(TOTAL_WIDTH as usize);
        itertools::assert_equal(output, expected_output);

        // Note: we turn off VSYNC not by writing 0, but by setting all bits but
        // bit 1. This is to make sure that all other bits are ignored.
        tia.write(registers::VSYNC, !flags::VSYNC_ON);
        assert_eq!(tia.tick().video, VideoOutput::blank());
    }

    #[test]
    fn emits_vblank() {
        let expected_output = test_utils::decode_video_outputs(
            "................||||||||||||||||....................................\
             ................................................................................\
             ................................................................................",
        );

        let mut tia = TIA::new();
        tia.write(registers::COLUBK, 0x32);
        tia.write(registers::VBLANK, flags::VBLANK_ON);
        let output = VideoOutputIterator { tia: &mut tia }.take(TOTAL_WIDTH as usize);
        itertools::assert_equal(output, expected_output);

        // Make sure that only bit 1 of VBLANK counts.
        tia.write(registers::VBLANK, !flags::VBLANK_ON);
        for _ in 0..HBLANK_WIDTH {
            tia.tick();
        }
        assert_eq!(tia.tick().video, VideoOutput::pixel(0x32));
    }

    #[test]
    fn emits_vblank_with_vsync() {
        let expected_output = test_utils::decode_video_outputs(
            "----------------++++++++++++++++------------------------------------\
             --------------------------------------------------------------------------------\
             --------------------------------------------------------------------------------",
        );

        let mut tia = TIA::new();
        tia.write(registers::VSYNC, flags::VSYNC_ON);
        tia.write(registers::VBLANK, flags::VBLANK_ON);
        let output = VideoOutputIterator { tia: &mut tia }.take(TOTAL_WIDTH as usize);
        itertools::assert_equal(output, expected_output);
    }
}
