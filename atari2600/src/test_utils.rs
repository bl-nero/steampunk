#![cfg(test)]
use crate::audio::create_consumer_and_source;
use crate::colors;
use crate::tia::VideoOutput;
use crate::Atari;
use crate::AtariAddressSpace;
use crate::FrameRendererBuilder;
use common::app::AppController;
use common::app::Machine;
use common::test_utils::as_single_hex_digit;
use image::DynamicImage;
use std::iter;
use std::path::Path;
use ya6502::memory::Rom;

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
            } => as_single_hex_digit(pixel),
            _ => '?',
        })
        .collect()
}

pub fn encode_audio<I: Iterator<Item = u8>>(outputs: I) -> String {
    outputs.map(as_single_hex_digit).collect()
}

pub fn atari_with_rom(file_name: &str) -> Atari {
    let rom = read_test_rom(file_name);
    let address_space = Box::new(AtariAddressSpace::new(Rom::new(&rom).unwrap()));
    let (consumer, _) = create_consumer_and_source();
    let mut atari = Atari::new(
        address_space,
        FrameRendererBuilder::new()
            .with_palette(colors::ntsc_palette())
            .build(),
        consumer,
    );
    atari.reset();
    return atari;
}

pub fn read_test_rom(name: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("OUT_DIR")).join("test_roms").join(name)).unwrap()
}

pub fn assert_images_equal(actual: DynamicImage, expected: DynamicImage, test_name: &str) {
    common::test_utils::assert_images_equal(
        actual,
        expected,
        test_name,
        &Path::new(env!("OUT_DIR")).join("test_results"),
    )
}

pub fn assert_current_frame(
    controller: &mut impl AppController,
    test_image_name: &str,
    test_name: &str,
) {
    common::test_utils::assert_current_frame(
        controller,
        test_image_name,
        test_name,
        &Path::new(env!("OUT_DIR")).join("test_results"),
    );
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
                VideoOutput::pixel(0xA0),
                VideoOutput::pixel(0x00).with_vsync(),
            ]),
            ".|-+02468ACE?=",
        );
    }

    #[test]
    fn encodes_audio() {
        assert_eq!(encode_audio(iter::empty()), "");
        assert_eq!(
            encode_audio(
                vec![0x0, 0x9, 0x8, 0xA, 0xF, 0xC, 0xE, 0x10]
                    .iter()
                    .copied()
            ),
            "098AFCE?"
        )
    }
}
