//! Constants in this module are bit masks for setting and testing register
//! values.

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
/// Bit mask for turning playfield/ball priority using the `CTRLPF` register.
pub const CTRLPF_PRIORITY: u8 = 0b0000_0100;
/// Bit mask for resetting missile position to player using the `RESMPx`
/// registers.
pub const RESMPX_RESET: u8 = 0b0000_0010;
/// Bit mask for reflecting player graphics using `REFPx` registers.
pub const REFPX_REFLECT: u8 = 0b0000_1000;
/// Bit mask for turning on sprites using `ENAxx` registers.
pub const ENAXX_ENABLE: u8 = 0b0000_0010;
/// Bit mask for turning on the vertical delay flags for sprites using `VDELxx`
/// registers.
pub const VDELXX_ON: u8 = 0b0000_0001;

// Indicates a HIGH status of an input port.
pub const INPUT_HIGH: u8 = 1 << 7;
