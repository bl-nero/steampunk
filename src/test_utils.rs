#![cfg(test)]

use crate::tia::TIAOutput;
use std::iter;

/// Decodes a convenient, character-based representation of a TIA output to an
/// iterator over a `TIAOutput` structure. Useful for representing test cases in
/// a clear, concise way.
///
/// | Character | Meaning                                   |
/// |-----------|-------------------------------------------|
/// | .         | Blank                                     |
/// | |         | VSYNC                                     |
/// | -         | HSYNC                                     |
/// | +         | HSYNC + VSYNC                             |
/// | 02468ACE  | Pixel (0x00 - 0x0E)                       |
/// | =         | Special case: VSYNC with pixel value 0x00 |
///
/// # Example
///
/// This is a typical example that produces two scanlines:
///
/// ```
/// let outputs = decode_tia_outputs(
///     "................||||||||||||||||....................................\
///      00000000000000000222222222222222222222200044444444444444400000000000000000000000\
///      00000000000000000000000000000000000000000044444444444444444444444444444440000000\
///      ................||||||||||||||||....................................\
///      88888888888888888AAAAAAAAAAAAAAAAAAAAAAAAAAA666600000000000000000000000000000000\
///      000000000000000000000000000000000000000000000EEEEEEEEEEEEEE222222222222222222222"
/// );
/// ```
pub fn decode_tia_outputs<'a>(encoded_signal: &'a str) -> impl Iterator<Item = TIAOutput> + 'a {
    encoded_signal.chars().map(|c| match c {
        '.' => TIAOutput::blank(),
        '|' => TIAOutput::blank().with_hsync(),
        '-' => TIAOutput::blank().with_vsync(),
        '+' => TIAOutput::blank().with_hsync().with_vsync(),
        '=' => TIAOutput::pixel(0x00).with_vsync(),
        _ => {
            let color = u8::from_str_radix(&c.to_string(), 16);
            let color = color.expect(&format!("Illegal character: {}", c));
            return TIAOutput::pixel(color);
        }
    })
}

mod tests {
    use super::*;

    #[test]
    fn decodes_tia_outputs() {
        itertools::assert_equal(decode_tia_outputs(""), iter::empty());
        itertools::assert_equal(
            decode_tia_outputs(".|-+02468ACE="),
            [
                TIAOutput::blank(),
                TIAOutput::blank().with_hsync(),
                TIAOutput::blank().with_vsync(),
                TIAOutput::blank().with_hsync().with_vsync(),
                TIAOutput::pixel(0x00),
                TIAOutput::pixel(0x02),
                TIAOutput::pixel(0x04),
                TIAOutput::pixel(0x06),
                TIAOutput::pixel(0x08),
                TIAOutput::pixel(0x0A),
                TIAOutput::pixel(0x0C),
                TIAOutput::pixel(0x0E),
                TIAOutput::pixel(0x00).with_vsync(),
            ]
            .iter()
            .cloned(),
        );
    }
}
