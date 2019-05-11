#[derive(Clone)]
pub struct TIA {
    // Registers
    reg_vsync: u8,
    reg_vblank: u8,
    reg_colubk: u8,

    column: i32,
    scanline: i32,
    hblank_on: bool,
    hsync_on: bool,
}

impl TIA {
    pub fn new() -> TIA {
        TIA {
            reg_vsync: 0,
            reg_vblank: 0,
            reg_colubk: 0,
            column: 0,
            scanline: 0,
            hsync_on: false,
            hblank_on: false,
        }
    }

    pub fn tick(&mut self) -> Output {
        match self.column {
            0 => self.hblank_on = true,
            HSYNC_START => self.hsync_on = true,
            HSYNC_END => self.hsync_on = false,
            HBLANK_WIDTH => self.hblank_on = false,
            _ => {}
        }

        let vsync_on = self.reg_vsync & flags::VSYNC_ON != 0;
        let vblank_on = self.reg_vblank & flags::VBLANK_ON != 0;
        let output = Output {
            hsync: self.hsync_on,
            vsync: vsync_on,
            pixel: if self.hblank_on || vblank_on {
                None
            } else {
                Some(self.reg_colubk)
            },
        };

        self.column = (self.column + 1) % TOTAL_WIDTH;
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

// We need to derive PartialEq to easily perform assertions in tests.
#[derive(PartialEq, Clone, Debug)]
pub struct Output {
    vsync: bool,
    hsync: bool,
    pixel: Option<u8>,
}

impl Output {
    pub fn pixel(pixel: u8) -> Output {
        Output {
            vsync: false,
            hsync: false,
            pixel: Some(pixel),
        }
    }

    pub fn blank() -> Output {
        Output {
            vsync: false,
            hsync: false,
            pixel: None,
        }
    }

    pub fn with_hsync(mut self) -> Self {
        self.hsync = true;
        self
    }

    pub fn with_vsync(mut self) -> Self {
        self.vsync = true;
        self
    }
}

const HSYNC_START: i32 = 16;
const HSYNC_END: i32 = 32; // 1 cycle after, to make it easy to construct a range.
const HBLANK_WIDTH: i32 = 68;
const FRAME_WIDTH: i32 = 160;
const TOTAL_WIDTH: i32 = FRAME_WIDTH + HBLANK_WIDTH;

// const FRAME_HEIGHT: i32 = 192;
// const VSYNC_HEIGHT: i32 = 3;
// const V_BLANK_HEIGHT: i32 = 37;
// const OVERSCAN_HEIGHT: i32 = 30;
// const TOTAL_HEIGHT: i32 = FRAME_HEIGHT + VSYNC_HEIGHT + V_BLANK_HEIGHT;

mod registers {
    pub const VSYNC: u16 = 0x00;
    pub const VBLANK: u16 = 0x01;
    pub const COLUBK: u16 = 0x09;
}

mod flags {
    pub const VSYNC_ON: u8 = 0b0000_0010;
    pub const VBLANK_ON: u8 = 0b0000_0010;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct OutputIterator<'a> {
        tia: &'a mut TIA,
    }

    impl<'a> Iterator for OutputIterator<'a> {
        type Item = Output;

        fn next(&mut self) -> Option<Output> {
            return Some(self.tia.tick());
        }
    }

    #[test]
    fn draws_background_pixels() {
        let mut tia = TIA::new();
        for _ in 0..HBLANK_WIDTH {
            tia.tick();
        }

        tia.write(registers::COLUBK, 0x02);
        assert_eq!(tia.tick(), Output::pixel(0x02));

        tia.write(registers::COLUBK, 0xfe);
        assert_eq!(tia.tick(), Output::pixel(0xfe));
    }

    #[test]
    fn draws_scanlines() {
        let mut expected_output = Vec::new();
        expected_output.resize(HSYNC_START as usize, Output::blank());
        expected_output.resize(HSYNC_END as usize, Output::blank().with_hsync());
        expected_output.resize(HBLANK_WIDTH as usize, Output::blank());
        expected_output.resize(TOTAL_WIDTH as usize, Output::pixel(0x80));
        expected_output.append(&mut expected_output.clone());

        let mut tia = TIA::new();
        tia.write(registers::COLUBK, 0x80);
        let output = OutputIterator { tia: &mut tia }.take(2 * TOTAL_WIDTH as usize);
        itertools::assert_equal(output, expected_output);
    }

    #[test]
    fn emits_vsync() {
        let mut expected_output = Vec::new();
        expected_output.resize(HSYNC_START as usize, Output::blank().with_vsync());
        expected_output.resize(
            HSYNC_END as usize,
            Output::blank().with_vsync().with_hsync(),
        );
        expected_output.resize(HBLANK_WIDTH as usize, Output::blank().with_vsync());
        expected_output.resize(TOTAL_WIDTH as usize, Output::pixel(0x12).with_vsync());

        let mut tia = TIA::new();
        tia.write(registers::COLUBK, 0x12);
        tia.write(registers::VSYNC, flags::VSYNC_ON);
        let output = OutputIterator { tia: &mut tia }.take(TOTAL_WIDTH as usize);
        itertools::assert_equal(output, expected_output);

        // Note: we turn off VSYNC not by writing 0, but by setting all bits but
        // bit 1. This is to make sure that all other bits are ignored.
        tia.write(registers::VSYNC, !flags::VSYNC_ON);
        assert_eq!(tia.tick(), Output::blank());
    }

    #[test]
    fn emits_vblank() {
        let mut expected_output = Vec::new();
        expected_output.resize(HSYNC_START as usize, Output::blank());
        expected_output.resize(HSYNC_END as usize, Output::blank().with_hsync());
        expected_output.resize(TOTAL_WIDTH as usize, Output::blank());

        let mut tia = TIA::new();
        tia.write(registers::COLUBK, 0x32);
        tia.write(registers::VBLANK, flags::VBLANK_ON);
        let output = OutputIterator { tia: &mut tia }.take(TOTAL_WIDTH as usize);
        itertools::assert_equal(output, expected_output);

        // Make sure that only bit 1 of VBLANK counts.
        tia.write(registers::VBLANK, !flags::VBLANK_ON);
        for _ in 0..HBLANK_WIDTH {
            tia.tick();
        }
        assert_eq!(tia.tick(), Output::pixel(0x32));
    }

    #[test]
    fn emits_vblank_with_vsync() {
        let mut expected_output = Vec::new();
        expected_output.resize(HSYNC_START as usize, Output::blank().with_vsync());
        expected_output.resize(
            HSYNC_END as usize,
            Output::blank().with_vsync().with_hsync(),
        );
        expected_output.resize(TOTAL_WIDTH as usize, Output::blank().with_vsync());

        let mut tia = TIA::new();
        tia.write(registers::VSYNC, flags::VSYNC_ON);
        tia.write(registers::VBLANK, flags::VBLANK_ON);
        let output = OutputIterator { tia: &mut tia }.take(TOTAL_WIDTH as usize);
        itertools::assert_equal(output, expected_output);
    }
}
