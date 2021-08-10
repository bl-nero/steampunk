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

/// Encodes a sequence of video outputs. See `decode_video_outputs` for
/// description of the format. Non-conforming outputs are encoded as '?'.
pub fn encode_video_outputs<I: IntoIterator<Item = VideoOutput>>(outputs: I) -> String {
    outputs
        .into_iter()
        .map(|video_output| match video_output {
            VideoOutput {
                vsync: false,
                hsync: false,
                pixel: None,
            } => '.',
            VideoOutput {
                vsync: false,
                hsync: true,
                pixel: None,
            } => '|',
            VideoOutput {
                vsync: true,
                hsync: false,
                pixel: None,
            } => '-',
            VideoOutput {
                vsync: true,
                hsync: true,
                pixel: None,
            } => '+',
            VideoOutput {
                vsync: true,
                hsync: false,
                pixel: Some(0x00),
            } => '=',
            VideoOutput {
                vsync: false,
                hsync: false,
                pixel: Some(pixel),
            } => {
                if pixel <= 0x0f {
                    format!("{:X}", pixel)
                        .chars()
                        .last()
                        .expect("Hex formatting error")
                } else {
                    '?'
                }
            }
            _ => '?',
        })
        .collect()
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
            .copied(),
        );
    }

    #[test]
    fn encodes_video_outputs() {
        assert_eq!(encode_video_outputs(iter::empty()), "");
        assert_eq!(
            encode_video_outputs(vec![
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
            ]),
            ".|-+02468ACE=",
        );
    }
}
