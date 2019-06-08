#![cfg(test)]

use crate::tia::VideoOutput;
use std::iter;

/// Decodes a convenient, character-based representation of a TIA video output to
/// an iterator over a `VideoOutput` structure. Useful for representing test
/// cases in a clear, concise way.
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
/// let outputs = decode_video_outputs(
///     "................||||||||||||||||....................................\
///      00000000000000000222222222222222222222200044444444444444400000000000000000000000\
///      00000000000000000000000000000000000000000044444444444444444444444444444440000000\
///      ................||||||||||||||||....................................\
///      88888888888888888AAAAAAAAAAAAAAAAAAAAAAAAAAA666600000000000000000000000000000000\
///      000000000000000000000000000000000000000000000EEEEEEEEEEEEEE222222222222222222222"
/// );
/// ```
pub fn decode_video_outputs<'a>(encoded_signal: &'a str) -> impl Iterator<Item = VideoOutput> + 'a {
    encoded_signal.chars().map(|c| match c {
        '.' => VideoOutput::blank(),
        '|' => VideoOutput::blank().with_hsync(),
        '-' => VideoOutput::blank().with_vsync(),
        '+' => VideoOutput::blank().with_hsync().with_vsync(),
        '=' => VideoOutput::pixel(0x00).with_vsync(),
        _ => {
            let color = u8::from_str_radix(&c.to_string(), 16);
            let color = color.expect(&format!("Illegal character: {}", c));
            return VideoOutput::pixel(color);
        }
    })
}

mod tests {
    use super::*;

    #[test]
    fn decodes_video_outputs() {
        itertools::assert_equal(decode_video_outputs(""), iter::empty());
        itertools::assert_equal(
            decode_video_outputs(".|-+02468ACE="),
            [
                VideoOutput::blank(),
                VideoOutput::blank().with_hsync(),
                VideoOutput::blank().with_vsync(),
                VideoOutput::blank().with_hsync().with_vsync(),
                VideoOutput::pixel(0x00),
                VideoOutput::pixel(0x02),
                VideoOutput::pixel(0x04),
                VideoOutput::pixel(0x06),
                VideoOutput::pixel(0x08),
                VideoOutput::pixel(0x0A),
                VideoOutput::pixel(0x0C),
                VideoOutput::pixel(0x0E),
                VideoOutput::pixel(0x00).with_vsync(),
            ]
            .iter()
            .cloned(),
        );
    }
}
