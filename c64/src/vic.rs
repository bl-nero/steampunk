use std::cell::RefCell;
use std::rc::Rc;
use ya6502::memory::Memory;
use ya6502::memory::Read;
use ya6502::memory::ReadError;
use ya6502::memory::ReadResult;
use ya6502::memory::Write;
use ya6502::memory::WriteError;
use ya6502::memory::WriteResult;

pub type Color = u8;

/// VIC-II video chip emulator that outputs a stream of bytes. Each byte encodes
/// a single pixel and has a value from a 0..=15 range.
#[derive(Debug)]
pub struct Vic<GM: Read, CM: Read> {
    graphics_memory: Box<GM>,
    color_memory: Rc<RefCell<CM>>,

    // Registers
    reg_control_2: u8,
    reg_border_color: Color,
    reg_background_color: Color,

    // Internal state
    //
    /// Counts the raster lines. Note that these are not the same as Y
    /// coordinates in any space; in particular, on NTSC, raster line 0 is
    /// actually near the bottom of the screen. See [`raster_line_to_screen_y`].
    raster_counter: usize,
    x_counter: usize,

    /// A buffer for graphics byte to be displayed next.
    graphics_buffer: u8,
    /// A buffer for graphics foreground color to be displayed.
    color_buffer: Color,
    /// A shift register for graphics byte, responsible for generating the
    /// graphics pixel by pixel.
    graphics_shifter: u8,
}

impl<GM: Read, CM: Read> Vic<GM, CM> {
    pub fn new(graphics_memory: Box<GM>, color_memory: Rc<RefCell<CM>>) -> Self {
        Self {
            graphics_memory,
            color_memory,

            reg_control_2: 0,
            reg_border_color: 0,
            reg_background_color: 0,

            raster_counter: 0,
            x_counter: 0,

            graphics_buffer: 0,
            color_buffer: 0,
            graphics_shifter: 0,
        }
    }

    /// Emulates a single tick of the pixel clock and returns a pixel color. For
    /// simplicity, we don't distinguish between blanking and visible pixels.
    /// This is different from TIA, since TIA is controlled to much higher
    /// degree by software.
    pub fn tick(&mut self) -> TickResult {
        const DISPLAY_WINDOW_LAST_LINE: usize = BOTTOM_BORDER_FIRST_LINE - 1;
        const DISPLAY_WINDOW_END: usize = RIGHT_BORDER_START - 1;
        const NARROW_DISPLAY_WINDOW_START: usize = DISPLAY_WINDOW_START + 8;
        const NARROW_DISPLAY_WINDOW_END: usize = DISPLAY_WINDOW_END - 8;
        let graphics_color = self.graphics_tick()?;

        let color = match self.raster_counter {
            DISPLAY_WINDOW_FIRST_LINE..=DISPLAY_WINDOW_LAST_LINE => {
                match (self.reg_control_2 & flags::CONTROL_2_CSEL, self.x_counter) {
                    (flags::CONTROL_2_CSEL, DISPLAY_WINDOW_START..=DISPLAY_WINDOW_END) => {
                        graphics_color
                    }
                    (0, NARROW_DISPLAY_WINDOW_START..=NARROW_DISPLAY_WINDOW_END) => graphics_color,
                    _ => self.reg_border_color,
                }
            }
            _ => self.reg_border_color,
        };

        let output = VicOutput {
            x: self.x_counter,
            raster_line: self.raster_counter,
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

    /// Computes the color currently produced by the character graphics layer.
    fn graphics_tick(&mut self) -> Result<Color, ReadError> {
        const DISPLAY_WINDOW_LAST_LINE: usize = BOTTOM_BORDER_FIRST_LINE - 1;
        const DISPLAY_WINDOW_END: usize = RIGHT_BORDER_START - 1;

        if !(DISPLAY_WINDOW_FIRST_LINE..=DISPLAY_WINDOW_LAST_LINE).contains(&self.raster_counter) {
            return Ok(self.reg_background_color);
        }

        let x_inside_display_window =
            (DISPLAY_WINDOW_START..=DISPLAY_WINDOW_END).contains(&self.x_counter);

        if x_inside_display_window {
            let subcolumn = (self.x_counter - DISPLAY_WINDOW_START) % 8;
            // Note: Using the XSCROLL value for comparison enables horizontal
            // scrolling by up to 7 pixels.
            if subcolumn == (self.reg_control_2 & flags::CONTROL_2_XSCROLL) as usize {
                self.graphics_shifter = self.graphics_buffer;
                // TODO: Move the screen and color memory access to a separate
                // procedure, to be executed during bad lines.
                self.color_buffer = self.read_color_memory()?;
            }
        }

        if (DISPLAY_WINDOW_START - 1..=DISPLAY_WINDOW_END - 1).contains(&self.x_counter)
            && self.x_counter % 8 == (DISPLAY_WINDOW_START - 1) % 8
        {
            self.graphics_buffer = self.read_bitmap_memory()?;
        }
        let draws_graphics_pixel = self.graphics_shifter & (1 << 7) != 0;
        self.graphics_shifter <<= 1;

        if !x_inside_display_window {
            return Ok(self.reg_background_color);
        }

        let color = if draws_graphics_pixel {
            self.color_buffer
        } else {
            self.reg_background_color
        };

        Ok(color)
    }

    /// Reads from bitmap memory a byte that corrensponds to the _next_
    /// character cell.
    fn read_bitmap_memory(&self) -> Result<u8, ReadError> {
        let char_column = (self.x_counter + 1 - DISPLAY_WINDOW_START) / 8;
        let char_row = (self.raster_counter - DISPLAY_WINDOW_FIRST_LINE) / 8;
        let char_offset = (self.raster_counter - DISPLAY_WINDOW_FIRST_LINE) % 8;
        let character_index = self
            .graphics_memory
            .read(0x0400 + (char_row * 40 + char_column) as u16)?;
        return self
            .graphics_memory
            .read(0x1000 + character_index as u16 * 8 + char_offset as u16);
    }

    /// Reads from color memory a color that corrensponds to the _current_
    /// character cell.
    fn read_color_memory(&self) -> Result<Color, ReadError> {
        let char_column = (self.x_counter - DISPLAY_WINDOW_START) / 8;
        let char_row = (self.raster_counter - DISPLAY_WINDOW_FIRST_LINE) / 8;
        self.color_memory
            .borrow()
            .read(0xD800 + (char_row * 40 + char_column) as u16)
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
    pub raster_line: usize,
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
            registers::CONTROL_2 => {
                if value & flags::CONTROL_2_MCM != 0 {
                    return Err(WriteError { address, value });
                }
                self.reg_control_2 = value;
            }
            registers::BORDER_COLOR => self.reg_border_color = value,
            registers::BACKGROUND_COLOR_0 => self.reg_background_color = value,
            _ => return Err(WriteError { address, value }),
        }
        Ok(())
    }
}

impl<GM: Read, CM: Read> Memory for Vic<GM, CM> {}

/// Converts raster line number to Y position on the rendered screen.
pub fn raster_line_to_screen_y(index: usize) -> usize {
    (index + TOTAL_HEIGHT - TOP_BORDER_FIRST_LINE) % TOTAL_HEIGHT
}

/// Converts Y position on the rendered screen to raster line number.
#[cfg(test)]
pub fn screen_y_to_raster_line(screen_y: usize) -> usize {
    (screen_y + TOP_BORDER_FIRST_LINE) % TOTAL_HEIGHT
}

pub const LEFT_BORDER_START: usize = 77;
pub const LEFT_BORDER_WIDTH: usize = 47;
pub const DISPLAY_WINDOW_START: usize = LEFT_BORDER_START + LEFT_BORDER_WIDTH;
pub const DISPLAY_WINDOW_WIDTH: usize = 320;
pub const RIGHT_BORDER_START: usize = DISPLAY_WINDOW_START + DISPLAY_WINDOW_WIDTH;
pub const RIGHT_BORDER_WIDTH: usize = 48;
pub const BORDER_END: usize = RIGHT_BORDER_START + RIGHT_BORDER_WIDTH;
pub const VISIBLE_PIXELS: usize = LEFT_BORDER_WIDTH + DISPLAY_WINDOW_WIDTH + RIGHT_BORDER_WIDTH;
pub const RASTER_LENGTH: usize = 65 * 8;
#[allow(dead_code)]
pub const RIGHT_BLANK_WIDTH: usize = RASTER_LENGTH - BORDER_END;

pub const TOP_BORDER_FIRST_LINE: usize = 41;
pub const TOP_BORDER_HEIGHT: usize = DISPLAY_WINDOW_FIRST_LINE - TOP_BORDER_FIRST_LINE;
pub const DISPLAY_WINDOW_FIRST_LINE: usize = 51;
pub const DISPLAY_WINDOW_HEIGHT: usize = 200;
pub const BOTTOM_BORDER_FIRST_LINE: usize = DISPLAY_WINDOW_FIRST_LINE + DISPLAY_WINDOW_HEIGHT;
pub const BLANK_AREA_FIRST_LINE: usize = 13;
#[allow(dead_code)]
pub const BLANK_AREA_HEIGHT: usize = TOP_BORDER_FIRST_LINE - BLANK_AREA_FIRST_LINE;
// This strange formula stems from the fact that the blank area first line
// actually comes after the raster line counter rolls back to 0. That's why we
// add TOTAL_HEIGHT.
pub const BOTTOM_BORDER_HEIGHT: usize =
    BLANK_AREA_FIRST_LINE + TOTAL_HEIGHT - BOTTOM_BORDER_FIRST_LINE;
pub const VISIBLE_LINES: usize = TOP_BORDER_HEIGHT + DISPLAY_WINDOW_HEIGHT + BOTTOM_BORDER_HEIGHT;
pub const TOTAL_HEIGHT: usize = 262; // Including vertical blank

mod registers {
    pub const CONTROL_2: u16 = 0xD016;
    pub const BORDER_COLOR: u16 = 0xD020;
    pub const BACKGROUND_COLOR_0: u16 = 0xD021;
}

mod flags {
    pub const CONTROL_2_XSCROLL: u8 = 0b0000_0111;
    pub const CONTROL_2_CSEL: u8 = 0b0000_1000;
    pub const CONTROL_2_MCM: u8 = 0b0001_0000;
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::test_utils::as_single_hex_digit;
    use ya6502::memory::Ram;

    /// Creates a VIC backed by a simple RAM architecture and runs enough raster
    /// lines to end up at the beginning of the first visible border line.
    fn vic_for_testing() -> Vic<Ram, Ram> {
        let mut vic = Vic::new(Box::new(Ram::new(16)), Rc::new(RefCell::new(Ram::new(16))));
        for _ in 0..RASTER_LENGTH * TOP_BORDER_FIRST_LINE {
            vic.tick().unwrap();
        }
        return vic;
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

    /// Grabs a raster line, and returns a range of pixels with given
    /// coordinates relative to the left edge of the graphics display window.
    fn grab_raster_line<GM: Read, CM: Read>(
        vic: &mut Vic<GM, CM>,
        left: isize,
        width: usize,
    ) -> Vec<Color> {
        let left = (DISPLAY_WINDOW_START as isize + left) as usize;
        let right = left + width;
        // Initialize to an illegal color to make sure that all pixels are
        // covered.
        let mut result = vec![0xFF; width];
        for _ in 0..RASTER_LENGTH {
            let vic_output = vic.tick().unwrap();
            if (left..right).contains(&vic_output.x) {
                result[vic_output.x - left] = vic_output.color;
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
        // We convert the raster line number to screen Y in order to create a
        // continuous range against which a screen Y coordinate can be tested.
        let top = raster_line_to_screen_y((DISPLAY_WINDOW_FIRST_LINE as isize + top) as usize);
        let left = (DISPLAY_WINDOW_START as isize + left) as usize;
        let bottom = top + height;
        let right = left + width;
        let mut result: Vec<Vec<Color>> =
            std::iter::repeat(vec![0xFF; width]).take(height).collect();
        for _ in 0..RASTER_LENGTH * TOTAL_HEIGHT {
            let vic_output = vic.tick().unwrap();
            let (x, y) = (
                vic_output.x,
                raster_line_to_screen_y(vic_output.raster_line),
            );
            if (left..right).contains(&x) && (top..bottom).contains(&y) {
                result[y - top][x - left] = vic_output.color;
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
        vic.write(registers::CONTROL_2, flags::CONTROL_2_CSEL)
            .unwrap();
        let border_line = "8".repeat(VISIBLE_PIXELS);
        let border_and_display_line = "8".repeat(LEFT_BORDER_WIDTH)
            + &"A".repeat(DISPLAY_WINDOW_WIDTH)
            + &"8".repeat(RIGHT_BORDER_WIDTH);

        // Expect the first line of top border.
        assert_eq!(encode_video(visible_raster_line(&mut vic)), border_line);
        // Expect the last line of top border.
        skip_raster_lines(&mut vic, TOP_BORDER_HEIGHT - 2);
        assert_eq!(encode_video(visible_raster_line(&mut vic)), border_line);

        // Expect the first line of the display window.
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

        // Last line of next frame's top border and first line of its display
        // window.
        skip_raster_lines(
            &mut vic,
            BOTTOM_BORDER_HEIGHT + BLANK_AREA_HEIGHT + TOP_BORDER_HEIGHT - 2,
        );
        assert_eq!(encode_video(visible_raster_line(&mut vic)), border_line);
        assert_eq!(
            encode_video(visible_raster_line(&mut vic)),
            border_and_display_line
        );
    }

    #[test]
    fn draws_border_38_column_mode() {
        let mut vic = vic_for_testing();
        vic.write(registers::BORDER_COLOR, 0x05).unwrap();
        vic.write(registers::BACKGROUND_COLOR_0, 0x0C).unwrap();
        vic.write(registers::CONTROL_2, 0).unwrap();
        let narrow_display_line = "5".repeat(LEFT_BORDER_WIDTH + 8)
            + &"C".repeat(DISPLAY_WINDOW_WIDTH - 16)
            + &"5".repeat(RIGHT_BORDER_WIDTH + 8);

        skip_raster_lines(&mut vic, TOP_BORDER_HEIGHT);
        assert_eq!(
            encode_video(visible_raster_line(&mut vic)),
            narrow_display_line
        );
    }

    #[test]
    fn draws_characters() {
        let mut vic = vic_for_testing();
        vic.write(registers::BORDER_COLOR, 0x01).unwrap();
        vic.write(registers::BACKGROUND_COLOR_0, 0x00).unwrap();
        vic.write(registers::CONTROL_2, flags::CONTROL_2_CSEL)
            .unwrap();

        // Set up characters
        vic.graphics_memory.bytes[0x1008..0x1028].copy_from_slice(&[
            0b11111111, 0b10000001, 0b10000001, 0b10000001, 0b10000001, 0b10000001, 0b10000001,
            0b11111111, 0b10000001, 0b01000010, 0b00100100, 0b00011000, 0b00011000, 0b00100100,
            0b01000010, 0b10000001, 0b00111100, 0b01000010, 0b10000001, 0b10000001, 0b10000001,
            0b10000001, 0b01000010, 0b00111100, 0b00011000, 0b00011000, 0b00100100, 0b00100100,
            0b01000010, 0b01000010, 0b10000001, 0b11111111,
        ]);
        // Set up screen
        vic.graphics_memory.bytes[0x0400] = 0x01;
        vic.graphics_memory.bytes[0x0401] = 0x02;
        vic.graphics_memory.bytes[0x0428] = 0x03;
        vic.graphics_memory.bytes[0x0429] = 0x04;
        // Set up colors
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
                "1AAAAAAAAB......B",
                "1A......A.B....B.",
                "1A......A..B..B..",
                "1A......A...BB...",
                "1A......A...BB...",
                "1A......A..B..B..",
                "1A......A.B....B.",
                "1AAAAAAAAB......B",
                "1..CCCC.....DD...",
                "1.C....C....DD...",
                "1C......C..D..D..",
                "1C......C..D..D..",
                "1C......C.D....D.",
                "1C......C.D....D.",
                "1.C....C.D......D",
                "1..CCCC..DDDDDDDD",
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
                "1...AA.....BBBB..",
                "1...AA....B....B.",
                "1..A..A..B......B",
                "1..A..A..B......B",
                "1.A....A.B......B",
                "1.A....A.B......B",
                "1A......A.B....B.",
                "1AAAAAAAA..BBBB..",
                "1C......CDDDDDDDD",
                "1.C....C.D......D",
                "1..C..C..D......D",
                "1...CC...D......D",
                "1...CC...D......D",
                "1..C..C..D......D",
                "1.C....C.D......D",
                "1C......CDDDDDDDD",
            ],
        );
    }

    #[test]
    fn horizontal_scrolling() {
        let mut vic = vic_for_testing();
        vic.write(registers::BORDER_COLOR, 0x01).unwrap();
        vic.write(registers::BACKGROUND_COLOR_0, 0x00).unwrap();
        let grab_line_left =
            move |vic: &mut Vic<Ram, Ram>| encode_video(grab_raster_line(vic, -1, 17));

        // Character 1: a simple bit pattern
        vic.graphics_memory.bytes[0x1008..0x1010].copy_from_slice(&[0b1010_0101; 8]);
        vic.graphics_memory.bytes[0x0400] = 0x01;
        {
            vic.color_memory.borrow_mut().bytes[0xD800] = 0x0A;
        }

        // Skip top border
        skip_raster_lines(&mut vic, TOP_BORDER_HEIGHT);

        vic.write(0xD016, flags::CONTROL_2_CSEL).unwrap();
        assert_eq!(grab_line_left(&mut vic), "1A.A..A.A........");
        vic.write(0xD016, flags::CONTROL_2_CSEL | 1).unwrap();
        assert_eq!(grab_line_left(&mut vic), "1.A.A..A.A.......");
        vic.write(0xD016, flags::CONTROL_2_CSEL | 2).unwrap();
        assert_eq!(grab_line_left(&mut vic), "1..A.A..A.A......");
        vic.write(0xD016, flags::CONTROL_2_CSEL | 7).unwrap();
        assert_eq!(grab_line_left(&mut vic), "1.......A.A..A.A.");
    }
}
