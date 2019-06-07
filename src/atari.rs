use crate::address_space::AddressSpace;
use crate::cpu::CPU;
use crate::memory::RAM;
use crate::tia::TIA;
use crate::frame_renderer::FrameRenderer;
use crate::frame_renderer::FrameRendererBuilder;
use crate::colors;
use image;
use image::DynamicImage;
use image::GenericImageView;
use image::Pixel;
use image::Rgba;
use image::RgbaImage;
use lcs_image_diff;
use std::fs;
use std::path::Path;

type AtariAddressSpace = AddressSpace<TIA, RAM, RAM>;

struct Atari<'a> {
    cpu: CPU<'a, AtariAddressSpace>,
    frame_renderer: FrameRenderer,
}

impl<'a> Atari<'a> {
    pub fn new(address_space: &mut AtariAddressSpace) -> Atari {
        Atari {
            cpu: CPU::new(address_space),
            frame_renderer: FrameRendererBuilder::new().with_palette(colors::ntsc_palette_2()).build(),
            // img: RgbaImage::new(160, 192),
            // img: RgbaImage::from_pixel(1970, 1540, Rgba::from_channels(0, 0, 0, 255)),
            // img: image::open("src/test_data/horizontal_stripes.png")
            //     .unwrap()
            //     .to_rgba(),
        }
    }

    pub fn next_frame(&mut self) -> &RgbaImage {
        loop {
            let tia_result = self.cpu.memory().tia.tick();
            if tia_result.cpu_tick {
                self.cpu.tick();
            }
            if self.frame_renderer.consume(tia_result.video) {
                return self.frame_renderer.frame_image();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_images_equal(mut actual: DynamicImage, mut expected: DynamicImage, test_name: &str) {
        let start = std::time::Instant::now();
        let equal = itertools::equal(actual.pixels(), expected.pixels());
        if equal {
            return;
        }

        let dir_path = Path::new(env!("OUT_DIR")).join("test_results");
        fs::create_dir_all(&dir_path).unwrap();
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

        let diff = lcs_image_diff::compare(&mut actual, &mut expected, 0.5).unwrap();

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

    #[test]
    fn shows_horizontal_stripes() {
        let rom_image = std::fs::read(
            Path::new(env!("OUT_DIR"))
                .join("roms")
                .join("horizontal_stripes.bin"),
        )
        .unwrap();
        let mut address_space = AtariAddressSpace {
            tia: TIA::new(),
            ram: RAM::new(),
            rom: RAM::with_program(&rom_image[..]),
        };
        let mut atari = Atari::new(&mut address_space);
        atari.cpu.reset();
        let img1 = DynamicImage::ImageRgba8(atari.next_frame().clone());
        let img2 = image::open(
            Path::new("src")
                .join("test_data")
                .join("horizontal_stripes.png"),
        )
        .unwrap();
        assert_images_equal(img1, img2, "shows_horizontal_stripes");
    }
}
