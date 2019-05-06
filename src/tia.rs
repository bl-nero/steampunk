pub struct TIA {
    // Registers
    reg_colubk: u8,

    column: i32,
    scanline: i32,
}

impl TIA {
    pub fn new() -> TIA {
        TIA {
            reg_colubk: 0,
            column: 0,
            scanline: 0,
        }
    }

    pub fn tick(&mut self) -> TickResult {
        let vo = self.video_output();
        self.column += 1;
        return vo;
    }

    fn video_output(&self) -> TickResult {
        if (self.column < 16) {
            return TickResult::empty();
        }
        if (self.column < 32) {
            return TickResult {
                horizontal_sync: true,
                vertical_sync: false,
                pixel: None,
            };
        }
        if (self.column < H_BLANK_WIDTH) {
            return TickResult::empty();
        }

        return TickResult::from_pixel(self.reg_colubk);
    }

    pub fn read(&self, address: u16) -> u8 {
        0
    }

    pub fn write(&mut self, address: u16, value: u8) {
        self.reg_colubk = value;
    }
}

// We need to derive PartialEq to easily perform assertions in tests.
#[derive(PartialEq, Debug)]
pub struct TickResult {
    vertical_sync: bool,
    horizontal_sync: bool,
    pixel: Option<u8>,
}

impl TickResult {
    pub fn from_pixel(pixel: u8) -> TickResult {
        TickResult {
            vertical_sync: false,
            horizontal_sync: false,
            pixel: Some(pixel),
        }
    }

    pub fn empty() -> TickResult {
        TickResult {
            vertical_sync: false,
            horizontal_sync: false,
            pixel: None,
        }
    }
}

const FRAME_WIDTH: i32 = 160;
const H_BLANK_WIDTH: i32 = 68;
const TOTAL_WIDTH: i32 = FRAME_WIDTH + H_BLANK_WIDTH;

const FRAME_HEIGHT: i32 = 192;
const VSYNC_HEIGHT: i32 = 3;
const V_BLANK_HEIGHT: i32 = 37;
const OVERSCAN_HEIGHT: i32 = 30;
const TOTAL_HEIGHT: i32 = FRAME_HEIGHT + VSYNC_HEIGHT + V_BLANK_HEIGHT;

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

    #[test]
    fn draws_background_pixels() {
        let mut tia = TIA::new();
        for _ in 0..H_BLANK_WIDTH {
            tia.tick();
        }

        tia.write(registers::COLUBK, 0x02);
        assert_eq!(TickResult::from_pixel(0x02), tia.tick());

        tia.write(registers::COLUBK, 0xFE);
        assert_eq!(TickResult::from_pixel(0xFE), tia.tick());
    }

    #[test]
    fn generates_hsync() {
        let mut tia = TIA::new();
        tia.write(registers::COLUBK, 0x80);
        for i in 0..16 {
            assert_eq!(TickResult::empty(), tia.tick(), "at index {}", i);
        }
        for i in 16..32 {
            assert_eq!(
                TickResult {
                    horizontal_sync: true,
                    vertical_sync: false,
                    pixel: None
                },
                tia.tick(),
                "at index {}",
                i
            );
        }
        for i in 32..H_BLANK_WIDTH {
            assert_eq!(TickResult::empty(), tia.tick(), "at index {}", i);
        }
    }
}
