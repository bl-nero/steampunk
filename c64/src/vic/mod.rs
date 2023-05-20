mod tests;

use std::cell::RefCell;
use std::rc::Rc;
use ya6502::memory::Inspect;
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
pub struct Vic<GrMem, ChrMem>
where
    GrMem: Read,
    ChrMem: Read,
{
    graphics_memory: Box<GrMem>,
    color_memory: Rc<RefCell<ChrMem>>,

    // Registers
    reg_control_1: u8,
    reg_control_2: u8,
    reg_interrupt: u8,
    reg_interrupt_mask: u8,
    reg_border_color: Color,
    reg_background_color: Color,

    // Internal state
    //
    /// Counts the raster lines. Note that these are not the same as Y
    /// coordinates in any space; in particular, on NTSC, raster line 0 is
    /// actually near the bottom of the screen. See [`raster_line_to_screen_y`].
    raster_counter: usize,
    /// Raster number that will trigger IRQ (if raster IRQ is enabled).
    irq_raster_line: usize,
    x_counter: usize,
    screen_on: bool,

    /// A buffer for graphics byte to be displayed next.
    graphics_buffer: u8,
    /// A buffer for graphics foreground color to be displayed.
    color_buffer: Color,
    /// A shift register for graphics byte, responsible for generating the
    /// graphics pixel by pixel.
    graphics_shifter: u8,

    /// For now, allow one-time initialization of certain registers to 0.
    reg_initialized: [bool; 0x2F],
}

impl<GrMem, ChrMem> Vic<GrMem, ChrMem>
where
    GrMem: Read,
    ChrMem: Read,
{
    pub fn new(graphics_memory: Box<GrMem>, color_memory: Rc<RefCell<ChrMem>>) -> Self {
        Self {
            graphics_memory,
            color_memory,

            reg_control_1: 0,
            reg_control_2: 0,
            reg_interrupt: flags::INTERRUPT_UNUSED,
            reg_interrupt_mask: flags::INTERRUPT_MASK_UNUSED,
            reg_border_color: 0,
            reg_background_color: 0,

            raster_counter: 0,
            irq_raster_line: 0,
            x_counter: 0,
            screen_on: true,

            graphics_buffer: 0,
            color_buffer: 0,
            graphics_shifter: 0,

            reg_initialized: [false; 0x2F],
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

        // We only sense and latch the `screen_on` flag during raster line 48.
        if self.raster_counter == 48 {
            if self.x_counter == 0 {
                self.screen_on = false;
            }
            self.screen_on |= self.reg_control_1 & flags::CONTROL_1_SCREEN_ON != 0;
        }

        let graphics_color = self.graphics_tick()?;

        let color = match self.raster_counter {
            DISPLAY_WINDOW_FIRST_LINE..=DISPLAY_WINDOW_LAST_LINE => {
                match (
                    self.screen_on,
                    self.reg_control_2 & flags::CONTROL_2_CSEL,
                    self.x_counter,
                ) {
                    (true, flags::CONTROL_2_CSEL, DISPLAY_WINDOW_START..=DISPLAY_WINDOW_END) => {
                        graphics_color
                    }
                    (true, 0, NARROW_DISPLAY_WINDOW_START..=NARROW_DISPLAY_WINDOW_END) => {
                        graphics_color
                    }
                    _ => self.reg_border_color,
                }
            }
            _ => self.reg_border_color,
        };

        if self.raster_counter == self.irq_raster_line
            && self.x_counter == 0
            && self.reg_interrupt_mask & flags::INTERRUPT_RASTER != 0
        {
            self.reg_interrupt |= flags::INTERRUPT_PENDING | flags::INTERRUPT_RASTER;
        }

        let output = VicOutput {
            video_output: VideoOutput {
                x: self.x_counter,
                raster_line: self.raster_counter,
                color: color & !flags::COLOR_UNUSED,
            },
            irq: self.reg_interrupt & flags::INTERRUPT_PENDING != 0,
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
    fn read_bitmap_memory(&mut self) -> Result<u8, ReadError> {
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
    fn read_color_memory(&mut self) -> Result<Color, ReadError> {
        let char_column = (self.x_counter - DISPLAY_WINDOW_START) / 8;
        let char_row = (self.raster_counter - DISPLAY_WINDOW_FIRST_LINE) / 8;
        self.color_memory
            .borrow_mut()
            .read(0xD800 + (char_row * 40 + char_column) as u16)
    }
}

pub struct VicOutput {
    /// Whether VIC reports an IRQ interrupt.
    pub irq: bool,
    pub video_output: VideoOutput,
}

/// The video output of [`Vic::tick`]. Note that the coordinates are raw and
/// include horizontal and vertical blanking areas; it's u to the consumer to
/// crop pixels to the viewport.
pub struct VideoOutput {
    pub color: Color,
    /// Raw X coordinate (including horizontal blanking area).
    pub x: usize,
    /// Raw Y coordinate (including vertical blanking area).
    pub raster_line: usize,
}

pub type TickResult = Result<VicOutput, ReadError>;

impl<GrMem, ChrMem> Inspect for Vic<GrMem, ChrMem>
where
    GrMem: Read,
    ChrMem: Read,
{
    fn inspect(&self, address: u16) -> ReadResult {
        match address {
            registers::CONTROL_1 => Ok(self.reg_control_1 & !flags::CONTROL_1_RASTER_8
                | (self.raster_counter >> 1) as u8 & flags::CONTROL_1_RASTER_8),
            registers::RASTER => Ok(self.raster_counter as u8),
            registers::CONTROL_2 => Ok(self.reg_control_2 | flags::CONTROL_2_UNUSED),
            registers::INTERRUPT => Ok(self.reg_interrupt),
            registers::INTERRUPT_MASK => Ok(self.reg_interrupt_mask),
            registers::BORDER_COLOR => Ok(self.reg_border_color | flags::COLOR_UNUSED),
            registers::BACKGROUND_COLOR_0 => Ok(self.reg_background_color | flags::COLOR_UNUSED),
            _ => Err(ReadError { address }),
        }
    }
}

impl<GrMem, ChrMem> Read for Vic<GrMem, ChrMem>
where
    GrMem: Read,
    ChrMem: Read,
{
    fn read(&mut self, address: u16) -> ReadResult {
        self.inspect(address)
    }
}

impl<GrMem: Read, ChrMem: Read> Write for Vic<GrMem, ChrMem> {
    fn write(&mut self, address: u16, value: u8) -> WriteResult {
        match address {
            registers::CONTROL_1 => {
                if value & !(flags::CONTROL_1_RASTER_8 | flags::CONTROL_1_SCREEN_ON)
                    != 3 | flags::CONTROL_1_RSEL
                {
                    return Err(WriteError { address, value });
                }
                self.reg_control_1 = value & !flags::CONTROL_1_RASTER_8;
                self.irq_raster_line = self.irq_raster_line & 0b1111_1111
                    | ((value & flags::CONTROL_1_RASTER_8) as usize) << 1;
            }
            registers::RASTER => {
                self.irq_raster_line = self.irq_raster_line & 0b1_0000_0000 | value as usize;
            }
            registers::CONTROL_2 => {
                if value & flags::CONTROL_2_MCM != 0 {
                    return Err(WriteError { address, value });
                }
                self.reg_control_2 = value | flags::CONTROL_2_UNUSED;
            }
            registers::INTERRUPT => {
                // TODO: For now, we just ignore acknowledging interrupts that
                // we don't yet support in the first place.
                if value & flags::INTERRUPT_RASTER != 0 {
                    self.reg_interrupt = flags::INTERRUPT_UNUSED;
                }
            }
            registers::INTERRUPT_MASK => {
                // Only raster interrupts are currently supported.
                if value & !flags::INTERRUPT_RASTER != 0 {
                    return Err(WriteError { address, value });
                }
                self.reg_interrupt_mask = value | flags::INTERRUPT_MASK_UNUSED;
            }
            registers::BORDER_COLOR => self.reg_border_color = value | flags::COLOR_UNUSED,
            registers::BACKGROUND_COLOR_0 => {
                self.reg_background_color = value | flags::COLOR_UNUSED
            }

            // We don't support ECM text mode or sprites just yet; for now,
            // ignore all writes.
            registers::BACKGROUND_COLOR_1..=registers::SPRITE_7_COLOR => {}

            _ => {
                if self.reg_initialized[(address - registers::BASE) as usize] {
                    return Err(WriteError { address, value });
                }
                self.reg_initialized[(address - registers::BASE) as usize] = true;
            }
        }
        Ok(())
    }
}

impl<GrMem: Read, ChrMem: Read> Memory for Vic<GrMem, ChrMem> {}

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
    pub const BASE: u16 = 0xD000;
    pub const CONTROL_1: u16 = 0xD011;
    pub const RASTER: u16 = 0xD012;
    pub const CONTROL_2: u16 = 0xD016;
    pub const INTERRUPT: u16 = 0xD019;
    pub const INTERRUPT_MASK: u16 = 0xD01A;
    pub const BORDER_COLOR: u16 = 0xD020;
    pub const BACKGROUND_COLOR_0: u16 = 0xD021;
    pub const BACKGROUND_COLOR_1: u16 = 0xD022;
    pub const SPRITE_7_COLOR: u16 = 0xD02E;
}

#[allow(dead_code)]
mod flags {
    pub const CONTROL_1_YSCROLL: u8 = 0b0000_0111;
    pub const CONTROL_1_RSEL: u8 = 0b0000_1000;
    pub const CONTROL_1_SCREEN_ON: u8 = 0b0001_0000;
    pub const CONTROL_1_BITMAP_MODE: u8 = 0b0010_0000;
    pub const CONTROL_1_EXTENDED_BG: u8 = 0b0100_0000;
    /// 8th bit of the raster line counter in the
    /// [`CONTROL_1`][super::registers::CONTROL_1] register.
    pub const CONTROL_1_RASTER_8: u8 = 0b1000_0000;

    pub const CONTROL_2_XSCROLL: u8 = 0b0000_0111;
    pub const CONTROL_2_CSEL: u8 = 0b0000_1000;
    pub const CONTROL_2_MCM: u8 = 0b0001_0000;
    pub const CONTROL_2_UNUSED: u8 = 0b1100_0000;

    /// Raster interrupt. Valid for [`INTERRUPT`][super::registers::INTERRUPT]
    /// and [`INTERRUPT_MASK`][super::registers::INTERRUPT_MASK]
    /// registers.
    pub const INTERRUPT_RASTER: u8 = 0b0000_0001;
    /// Sprite-background collision detected. Valid for
    /// [`INTERRUPT`][super::registers::INTERRUPT] and
    /// [`INTERRUPT_MASK`][super::registers::INTERRUPT_MASK] registers.
    pub const INTERRUPT_SPRITE_BACKGROUND: u8 = 0b0000_0010;
    /// Sprite-sprite collision detected. Valid for
    /// [`INTERRUPT`][super::registers::INTERRUPT] and
    /// [`INTERRUPT_MASK`][super::registers::INTERRUPT_MASK] registers.
    pub const INTERRUPT_SPRITE_SPRITE: u8 = 0b0000_0100;
    /// Light pen signal arrived. Valid for
    /// [`INTERRUPT`][super::registers::INTERRUPT] and
    /// [`INTERRUPT_MASK`][super::registers::INTERRUPT_MASK] registers.
    pub const INTERRUPT_LIGHT_PEN: u8 = 0b0000_1000;
    /// There is an unacknowledged interrupt in the
    /// [`INTERRUPT`][super::registers::INTERRUPT] register.
    pub const INTERRUPT_PENDING: u8 = 0b1000_0000;

    /// Unused bits of [`INTERRUPT`][super::registers::INTERRUPT] register.
    pub const INTERRUPT_UNUSED: u8 = 0b0111_0000;

    /// Unused bits of
    /// [`INTERRUPT_MASK`][super::registers::INTERRUPT_MASK] register.
    pub const INTERRUPT_MASK_UNUSED: u8 = 0b1111_0000;
    /// Unused bits of color registers.
    pub const COLOR_UNUSED: u8 = 0b1111_0000;
}
