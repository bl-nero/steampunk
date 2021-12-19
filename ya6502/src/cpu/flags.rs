pub const N: u8 = 1 << 7;
pub const V: u8 = 1 << 6;
pub const UNUSED: u8 = 1 << 5;
pub const B: u8 = 1 << 4;
pub const D: u8 = 1 << 3;
pub const I: u8 = 1 << 2;
pub const Z: u8 = 1 << 1;
pub const C: u8 = 1;

/// Flags that are always on when the flag register is programmatically pushed
/// onto the stack.
pub const PUSHED: u8 = B | UNUSED;

pub fn flags_to_string(flags: u8) -> String {
    format!("{:08b}", flags)
        .chars()
        .map(|ch| match ch {
            '0' => '.',
            '1' => '*',
            _ => ch,
        })
        .collect()
}
