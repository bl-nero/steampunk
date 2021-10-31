mod audio_generator;
mod delay_buffer;
mod flags;
mod registers;
mod sprite;
mod tests;

use audio_generator::AudioGenerator;
use delay_buffer::DelayBuffer;
use enum_map::{enum_map, Enum, EnumMap};
use sprite::{missile_reset_delay_for_player, set_reg_nusiz, Sprite};
use ya6502::memory::{Memory, ReadError, ReadResult, WriteError, WriteResult};

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
    /// Register that resets missile 0 position to player 0.
    reg_resmp0: u8,
    /// Register that resets missile 1 position to player 1.
    reg_resmp1: u8,

    // Collision registers.
    reg_cxm0p: u8,
    reg_cxm1p: u8,
    reg_cxp0fb: u8,
    reg_cxp1fb: u8,
    reg_cxm0fb: u8,
    reg_cxm1fb: u8,
    reg_cxblpf: u8,
    reg_cxppmm: u8,

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
    /// Temporarily latches playfield bits for rendering.
    playfield_buffer: DelayBuffer<bool>,
    /// Latches the HMOVE signal until end of the scanline.
    hmove_latch: bool,
    /// Counts from 7 down to -8 while additional clock ticks are sent to the
    /// player graphics objects.
    hmove_counter: i8,

    player0: Sprite,
    player1: Sprite,
    missile0: Sprite,
    missile1: Sprite,
    audio0: AudioGenerator,
    audio1: AudioGenerator,

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
            reg_resmp0: 0,
            reg_resmp1: 0,

            reg_cxm0p: 0,
            reg_cxm1p: 0,
            reg_cxp0fb: 0,
            reg_cxp1fb: 0,
            reg_cxm0fb: 0,
            reg_cxm1fb: 0,
            reg_cxblpf: 0,
            reg_cxppmm: 0,

            reg_inpt: enum_map! { _ => flags::INPUT_HIGH },

            column_counter: 0,
            hsync_on: false,
            hblank_on: false,
            wait_for_sync: false,
            playfield_buffer: DelayBuffer::new(2),
            hmove_latch: false,
            hmove_counter: 0,

            player0: Sprite::new(),
            player1: Sprite::new(),
            missile0: Sprite::new(),
            missile1: Sprite::new(),
            audio0: AudioGenerator::new(),
            audio1: AudioGenerator::new(),

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
        let playfield_bit = self.playfield_tick();
        if self.hmove_latch && self.hmove_counter > -8 && self.column_counter % 4 == 0 {
            self.player0.hmove_tick(self.hmove_counter);
            self.player1.hmove_tick(self.hmove_counter);
            self.missile0.hmove_tick(self.hmove_counter);
            self.missile1.hmove_tick(self.hmove_counter);
            self.hmove_counter -= 1;
        }

        let p0_bit = self.player0.tick(!self.hblank_on);
        let p1_bit = self.player1.tick(!self.hblank_on);
        let m0_bit = self.missile0.tick(!self.hblank_on);
        let m1_bit = self.missile1.tick(!self.hblank_on);

        let pixel = if self.hblank_on {
            None
        } else {
            let resmp0 = self.reg_resmp0 & flags::RESMPX_RESET != 0;
            let resmp1 = self.reg_resmp1 & flags::RESMPX_RESET != 0;
            let m0_bit = !resmp0 && m0_bit;
            let m1_bit = !resmp1 && m1_bit;
            if resmp0 && self.player0.position_counter() == 1 {
                self.missile0
                    .reset_position(missile_reset_delay_for_player(&self.player0));
            }
            if resmp1 && self.player1.position_counter() == 1 {
                self.missile1
                    .reset_position(missile_reset_delay_for_player(&self.player1));
            }
            if vblank_on {
                None
            } else {
                if m0_bit && p1_bit {
                    self.reg_cxm0p |= 1 << 7;
                }
                if m0_bit && p0_bit {
                    self.reg_cxm0p |= 1 << 6;
                }
                if m1_bit && p0_bit {
                    self.reg_cxm1p |= 1 << 7;
                }
                if m1_bit && p1_bit {
                    self.reg_cxm1p |= 1 << 6;
                }
                if p0_bit && playfield_bit {
                    self.reg_cxp0fb |= 1 << 7;
                }
                if p1_bit && playfield_bit {
                    self.reg_cxp1fb |= 1 << 7;
                }
                if m0_bit && playfield_bit {
                    self.reg_cxm0fb |= 1 << 7;
                }
                if m1_bit && playfield_bit {
                    self.reg_cxm1fb |= 1 << 7;
                }
                if p0_bit && p1_bit {
                    self.reg_cxppmm |= 1 << 7;
                }
                if m0_bit && m1_bit {
                    self.reg_cxppmm |= 1 << 6;
                }
                Some(
                    if self.reg_ctrlpf & flags::CTRLPF_PRIORITY != 0 && playfield_bit {
                        self.reg_colupf
                    } else if p0_bit || m0_bit {
                        self.reg_colup0
                    } else if p1_bit || m1_bit {
                        self.reg_colup1
                    } else if self.reg_ctrlpf & flags::CTRLPF_PRIORITY == 0 && playfield_bit {
                        self.reg_colupf
                    } else {
                        self.reg_colubk
                    },
                )
            }
        };

        let output = TiaOutput {
            video: VideoOutput {
                hsync: self.hsync_on,
                vsync: vsync_on,
                pixel,
            },
            audio: self.audio_tick(),
            riot_tick: self.column_counter % 3 == 0,
            cpu_tick: !self.wait_for_sync && self.column_counter % 3 == 0,
        };

        self.column_counter = (self.column_counter + 1) % TOTAL_WIDTH;
        return output;
    }

    fn playfield_tick(&mut self) -> bool {
        if self.column_counter % 4 == 0 {
            self.playfield_buffer
                .shift(self.playfield_bit_at(self.playfiled_bit_index_to_latch()));
        }
        return *self.playfield_buffer.peek();
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

    fn audio_tick(&mut self) -> Option<AudioOutput> {
        // TODO: Temporary. Remove before merging to master.
        if self.column_counter != 0 && self.column_counter != TOTAL_WIDTH / 2 {
            return None;
        }
        return Some(AudioOutput {
            au0: self.audio0.tick(),
            au1: self.audio1.tick(),
        });
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
        match address & 0b0000_1111 {
            registers::CXM0P => Ok(self.reg_cxm0p),
            registers::CXM1P => Ok(self.reg_cxm1p),
            registers::CXP0FB => Ok(self.reg_cxp0fb),
            registers::CXP1FB => Ok(self.reg_cxp1fb),
            registers::CXM0FB => Ok(self.reg_cxm0fb),
            registers::CXM1FB => Ok(self.reg_cxm1fb),
            registers::CXBLPF => Ok(self.reg_cxblpf),
            registers::CXPPMM => Ok(self.reg_cxppmm),
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
            registers::NUSIZ0 => {
                set_reg_nusiz(&mut self.player0, &mut self.missile0, value);
            }
            registers::NUSIZ1 => {
                set_reg_nusiz(&mut self.player1, &mut self.missile1, value);
            }
            registers::COLUP0 => self.reg_colup0 = value,
            registers::COLUP1 => self.reg_colup1 = value,
            registers::COLUPF => self.reg_colupf = value,
            registers::COLUBK => self.reg_colubk = value,
            registers::CTRLPF => self.reg_ctrlpf = value,
            registers::REFP0 => self.player0.set_reg_refp(value),
            registers::REFP1 => self.player1.set_reg_refp(value),
            registers::PF0 => self.reg_pf0 = value,
            registers::PF1 => self.reg_pf1 = value,
            registers::PF2 => self.reg_pf2 = value,
            registers::RESP0 => self.player0.reset_position(5),
            registers::RESP1 => self.player1.reset_position(5),
            registers::RESM0 => self.missile0.reset_position(4),
            registers::RESM1 => self.missile1.reset_position(4),

            registers::AUDC0 => self.audio0.set_pattern(value),
            registers::AUDC1 => self.audio1.set_pattern(value),
            registers::AUDF0 => self.audio0.set_frequency_divider(value),
            registers::AUDF1 => self.audio1.set_frequency_divider(value),
            registers::AUDV0 => self.audio0.set_volume(value),
            registers::AUDV1 => self.audio1.set_volume(value),

            registers::GRP0 => {
                self.player1.shift_bitmaps();
                self.player0.set_bitmap(value);
            }
            registers::GRP1 => {
                self.player0.shift_bitmaps();
                self.player1.set_bitmap(value);
            }
            registers::ENAM0 => self.missile0.set_bitmap((value & flags::ENAXX_ENABLE) << 6),
            registers::ENAM1 => self.missile1.set_bitmap((value & flags::ENAXX_ENABLE) << 6),
            registers::HMP0 => self.player0.set_reg_hm(value),
            registers::HMP1 => self.player1.set_reg_hm(value),
            registers::HMM0 => self.missile0.set_reg_hm(value),
            registers::HMM1 => self.missile1.set_reg_hm(value),
            registers::VDELP0 => self.player0.set_reg_vdel(value),
            registers::VDELP1 => self.player1.set_reg_vdel(value),
            registers::RESMP0 => self.reg_resmp0 = value,
            registers::RESMP1 => self.reg_resmp1 = value,
            // Note: there is an additional delay here, but it requires emulating the Hφ1 signal.
            registers::HMOVE => {
                self.hmove_latch = true;
                self.hmove_counter = 7;
            }
            registers::HMCLR => {
                self.player0.set_reg_hm(0);
                self.player1.set_reg_hm(0);
                self.missile0.set_reg_hm(0);
                self.missile1.set_reg_hm(0);
            }
            registers::CXCLR => {
                self.reg_cxm0p = 0;
                self.reg_cxm1p = 0;
                self.reg_cxp0fb = 0;
                self.reg_cxp1fb = 0;
                self.reg_cxm0fb = 0;
                self.reg_cxm1fb = 0;
                self.reg_cxblpf = 0;
                self.reg_cxppmm = 0;
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

/// TIA output structure. It indicates how a single TIA clock tick influences
/// other parts of the system.
pub struct TiaOutput {
    pub video: VideoOutput,
    pub audio: Option<AudioOutput>,
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

pub struct AudioOutput {
    pub au0: u8,
    pub au1: u8,
}
