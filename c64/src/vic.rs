use std::cell::RefCell;
use std::rc::Rc;
use ya6502::memory::Read;
use ya6502::memory::ReadError;
use ya6502::memory::ReadResult;
use ya6502::memory::Write;
use ya6502::memory::WriteError;
use ya6502::memory::WriteResult;

pub type Color = u8;

/// VIC-II video chip emulator that outputs a stream of bytes. Each byte encodes
/// a single pixel and has a value from a 0..=15 range.
pub struct Vic<GM: Read, CM: Read> {
    graphics_memory: Box<GM>,
    color_memory: Rc<RefCell<CM>>,

    reg_border_color: Color,
    reg_background_color: Color,

    raster_counter: usize,
    x_counter: usize,
    graphics_column: u16,
    graphics_row: u16,
    character_offset: u16,
    graphics_mask: u8,
}

impl<GM: Read, CM: Read> Vic<GM, CM> {
    pub fn new(graphics_memory: Box<GM>, color_memory: Rc<RefCell<CM>>) -> Self {
        Self {
            graphics_memory,
            color_memory,

            reg_border_color: 0,
            reg_background_color: 0,

            raster_counter: 0,
            x_counter: 0,
            graphics_column: 0,
            graphics_row: 0,
            character_offset: 0,
            graphics_mask: 0b1000_0000,
        }
    }

    /// Emulates a single tick of the pixel clock and returns a pixel color. For
    /// simplicity, we don't distinguish between blanking and visible pixels.
    /// This is different from TIA, since TIA is controlled to much higher
    /// degree by software.
    pub fn tick(&mut self) -> TickResult {
        const DISPLAY_WINDOW_LAST_LINE: usize = BOTTOM_BORDER_FIRST_LINE - 1;
        const DISPLAY_WINDOW_END: usize = RIGHT_BORDER_START - 1;
        let color = match self.raster_counter {
            DISPLAY_WINDOW_FIRST_LINE..=DISPLAY_WINDOW_LAST_LINE => match self.x_counter {
                DISPLAY_WINDOW_START..=DISPLAY_WINDOW_END => self.background_tick()?,
                _ => self.reg_border_color,
            },
            _ => self.reg_border_color,
        };

        let output = VicOutput {
            x: self.x_counter,
            y: self.raster_counter,
            color,
        };

        self.x_counter += 1;
        if self.x_counter >= RASTER_LENGTH {
            self.x_counter = 0;
            self.raster_counter += 1;
            if self.raster_counter >= TOTAL_HEIGHT {
                self.raster_counter = 0;
            }
        }

        return Ok(output);
    }

    fn background_tick(&mut self) -> Result<Color, ReadError> {
        let character_index = self
            .graphics_memory
            .read(0x0400 + self.graphics_row + self.graphics_column)?;
        let character_pixel_row = self
            .graphics_memory
            .read(0x1000 + character_index as u16 * 8 + self.character_offset)?;
        let draws_graphics_pixel = character_pixel_row & self.graphics_mask != 0;
        let color = if draws_graphics_pixel {
            self.color_memory
                .borrow_mut()
                .read(0xD800 + self.graphics_row + self.graphics_column)?
        } else {
            self.reg_background_color
        };

        self.graphics_mask = self.graphics_mask.rotate_right(1);
        if self.graphics_mask & 0b1000_0000 != 0 {
            if self.graphics_column >= 39 {
                self.graphics_column = 0;
                if self.character_offset >= 7 {
                    self.character_offset = 0;
                    self.graphics_row = (self.graphics_row + 40) % (40 * 25);
                } else {
                    self.character_offset += 1;
                }
            } else {
                self.graphics_column += 1;
            }
        }

        return Ok(color);
    }
}

/// The video output of [`Vic::tick`]. Note that the coordinates are raw and
/// include horizontal and vertical blanking areas; it's u to the consumer to
/// crop pixels to the viewport.
pub struct VicOutput {
    pub color: Color,
    /// Raw X coordinate (including horizontal blanking area).
    pub x: usize,
    /// Raw Y coordinate (including vertical blanking area).
    pub y: usize,
}

pub type TickResult = Result<VicOutput, ReadError>;

impl<GM: Read, CM: Read> Read for Vic<GM, CM> {
    fn read(&self, address: u16) -> ReadResult {
        Err(ReadError { address })
    }
}

impl<GM: Read, CM: Read> Write for Vic<GM, CM> {
    fn write(&mut self, address: u16, value: u8) -> WriteResult {
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
    use ya6502::memory::Ram;

    fn vic_for_testing() -> Vic<Ram, Ram> {
        Vic::new(Box::new(Ram::new(16)), Rc::new(RefCell::new(Ram::new(16))))
    }

    /// Grabs a single visible raster line, discarding the blanking area. Note
    /// that the visible area is established by convention, as we don't have to
    /// pay attention to details too much here.
    fn visible_raster_line<GM: Read, CM: Read>(vic: &mut Vic<GM, CM>) -> Vec<Color> {
        // Initialize to an illegal color to make sure that all pixels are
        // covered.
        let mut result = vec![0xFF; VISIBLE_PIXELS];
        for _ in 0..RASTER_LENGTH {
            let vic_output = vic.tick().unwrap();
            if (LEFT_BORDER_START..BORDER_END).contains(&vic_output.x) {
                result[vic_output.x - LEFT_BORDER_START] = vic_output.color;
            }
        }
        return result;
    }

    /// Skips a given number of full raster lines and discards results.
    fn skip_raster_lines<GM: Read, CM: Read>(vic: &mut Vic<GM, CM>, n: usize) {
        for _ in 0..n * RASTER_LENGTH {
            vic.tick().unwrap();
        }
    }

    /// Retrieves a full frame, including blank areas, and returns a rectangle
    /// at given coordinates relative to the upper left corner of the graphics
    /// display window.
    fn grab_frame<GM: Read, FM: Read>(
        vic: &mut Vic<GM, FM>,
        left: isize,
        top: isize,
        width: usize,
        height: usize,
    ) -> Vec<Vec<Color>> {
        let top = (DISPLAY_WINDOW_FIRST_LINE as isize + top) as usize;
        let left = (DISPLAY_WINDOW_START as isize + left) as usize;
        let bottom = top + width;
        let right = left + height;
        let mut result: Vec<Vec<Color>> =
            std::iter::repeat(vec![0xFF; width]).take(height).collect();
        for _ in 0..RASTER_LENGTH * TOTAL_HEIGHT {
            let vic_output = vic.tick().unwrap();
            if (left..right).contains(&vic_output.x) && (top..bottom).contains(&vic_output.y) {
                result[vic_output.y - top][vic_output.x - left] = vic_output.color;
            }
        }
        return result;
    }

    /// Encodes a sequence of colors into an easy to read string where each
    /// color from a 4-bit palette is denoted by a single hexadecimal character.
    /// The color 0 (black) is denoted as '.' for better readability.
    fn encode_video<I: IntoIterator<Item = Color>>(outputs: I) -> String {
        outputs
            .into_iter()
            .map(|color| match color {
                0 => '.',
                c => as_single_hex_digit(c),
            })
            .collect()
    }

    fn encode_video_lines<Iter, IterIter>(outputs: IterIter) -> Vec<String>
    where
        Iter: IntoIterator<Item = Color>,
        IterIter: IntoIterator<Item = Iter>,
    {
        outputs.into_iter().map(encode_video).collect()
    }

    #[test]
    fn draws_border() {
        let mut vic = vic_for_testing();
        vic.write(registers::BORDER_COLOR, 0x00).unwrap();
        assert_eq!(vic.tick().unwrap().color, 0x00);

        vic.write(registers::BORDER_COLOR, 0x01).unwrap();
        assert_eq!(vic.tick().unwrap().color, 0x01);

        vic.write(registers::BORDER_COLOR, 0x0F).unwrap();
        assert_eq!(vic.tick().unwrap().color, 0x0F);
    }

    #[test]
    fn draws_border_raster_lines() {
        let mut vic = vic_for_testing();
        vic.write(registers::BORDER_COLOR, 0x08).unwrap();
        vic.write(registers::BACKGROUND_COLOR_0, 0x0A).unwrap();
        let border_line = "8".repeat(VISIBLE_PIXELS);
        let border_and_display_line = "8".repeat(LEFT_BORDER_WIDTH)
            + &"A".repeat(DISPLAY_WINDOW_WIDTH)
            + &"8".repeat(RIGHT_BORDER_WIDTH);

        // Expect a border line.
        assert_eq!(encode_video(visible_raster_line(&mut vic)), border_line);

        // Expect the first line of the display window.
        skip_raster_lines(&mut vic, DISPLAY_WINDOW_FIRST_LINE - 1);
        assert_eq!(
            encode_video(visible_raster_line(&mut vic)),
            border_and_display_line
        );

        // Last line of the display window and the first one of the bottom
        // border.
        skip_raster_lines(&mut vic, DISPLAY_WINDOW_HEIGHT - 2);
        assert_eq!(
            encode_video(visible_raster_line(&mut vic)),
            border_and_display_line
        );
        assert_eq!(encode_video(visible_raster_line(&mut vic)), border_line);

        // First line of the next frame's display window.
        skip_raster_lines(
            &mut vic,
            BOTTOM_BORDER_HEIGHT - 1 + DISPLAY_WINDOW_FIRST_LINE,
        );
        assert_eq!(
            encode_video(visible_raster_line(&mut vic)),
            border_and_display_line
        );
    }

    #[test]
    fn draws_characters() {
        let mut vic = vic_for_testing();
        vic.write(registers::BORDER_COLOR, 0x01).unwrap();
        vic.write(registers::BACKGROUND_COLOR_0, 0x00).unwrap();

        vic.graphics_memory.bytes[0x1008..0x1028].copy_from_slice(&[
            0b00000000, 0b01111111, 0b01000001, 0b01000001, 0b01000001, 0b01000001, 0b01000001,
            0b01111111, 0b00000000, 0b01000001, 0b00100010, 0b00010100, 0b00001000, 0b00010100,
            0b00100010, 0b01000001, 0b00000000, 0b00011100, 0b00100010, 0b01000001, 0b01000001,
            0b01000001, 0b00100010, 0b00011100, 0b00000000, 0b00001000, 0b00010100, 0b00010100,
            0b00100010, 0b00100010, 0b01000001, 0b01111111,
        ]);
        vic.graphics_memory.bytes[0x0400] = 0x01;
        vic.graphics_memory.bytes[0x0401] = 0x02;
        vic.graphics_memory.bytes[0x0428] = 0x03;
        vic.graphics_memory.bytes[0x0429] = 0x04;
        {
            let mut color_memory = vic.color_memory.borrow_mut();
            color_memory.bytes[0xD800] = 0x0A;
            color_memory.bytes[0xD801] = 0x0B;
            color_memory.bytes[0xD828] = 0x0C;
            color_memory.bytes[0xD829] = 0x0D;
        }

        itertools::assert_equal(
            encode_video_lines(grab_frame(&mut vic, -1, -1, 17, 17)).iter(),
            &[
                "11111111111111111",
                "1................",
                "1.AAAAAAA.B.....B",
                "1.A.....A..B...B.",
                "1.A.....A...B.B..",
                "1.A.....A....B...",
                "1.A.....A...B.B..",
                "1.A.....A..B...B.",
                "1.AAAAAAA.B.....B",
                "1................",
                "1...CCC......D...",
                "1..C...C....D.D..",
                "1.C.....C...D.D..",
                "1.C.....C..D...D.",
                "1.C.....C..D...D.",
                "1..C...C..D.....D",
                "1...CCC...DDDDDDD",
            ],
        );

        vic.graphics_memory.bytes[0x0400] = 0x04;
        vic.graphics_memory.bytes[0x0401] = 0x03;
        vic.graphics_memory.bytes[0x0428] = 0x02;
        vic.graphics_memory.bytes[0x0429] = 0x01;

        itertools::assert_equal(
            encode_video_lines(grab_frame(&mut vic, -1, -1, 17, 17)).iter(),
            &[
                "11111111111111111",
                "1................",
                "1....A......BBB..",
                "1...A.A....B...B.",
                "1...A.A...B.....B",
                "1..A...A..B.....B",
                "1..A...A..B.....B",
                "1.A.....A..B...B.",
                "1.AAAAAAA...BBB..",
                "1................",
                "1.C.....C.DDDDDDD",
                "1..C...C..D.....D",
                "1...C.C...D.....D",
                "1....C....D.....D",
                "1...C.C...D.....D",
                "1..C...C..D.....D",
                "1.C.....C.DDDDDDD",
            ],
        );
    }
}
