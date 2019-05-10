#[derive(Clone)]
pub struct TIA {
    // Registers
    reg_vsync: u8,
    reg_colubk: u8,

    column: i32,
    scanline: i32,
}

impl TIA {
    pub fn new() -> TIA {
        TIA {
            reg_vsync: 0,
            reg_colubk: 0,
            column: 0,
            scanline: 0,
        }
    }

    pub fn tick(&mut self) -> Output {
        let vo = self.video_output();
        self.column = (self.column + 1) % TOTAL_WIDTH;
        return vo;
    }

    fn video_output(&self) -> Output {
        let vsync = self.reg_vsync & flags::VSYNC_ON != 0;
        if self.column < H_SYNC_START {
            return Output {
                horizontal_sync: false,
                vertical_sync: vsync,
                pixel: None,
            };
        }
        if self.column < H_SYNC_END {
            return Output {
                horizontal_sync: true,
                vertical_sync: vsync,
                pixel: None,
            };
        }
        if self.column < H_BLANK_WIDTH {
            return Output {
                horizontal_sync: false,
                vertical_sync: vsync,
                pixel: None,
            };
        }

        return Output {
            horizontal_sync: false,
            vertical_sync: vsync,
            pixel: if vsync { None } else { Some(self.reg_colubk) },
        };
    }

    pub fn read(&self, address: u16) -> u8 {
        0
    }

    pub fn write(&mut self, address: u16, value: u8) {
        match address {
            registers::COLUBK => self.reg_colubk = value,
            registers::VSYNC => self.reg_vsync = value,
            _ => {}
        }
    }
}

// We need to derive PartialEq to easily perform assertions in tests.
#[derive(PartialEq, Clone, Debug)]
pub struct Output {
    vertical_sync: bool,
    horizontal_sync: bool,
    pixel: Option<u8>,
}

impl Output {
    pub fn from_pixel(pixel: u8) -> Output {
        Output {
            vertical_sync: false,
            horizontal_sync: false,
            pixel: Some(pixel),
        }
    }

    pub fn empty() -> Output {
        Output {
            vertical_sync: false,
            horizontal_sync: false,
            pixel: None,
        }
    }

    pub fn with_horizontal_sync(mut self) -> Self {
        self.horizontal_sync = true;
        self
    }

    pub fn with_vertical_sync(mut self) -> Self {
        self.vertical_sync = true;
        self
    }
}

const H_SYNC_START: i32 = 16;
const H_SYNC_END: i32 = 32; // 1 cycle after, to make it easy to construct a range.
const H_BLANK_WIDTH: i32 = 68;
const FRAME_WIDTH: i32 = 160;
const TOTAL_WIDTH: i32 = FRAME_WIDTH + H_BLANK_WIDTH;

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
        for _ in 0..H_BLANK_WIDTH {
            tia.tick();
        }

        tia.write(registers::COLUBK, 0x02);
        assert_eq!(tia.tick(), Output::from_pixel(0x02));

        tia.write(registers::COLUBK, 0xfe);
        assert_eq!(tia.tick(), Output::from_pixel(0xfe));
    }

    #[test]
    fn draws_scanlines() {
        let mut expected_output = Vec::new();
        expected_output.resize(H_SYNC_START as usize, Output::empty());
        expected_output.resize(H_SYNC_END as usize, Output::empty().with_horizontal_sync());
        expected_output.resize(H_BLANK_WIDTH as usize, Output::empty());
        expected_output.resize(TOTAL_WIDTH as usize, Output::from_pixel(0x80));
        expected_output.append(&mut expected_output.clone());

        let mut tia = TIA::new();
        tia.write(registers::COLUBK, 0x80);
        let output = OutputIterator { tia: &mut tia }.take(2 * TOTAL_WIDTH as usize);
        itertools::assert_equal(output, expected_output);
    }

    #[test]
    fn emits_vsync() {
        let mut expected_output = Vec::new();
        expected_output.resize(H_SYNC_START as usize, Output::empty().with_vertical_sync());
        expected_output.resize(
            H_SYNC_END as usize,
            Output::empty().with_vertical_sync().with_horizontal_sync(),
        );
        expected_output.resize(H_BLANK_WIDTH as usize, Output::empty().with_vertical_sync());
        expected_output.resize(
            TOTAL_WIDTH as usize,
            Output::from_pixel(0x12).with_vertical_sync(),
        );

        let mut tia = TIA::new();
        tia.write(registers::COLUBK, 0x12);
        tia.write(registers::VSYNC, flags::VSYNC_ON);
        let output = OutputIterator { tia: &mut tia }.take(TOTAL_WIDTH as usize);
        itertools::assert_equal(output, expected_output);

        tia.write(registers::VSYNC, !flags::VSYNC_ON);
        assert_eq!(tia.tick(), Output::empty());
    }
}
