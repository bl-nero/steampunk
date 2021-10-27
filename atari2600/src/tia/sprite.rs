use super::delay_buffer::DelayBuffer;
use super::flags;

/// A list of position counter values that trigger a "start drawing" signal for
/// player sprites. Indexes are values of NUSIZx registers, masked with
/// `flags::NUSIZX_PLAYER_MASK`.
const PLAYER_OFFSETS: [&[i32]; 8] = [
    &[156],
    &[156, 12],
    &[156, 28],
    &[156, 12, 28],
    &[156, 60],
    // Note: For some reason, double or quad-size player sprites add 1 CLK to
    // the position counter, hence 157 instead of 156.
    &[157],
    &[156, 28, 60],
    &[157],
];

/// A list of position counter values that trigger a "start drawing" signal for
/// missile sprites. Indexes are values of NUSIZx registers, masked with
/// `flags::NUSIZX_PLAYER_MASK`.
///
/// TODO: this probably needs tweaking.
const MISSILE_OFFSETS: [&[i32]; 8] = [
    &[156],
    &[156, 12],
    &[156, 28],
    &[156, 12, 28],
    &[156, 60],
    &[156],
    &[156, 28, 60],
    &[156],
];

/// Represents a sprite graphics state: the pixel counter and bitmap. Also
/// handles RESPx register strobing.
#[derive(Debug)]
pub struct Sprite {
    position_counter: i32,
    /// Position counter value where the current sprite copy was started.
    current_start: i32,
    /// A list of position counter values that trigger a "start drawing" signal.
    offsets: &'static [i32],
    scale: i32,
    /// New and old bitmap.
    bitmaps: [u8; 2],
    /// Index to the bitmaps array.
    bitmap_index: usize,
    /// A buffer that holds the bitmap to be drawn.
    bitmap_buffer: DelayBuffer<u8>,
    /// Index of the current bit being rendered (if any).
    current_bit: Option<u8>,
    reflect: bool,
    /// Counts down until position reset happens to emulate TIA latching delays.
    reset_countdown: i32,
    hmove_offset: i8,
    /// A buffer for bit masks.
    mask_buffer: DelayBuffer<u8>,
    /// A buffer that delays the "start drawing" signal.
    start_drawing_buffer: DelayBuffer<bool>,
}

impl Sprite {
    pub fn new() -> Self {
        Sprite {
            position_counter: 0,
            current_start: 0,
            offsets: PLAYER_OFFSETS[flags::NUSIZX_ONE_COPY as usize],
            scale: 1,
            bitmaps: [0b0000_0000, 0b0000_0000],
            bitmap_index: 0,
            bitmap_buffer: DelayBuffer::new(3),
            current_bit: None,
            reflect: false,
            reset_countdown: 0,
            hmove_offset: 0,
            mask_buffer: DelayBuffer::new(3),
            start_drawing_buffer: DelayBuffer::new(4),
        }
    }

    pub fn position_counter(&self) -> i32 {
        self.position_counter
    }

    /// Sets thee REFPx register value, which controls the player image
    /// reflection.
    pub fn set_reg_refp(&mut self, value: u8) {
        self.reflect = value & flags::REFPX_REFLECT != 0;
    }

    pub fn set_bitmap(&mut self, bitmap: u8) {
        self.bitmaps[0] = bitmap;
    }

    pub fn shift_bitmaps(&mut self) {
        self.bitmaps[1] = self.bitmaps[0];
    }

    /// Sets the HMxx register value, which controls the HMOVE offset.
    pub fn set_reg_hm(&mut self, value: u8) {
        self.hmove_offset = (value as i8) >> 4;
    }

    /// Sets the VDELPx register, which controls the player image delay.
    pub fn set_reg_vdel(&mut self, value: u8) {
        self.bitmap_index = (value & flags::VDELXX_ON) as usize;
    }

    /// Performs a clock tick and returns `true` if a player pixel should be
    /// drawn, or `false` otherwise.
    pub fn tick(&mut self, run_sprite_clock: bool) -> bool {
        if self.reset_countdown > 0 {
            self.reset_countdown -= 1;
            if self.reset_countdown == 0 {
                self.position_counter = 0;
            }
        }

        let bitmap = self.bitmap_buffer.shift(self.bitmaps[self.bitmap_index]);

        if run_sprite_clock {
            let start = self
                .start_drawing_buffer
                .shift(self.offsets.contains(&self.position_counter));
            if start {
                self.current_bit = Some(7);
                self.current_start = self.position_counter;
            }
            let mask = self.mask_buffer.shift(match self.current_bit {
                None => 0,
                Some(bit) => 1 << if self.reflect { 7 - bit } else { bit },
            });
            self.position_counter = (self.position_counter + 1) % 160;
            let go_to_next_bit = (self.position_counter - self.current_start) % self.scale == 0;
            if go_to_next_bit {
                self.current_bit = match self.current_bit {
                    None | Some(0) => None,
                    Some(bit) => Some(bit - 1),
                };
            }
            return bitmap & mask != 0;
        } else {
            return false;
        }
    }

    pub fn hmove_tick(&mut self, hmove_counter: i8) {
        if self.hmove_offset >= hmove_counter {
            self.tick(true);
        }
    }

    /// Resets player position. Called when RESPx register gets strobed.
    pub fn reset_position(&mut self, delay: i32) {
        self.reset_countdown = delay;
        if self.reset_countdown == 0 {
            self.position_counter = 0;
        }
    }
}

/// Sets sprites' offset and scale values basing on a NUSIZx register value.
pub fn set_reg_nusiz(player: &mut Sprite, missile: &mut Sprite, value: u8) {
    let player_value = value & flags::NUSIZX_PLAYER_MASK;
    let missile_value = value & flags::NUSIZX_MISSILE_WIDTH_MASK;
    player.offsets = PLAYER_OFFSETS[player_value as usize];
    player.scale = match player_value {
        flags::NUSIZX_DOUBLE_SIZED_PLAYER => 2,
        flags::NUSIZX_QUAD_SIZED_PLAYER => 4,
        _ => 1,
    };
    missile.offsets = MISSILE_OFFSETS[player_value as usize];
    missile.scale = match missile_value {
        flags::NUSIZX_MISSILE_WIDTH_1 => 1,
        flags::NUSIZX_MISSILE_WIDTH_2 => 2,
        flags::NUSIZX_MISSILE_WIDTH_4 => 4,
        flags::NUSIZX_MISSILE_WIDTH_8 => 8,
        _ => 1,
    };
}

/// Returns missile reset delay appropriate to the scale of player sprite.
pub fn missile_reset_delay_for_player(player: &Sprite) -> i32 {
    match player.scale {
        2 => 8,
        4 => 11,
        _ => 4,
    }
}
