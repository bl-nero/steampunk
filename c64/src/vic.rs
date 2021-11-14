use ya6502::memory::Memory;
use ya6502::memory::WriteError;

pub struct Vic {
    reg_border_color: u8,
    reg_background_color: u8,

    raster_counter: usize,
    x_counter: usize,
}

impl Vic {
    pub fn new() -> Self {
        Self {
            reg_border_color: 0,
            reg_background_color: 0,

            raster_counter: 0,
            x_counter: 0,
        }
    }
    pub fn tick(&mut self) -> u8 {
        const DISPLAY_WINDOW_LAST_LINE: usize = BOTTOM_BORDER_FIRST_LINE - 1;
        const DISPLAY_WINDOW_END: usize = RIGHT_BORDER_START - 1;
        let color = match self.raster_counter {
            DISPLAY_WINDOW_FIRST_LINE..=DISPLAY_WINDOW_LAST_LINE => match self.x_counter {
                DISPLAY_WINDOW_START..=DISPLAY_WINDOW_END => self.reg_background_color,
                _ => self.reg_border_color,
            },
            _ => self.reg_border_color,
        };
        self.x_counter += 1;
        if self.x_counter >= RASTER_LENGTH {
            self.x_counter = 0;
            self.raster_counter += 1;
        }
        return color;
    }
}

impl Memory for Vic {
    fn read(&self, _: u16) -> std::result::Result<u8, ya6502::memory::ReadError> {
        todo!()
    }
    fn write(&mut self, address: u16, value: u8) -> Result<(), WriteError> {
        match address {
            registers::BORDER_COLOR => self.reg_border_color = value,
            registers::BACKGROUND_COLOR_0 => self.reg_background_color = value,
            _ => return Err(WriteError { address, value }),
        }
        Ok(())
    }
}

const LEFT_BORDER_START: usize = 77;
const LEFT_BORDER_WIDTH: usize = 47;
const DISPLAY_WINDOW_START: usize = LEFT_BORDER_START + LEFT_BORDER_WIDTH;
const DISPLAY_WINDOW_WIDTH: usize = 320;
const RIGHT_BORDER_START: usize = DISPLAY_WINDOW_START + DISPLAY_WINDOW_WIDTH;
#[allow(dead_code)]
const RIGHT_BORDER_WIDTH: usize = 48;
#[allow(dead_code)]
const BORDER_END: usize = RIGHT_BORDER_START + RIGHT_BORDER_WIDTH;
#[allow(dead_code)]
const VISIBLE_PIXELS: usize = LEFT_BORDER_WIDTH + DISPLAY_WINDOW_WIDTH + RIGHT_BORDER_WIDTH;
const RASTER_LENGTH: usize = 65 * 8;
#[allow(dead_code)]
const RIGHT_BLANK_WIDTH: usize = RASTER_LENGTH - BORDER_END;

#[allow(dead_code)]
const TOP_BORDER_FIRST_LINE: usize = 20;
#[allow(dead_code)]
const TOP_BORDER_HEIGHT: usize = DISPLAY_WINDOW_FIRST_LINE - TOP_BORDER_FIRST_LINE;
const DISPLAY_WINDOW_FIRST_LINE: usize = 51;
const DISPLAY_WINDOW_HEIGHT: usize = 200;
const BOTTOM_BORDER_FIRST_LINE: usize = DISPLAY_WINDOW_FIRST_LINE + DISPLAY_WINDOW_HEIGHT;
#[allow(dead_code)]
const BOTTOM_BORDER_HEIGHT: usize = TOTAL_HEIGHT - BOTTOM_BORDER_FIRST_LINE;
#[allow(dead_code)]
const TOTAL_HEIGHT: usize = 262;

mod registers {
    pub const BORDER_COLOR: u16 = 0xD020;
    pub const BACKGROUND_COLOR_0: u16 = 0xD021;
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::test_utils::as_single_hex_digit;

    fn visible_raster_line<'a>(vic: &'a mut Vic) -> Vec<u8> {
        for _ in 0..LEFT_BORDER_START {
            vic.tick();
        }
        let result: Vec<u8> = std::iter::from_fn(|| Some(vic.tick()))
            .take(VISIBLE_PIXELS as usize)
            .collect();
        for _ in BORDER_END..RASTER_LENGTH {
            vic.tick();
        }
        return result;
    }

    fn skip_raster_lines(vic: &mut Vic, n: usize) {
        for _ in 0..n * RASTER_LENGTH {
            vic.tick();
        }
    }

    fn encode_video<I: IntoIterator<Item = u8>>(outputs: I) -> String {
        outputs.into_iter().map(as_single_hex_digit).collect()
    }

    #[test]
    fn draws_border() {
        let mut vic = Vic::new();
        vic.write(registers::BORDER_COLOR, 0x00).unwrap();
        assert_eq!(vic.tick(), 0x00);

        vic.write(registers::BORDER_COLOR, 0x01).unwrap();
        assert_eq!(vic.tick(), 0x01);

        vic.write(registers::BORDER_COLOR, 0x0F).unwrap();
        assert_eq!(vic.tick(), 0x0F);
    }

    #[test]
    fn draws_border_raster_lines() {
        let mut vic = Vic::new();
        vic.write(registers::BORDER_COLOR, 0x08).unwrap();
        vic.write(registers::BACKGROUND_COLOR_0, 0x0A).unwrap();
        let border_line = "8".repeat(VISIBLE_PIXELS);
        let border_and_display_line = "8".repeat(LEFT_BORDER_WIDTH)
            + &"A".repeat(DISPLAY_WINDOW_WIDTH)
            + &"8".repeat(RIGHT_BORDER_WIDTH);

        assert_eq!(encode_video(visible_raster_line(&mut vic)), border_line);

        skip_raster_lines(&mut vic, DISPLAY_WINDOW_FIRST_LINE - 1);
        assert_eq!(
            encode_video(visible_raster_line(&mut vic)),
            border_and_display_line
        );
    }
}
