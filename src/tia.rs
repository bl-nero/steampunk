use crate::memory::{Memory, ReadError, ReadResult, WriteError, WriteResult};
use enum_map::{enum_map, Enum, EnumMap};

#[derive(Debug, Enum, Copy, Clone)]
pub enum Port {
    Input4,
    Input5,
}

/// TIA is responsible for generating the video signal, sound (not yet
/// implemented) and for synchronizing CPU with the screen's electron beam.
#[derive(Debug)]
pub struct Tia {
    // *** REGISTERS ***
    /// If bit 1 (`flags::VSYNC_ON`) is set, TIA emits a VSYNC signal.
    reg_vsync: u8,
    /// If bit 1 (`flags::VBLANK_ON`) is set, TIA doesn't emit pixels. Bit 6
    /// (`flags::VBLANK_INPUT_LATCH`) enables latches on input ports 4 and 5.
    reg_vblank: u8,
    /// Color and luminance of player 0. See
    /// [`VideoOutput::pixel`](struct.VideoOutput.html#structfield.pixel) for details.
    reg_colup0: u8,
    /// Color and luminance of player 1. See
    /// [`VideoOutput::pixel`](struct.VideoOutput.html#structfield.pixel) for details.
    reg_colup1: u8,
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
    /// Input port registers.
    reg_inpt: EnumMap<Port, u8>,

    /// Each frame has 228 cycles, including 160 cycles that actually emit
    /// pixels.
    column_counter: u32,
    /// Indicates whether a horizontal blank signal is being generated.
    hblank_on: bool,
    /// Indicates whether a horizontal sync signal is being generated.
    hsync_on: bool,
    /// Holds CPU ticks until we reach the end of a scanline.
    wait_for_sync: bool,
    /// Temporarily stores a playfield register bit.
    playfield_bit_latch_1: bool,
    /// Temporarily stores a playfield register bit.
    playfield_bit_latch_2: bool,
    /// Latches the HMOVE signal until end of the scanline.
    hmove_latch: bool,
    /// Counts from 7 down to -8 while additional clock ticks are sent to the
    /// player graphics objects.
    hmove_counter: i8,

    player0: PlayerGraphics,
    player1: PlayerGraphics,
    // missile_0_pos: u32,
    // missile_1_pos: u32,
    // ball_pos: u32,

    // "Raw" values on the input port pins. They don't necessarily directly
    // reflect `reg_inpt`, since they are not latched.
    input_ports: EnumMap<Port, bool>,

    // A temporary hack to allow one-time initialization before complaining each
    // time a register is written to.
    initialized_registers: [bool; 0x100],
}

impl Tia {
    pub fn new() -> Tia {
        Tia {
            reg_vsync: 0,
            reg_vblank: 0,
            reg_colup0: 0,
            reg_colup1: 0,
            reg_colupf: 0,
            reg_colubk: 0,
            reg_ctrlpf: 0,
            reg_pf0: 0,
            reg_pf1: 0,
            reg_pf2: 0,
            reg_inpt: enum_map! { _ => flags::INPUT_HIGH },

            column_counter: 0,
            hsync_on: false,
            hblank_on: false,
            wait_for_sync: false,
            playfield_bit_latch_1: false,
            playfield_bit_latch_2: false,
            hmove_latch: false,
            hmove_counter: 0,
            player0: PlayerGraphics::new(),
            player1: PlayerGraphics::new(),
            // missile_0_pos: 0,
            // missile_1_pos: 0,
            // ball_pos: 0,
            input_ports: enum_map! { _ => true },
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
            HBLANK_WIDTH => {
                if !self.hmove_latch {
                    self.hblank_on = false
                }
            }
            HBLANK_EXTENDED_WIDTH => {
                if self.hmove_latch {
                    self.hblank_on = false
                }
            }
            LAST_COLUMN => self.hmove_latch = false,
            _ => {}
        }

        let vsync_on = self.reg_vsync & flags::VSYNC_ON != 0;
        let vblank_on = self.reg_vblank & flags::VBLANK_ON != 0;
        let playfield_color = self.playfield_tick();
        if self.hmove_latch && self.hmove_counter > -8 && self.column_counter % 4 == 0 {
            self.player0.hmove_tick(self.hmove_counter);
            self.player1.hmove_tick(self.hmove_counter);
            self.hmove_counter -= 1;
        }

        let pixel = if self.hblank_on {
            None
        } else {
            // Even if these bits can ultimately remain unused, we still need to
            // perform a tick if we are outside the horizontal blank.
            let p0_bit = self.player0.tick();
            let p1_bit = self.player1.tick();
            if vblank_on {
                None
            } else {
                Some(if p0_bit {
                    self.reg_colup0
                } else if p1_bit {
                    self.reg_colup1
                } else {
                    playfield_color
                })
            }
        };

        let output = TiaOutput {
            video: VideoOutput {
                hsync: self.hsync_on,
                vsync: vsync_on,
                pixel,
            },
            riot_tick: self.column_counter % 3 == 0,
            cpu_tick: !self.wait_for_sync && self.column_counter % 3 == 0,
        };

        self.column_counter = (self.column_counter + 1) % TOTAL_WIDTH;
        return output;
    }

    fn playfield_tick(&mut self) -> u8 {
        if self.column_counter % 4 == 0 {
            self.playfield_bit_latch_2 = self.playfield_bit_latch_1;
            self.playfield_bit_latch_1 = self.playfield_bit_at(self.playfiled_bit_index_to_latch());
        }
        return if self.playfield_bit_latch_2 {
            self.reg_colupf
        } else {
            self.reg_colubk
        };
    }

    fn playfield_bit_at(&self, playfield_bit_index: i32) -> bool {
        let mask = match playfield_bit_index {
            0..=3 => 0b0001_0000 << playfield_bit_index,
            4..=11 => 0b1000_0000 >> (playfield_bit_index - 4),
            12..=19 => 0b0000_0001 << (playfield_bit_index - 12),
            _ => 0,
        };
        let playfield_register_value = match playfield_bit_index {
            0..=3 => self.reg_pf0,
            4..=11 => self.reg_pf1,
            12..=19 => self.reg_pf2,
            _ => 0,
        };

        return mask & playfield_register_value != 0;
    }

    /// Returns a playfield pixel bit index from a [0, 20) range that should be
    /// latched in the playfield bit latch during current cycle.  The resulting
    /// value can be directly used to access the playfield registers, because it
    /// takes into consideration playfield reflection.
    fn playfiled_bit_index_to_latch(&self) -> i32 {
        // Playfield has 4 times lower resolution than other stuff.
        let hsync_counter = self.column_counter as i32 / 4;
        // We start latching one hsync clock cycle before the actual pixels
        // start.
        let playfield_start = HBLANK_WIDTH as i32 / 4 - 1;
        let x = hsync_counter - playfield_start;
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

    pub fn set_port(&mut self, port: Port, value: bool) {
        self.input_ports[port] = value;
        self.update_port_register(port);
    }

    fn update_port_register(&mut self, port: Port) {
        let port_value = self.input_ports[port];
        let reg_previous = self.reg_inpt[port] != 0;
        let latch = self.reg_vblank & flags::VBLANK_INPUT_LATCH != 0;

        let reg_next = port_value && (!latch || reg_previous);
        self.reg_inpt[port] = if reg_next { flags::INPUT_HIGH } else { 0 };
    }
}

impl Memory for Tia {
    fn read(&self, address: u16) -> ReadResult {
        match address {
            // TODO: mirroring
            registers::INPT4 => Ok(self.reg_inpt[Port::Input4]),
            registers::INPT5 => Ok(self.reg_inpt[Port::Input5]),
            _ => Err(ReadError { address }),
        }
    }

    fn write(&mut self, address: u16, value: u8) -> WriteResult {
        match address & 0b0011_1111 {
            registers::VSYNC => self.reg_vsync = value,
            registers::VBLANK => {
                self.reg_vblank = value;
                self.update_port_register(Port::Input4);
                self.update_port_register(Port::Input5);
            }
            registers::WSYNC => self.wait_for_sync = true,
            registers::RSYNC => self.column_counter = TOTAL_WIDTH - 3,
            registers::COLUP0 => self.reg_colup0 = value,
            registers::COLUP1 => self.reg_colup1 = value,
            registers::COLUPF => self.reg_colupf = value,
            registers::COLUBK => self.reg_colubk = value,
            registers::CTRLPF => self.reg_ctrlpf = value,
            registers::PF0 => self.reg_pf0 = value,
            registers::PF1 => self.reg_pf1 = value,
            registers::PF2 => self.reg_pf2 = value,
            registers::RESP0 => self.player0.reset_player_position(),
            registers::RESP1 => self.player1.reset_player_position(),

            // Audio. Skip that thing for now, since it's complex and not
            // essential.
            registers::AUDC0
            | registers::AUDC1
            | registers::AUDV0
            | registers::AUDV1
            | registers::AUDF0
            | registers::AUDF1 => {}

            registers::GRP0 => self.player0.bitmap = value,
            registers::GRP1 => self.player1.bitmap = value,
            registers::HMP0 => self.player0.hmove_offset = (value as i8) >> 4,
            registers::HMP1 => self.player1.hmove_offset = (value as i8) >> 4,
            // Note: there is an additional delay here, but it requires emulating the Hφ1 signal.
            registers::HMOVE => {
                self.hmove_latch = true;
                self.hmove_counter = 7;
            }

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

/// Represents player graphics state: the pixel counter and bitmap. Also handles
/// RESPx register strobing.
#[derive(Debug)]
struct PlayerGraphics {
    counter: u32,
    bitmap: u8,
    /// Current bitmap pixel mask.
    mask: u8,
    /// Counts down until position reset happens to emulate TIA latching delays.
    reset_countdown: i32,
    hmove_offset: i8,
}

impl PlayerGraphics {
    fn new() -> Self {
        PlayerGraphics {
            counter: 0,
            bitmap: 0b0000_0000,
            mask: 0b0000_0000,
            reset_countdown: 0,
            hmove_offset: 0,
        }
    }

    /// Performs a clock tick and returns `true` if a player pixel should be
    /// drawn, or `false` otherwise.
    fn tick(&mut self) -> bool {
        let result = self.bitmap & self.mask != 0;
        self.mask >>= 1;

        self.counter = (self.counter + 1) % 160;
        if self.reset_countdown > 0 {
            self.reset_countdown -= 1;
            if self.reset_countdown == 0 {
                self.counter = 0;
            }
        }
        if self.counter == 1 {
            self.mask = 0b1000_0000;
        }

        return result;
    }

    fn hmove_tick(&mut self, hmove_counter: i8) {
        if self.hmove_offset >= hmove_counter {
            self.tick();
        }
    }

    /// Resets player position. Called when RESPx register gets strobed.
    fn reset_player_position(&mut self) {
        self.reset_countdown = 6;
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
#[derive(PartialEq, Copy, Clone, Debug)]
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
pub const HBLANK_EXTENDED_WIDTH: u32 = 68 + 8;
pub const FRAME_WIDTH: u32 = 160;
pub const LAST_COLUMN: u32 = TOTAL_WIDTH - 1;
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

    // Write registers:
    pub const VSYNC: u16 = 0x00;
    pub const VBLANK: u16 = 0x01;
    pub const WSYNC: u16 = 0x02;
    pub const RSYNC: u16 = 0x03;
    // pub const NUSIZ0: u16 = 0x04;
    // pub const NUSIZ1: u16 = 0x05;
    pub const COLUP0: u16 = 0x06;
    pub const COLUP1: u16 = 0x07;
    pub const COLUPF: u16 = 0x08;
    pub const COLUBK: u16 = 0x09;
    pub const CTRLPF: u16 = 0x0A;
    // pub const REFP0: u16 = 0x0B;
    // pub const REFP1: u16 = 0x0C;
    pub const PF0: u16 = 0x0D;
    pub const PF1: u16 = 0x0E;
    pub const PF2: u16 = 0x0F;
    pub const RESP0: u16 = 0x10;
    pub const RESP1: u16 = 0x11;
    // pub const RESM0: u16 = 0x12;
    // pub const RESM1: u16 = 0x13;
    // pub const RESBL: u16 = 0x14;
    pub const AUDC0: u16 = 0x15;
    pub const AUDC1: u16 = 0x16;
    pub const AUDF0: u16 = 0x17;
    pub const AUDF1: u16 = 0x18;
    pub const AUDV0: u16 = 0x19;
    pub const AUDV1: u16 = 0x1A;
    pub const GRP0: u16 = 0x1B;
    pub const GRP1: u16 = 0x1C;
    // pub const ENAM0: u16 = 0x1D;
    // pub const ENAM1: u16 = 0x1E;
    // pub const ENABL: u16 = 0x1F;
    pub const HMP0: u16 = 0x20;
    pub const HMP1: u16 = 0x21;
    // pub const HMM0: u16 = 0x22;
    // pub const HMM1: u16 = 0x23;
    // pub const HMBL: u16 = 0x24;
    // pub const VDELP0: u16 = 0x25;
    // pub const VDELP1: u16 = 0x26;
    // pub const VDELBL: u16 = 0x27;
    // pub const RESMP0: u16 = 0x28;
    // pub const RESMP1: u16 = 0x29;
    pub const HMOVE: u16 = 0x2A;
    // pub const HMCLR: u16 = 0x2B;
    // pub const CXCLR: u16 = 0x2C;

    // Read registers:
    // pub const CXM0P: u16 = 0x00;
    // pub const CXM1P: u16 = 0x01;
    // pub const CXP0FB: u16 = 0x02;
    // pub const CXP1FB: u16 = 0x03;
    // pub const CXM0FB: u16 = 0x04;
    // pub const CXM1FB: u16 = 0x05;
    // pub const CXBLPF: u16 = 0x06;
    // pub const CXPPMM: u16 = 0x07;
    // pub const INPT0: u16 = 0x08;
    // pub const INPT1: u16 = 0x09;
    // pub const INPT2: u16 = 0x0A;
    // pub const INPT3: u16 = 0x0B;
    pub const INPT4: u16 = 0x0C;
    pub const INPT5: u16 = 0x0D;
}

/// Constants in this module are bit masks for setting and testing register
/// values.
pub mod flags {
    /// Bit mask for turning on VSYNC signal using `VSYNC` register.
    pub const VSYNC_ON: u8 = 0b0000_0010;
    /// Bit mask for turning on vertical blanking using `VBLANK` register.
    pub const VBLANK_ON: u8 = 0b0000_0010;
    /// Bit mask for turning on input latches using `VBLANK` register.
    pub const VBLANK_INPUT_LATCH: u8 = 0b0100_0000;
    /// Bit mask for turning on reflected playfield using `CTRLPF` register.
    pub const CTRLPF_REFLECT: u8 = 0b0000_0001;
    /// Bit mask for turning on the playfield score mode using `CTRLPF` register.
    #[cfg(test)]
    pub const CTRLPF_SCORE: u8 = 0b0000_0010;
    // Indicates a HIGH status of an input port.
    pub const INPUT_HIGH: u8 = 1 << 7;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::decode_video_outputs;
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
    fn draws_players() {
        let mut tia = Tia::new();
        tia.write(registers::COLUBK, 0x02).unwrap();
        tia.write(registers::COLUP0, 0x04).unwrap();
        tia.write(registers::COLUP1, 0x06).unwrap();
        tia.write(registers::GRP0, 0b1010_0101).unwrap();
        tia.write(registers::GRP1, 0b1100_0011).unwrap();

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
             22222222222222222222222222222424224242662222662222222222222222222222222222222222\
             22222222222222222222222222222222222222222222222222222222222222222222222222222222",
        );

        tia.write(registers::COLUP0, 0x08).unwrap();
        tia.write(registers::COLUP1, 0x0A).unwrap();
        tia.write(registers::GRP0, 0b1111_0101).unwrap();
        tia.write(registers::GRP1, 0b1010_1111).unwrap();

        let p0_delay = 36 * 3;
        let p1_delay = 6 * 3;
        wait_ticks(&mut tia, p0_delay);
        tia.write(registers::RESP0, 0).unwrap();
        wait_ticks(&mut tia, p1_delay);
        tia.write(registers::RESP1, 0).unwrap();
        wait_ticks(&mut tia, TOTAL_WIDTH - p0_delay - p1_delay);

        assert_eq!(
            encode_video_outputs(scan_video(&mut tia, TOTAL_WIDTH)),
            "................||||||||||||||||....................................\
             22222222222222222222222222222222222222222222222888828282222222222A2A2AAAA2222222\
             22222222222222222222222222222222222222222222222222222222222222222222222222222222",
        );
    }

    #[test]
    fn moves_players() {
        let mut tia = Tia::new();
        tia.write(registers::COLUBK, 0x00).unwrap();
        tia.write(registers::COLUP0, 0x02).unwrap();
        tia.write(registers::COLUP1, 0x04).unwrap();
        tia.write(registers::GRP0, 0b1110_0111).unwrap();
        tia.write(registers::GRP1, 0b1101_1011).unwrap();
        tia.write(registers::HMP0, 3 << 4).unwrap();
        tia.write(registers::HMP1, (-5i8 << 4) as u8).unwrap();

        let p0_delay = 32 * 3;
        let p1_delay = 6 * 3;
        wait_ticks(&mut tia, p0_delay);
        tia.write(registers::RESP0, 0).unwrap();
        wait_ticks(&mut tia, p1_delay);
        tia.write(registers::RESP1, 0).unwrap();
        wait_ticks(&mut tia, TOTAL_WIDTH - p0_delay - p1_delay);

        // Pretend we're doing an STA: wait for 2 CPU cycles, write to register
        // on the 3rd one.
        let mut scanline = scan_video(&mut tia, 2 * 3 + 1);
        tia.write(registers::HMOVE, 0).unwrap();
        scanline.append(&mut scan_video(&mut tia, TOTAL_WIDTH - (2 * 3 + 1)));

        assert_eq!(
            encode_video_outputs(scanline),
            "................||||||||||||||||....................................\
             ........000000000000000000000000222002220000000000000000004404404400000000000000\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000",
        );
    }

    #[test]
    fn address_mirroring() {
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
    fn unlatched_input_ports() {
        let mut tia = Tia::new();
        tia.write(registers::VBLANK, 0).unwrap();

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
}
