use crate::address_space::AddressSpace;
use crate::colors;
use crate::cpu::CPU;
use crate::frame_renderer::FrameRenderer;
use crate::frame_renderer::FrameRendererBuilder;
use crate::memory::RAM;
use crate::tia::TIA;
use image;
use image::RgbaImage;

type AtariAddressSpace = AddressSpace<TIA, RAM, RAM>;

pub struct Atari<'a> {
    cpu: CPU<'a, AtariAddressSpace>,
    frame_renderer: FrameRenderer,
}

impl<'a> Atari<'a> {
    pub fn new(address_space: &mut AtariAddressSpace) -> Atari {
        Atari {
            cpu: CPU::new(address_space),
            frame_renderer: FrameRendererBuilder::new()
                .with_palette(colors::ntsc_palette())
                .build(),
        }
    }

    pub fn next_frame(&mut self) -> &RgbaImage {
        loop {
            let frame_complete = self.tick();
            if frame_complete {
                return self.frame_renderer.frame_image();
            }
        }
    }

    pub fn tick(&mut self) -> bool {
        let tia_result = self.cpu.memory().tia.tick();
        if tia_result.cpu_tick {
            self.cpu.tick();
        }
        return self.frame_renderer.consume(tia_result.video);
    }

    pub fn frame_image(&self) -> &RgbaImage {
        self.frame_renderer.frame_image()
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;
    use image::GenericImageView;
    use lcs_image_diff;
    use std::fs;
    use std::path::Path;

    fn read_test_rom(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("OUT_DIR"))
                .join("roms")
                .join(name),
        )
        .unwrap()
    }

    fn read_test_image(name: &str) -> DynamicImage {
        image::open(
            Path::new("src")
                .join("test_data")
                .join(name),
        )
        .unwrap()
    }

    fn assert_images_equal(mut actual: DynamicImage, mut expected: DynamicImage, test_name: &str) {
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

        let diff = lcs_image_diff::compare(&mut actual, &mut expected, 0.8).unwrap();

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
        let rom = read_test_rom("horizontal_stripes.bin");
        let mut address_space = AtariAddressSpace {
            tia: TIA::new(),
            ram: RAM::new(),
            rom: RAM::with_program(&rom[..]),
        };
        let mut atari = Atari::new(&mut address_space);

        atari.cpu.reset();
        let expected_image = read_test_image("horizontal_stripes_1.png");
        let actual_image = DynamicImage::ImageRgba8(atari.next_frame().clone());

        assert_images_equal(actual_image, expected_image, "shows_horizontal_stripes");
    }

    #[test]
    fn animates_horizontal_stripes() {
        let rom = read_test_rom("horizontal_stripes_animated.bin");
        let expected_image_1 = read_test_image("horizontal_stripes_1.png");
        let expected_image_2 = read_test_image("horizontal_stripes_2.png");

        let mut address_space = AtariAddressSpace {
            tia: TIA::new(),
            ram: RAM::new(),
            rom: RAM::with_program(&rom[..]),
        };
        let mut atari = Atari::new(&mut address_space);

        atari.cpu.reset();
        let actual_image_1 = DynamicImage::ImageRgba8(atari.next_frame().clone());
        let actual_image_2 = DynamicImage::ImageRgba8(atari.next_frame().clone());

        assert_images_equal(actual_image_1, expected_image_1, "animates_horizontal_stripes_1");
        assert_images_equal(actual_image_2, expected_image_2, "animates_horizontal_stripes_2");
    }
}
