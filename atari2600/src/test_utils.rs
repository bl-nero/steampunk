#![cfg(test)]
use crate::colors;
use crate::tia::VideoOutput;
use crate::Atari;
use crate::AtariAddressSpace;
use crate::AtariRam;
use crate::AtariRom;
use crate::FrameRendererBuilder;
use crate::Riot;
use crate::Tia;
use image::DynamicImage;
use std::fs::create_dir_all;
use std::iter;
use std::path::Path;

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

pub fn atari_with_rom(file_name: &str) -> Atari {
    let rom = read_test_rom(file_name);
    let address_space = Box::new(AtariAddressSpace {
        tia: Tia::new(),
        ram: AtariRam::new(),
        riot: Riot::new(),
        rom: AtariRom::new(&rom).unwrap(),
    });
    let mut atari = Atari::new(
        address_space,
        FrameRendererBuilder::new()
            .with_palette(colors::ntsc_palette())
            .build(),
    );
    atari.reset().unwrap();
    return atari;
}

pub fn read_test_rom(name: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("OUT_DIR")).join("roms").join(name)).unwrap()
}

pub fn read_test_image(name: &str) -> DynamicImage {
    image::open(Path::new("src").join("test_data").join(name)).unwrap()
}

pub fn assert_images_equal(actual: DynamicImage, expected: DynamicImage, test_name: &str) {
    use image::GenericImageView;
    let equal = itertools::equal(actual.pixels(), expected.pixels());
    if equal {
        return;
    }

    let dir_path = Path::new(env!("OUT_DIR")).join("test_results");
    create_dir_all(&dir_path).unwrap();
    let actual_path = dir_path
        .join(String::from(test_name) + "-actual")
        .with_extension("png");
    let expected_path = dir_path
        .join(String::from(test_name) + "-expected")
        .with_extension("png");
    let diff_path = dir_path
        .join(String::from(test_name) + "-diff")
        .with_extension("png");
    let new_golden_path = dir_path
        .join(String::from(test_name) + "-new-golden")
        .with_extension("png");
    actual.save(&new_golden_path).unwrap();

    let diff = image_diff::diff(&expected, &actual).unwrap();

    actual.save(&actual_path).unwrap();
    expected.save(&expected_path).unwrap();
    diff.save(&diff_path).unwrap();
    panic!(
        "Images differ for test {}\nActual: {}\nExpected: {}\nDiff: {}\nNew golden: {}",
        test_name,
        actual_path.display(),
        expected_path.display(),
        diff_path.display(),
        new_golden_path.display(),
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
                VideoOutput::pixel(0x00).with_vsync(),
            ]),
            ".|-+02468ACE=",
        );
    }
}
