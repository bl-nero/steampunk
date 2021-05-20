use crate::address_space::AddressSpace;
use crate::colors;
use crate::cpu::Cpu;
use crate::frame_renderer::FrameRenderer;
use crate::frame_renderer::FrameRendererBuilder;
use crate::memory::{AtariRam, AtariRom};
use crate::riot::Riot;
use crate::tia::Tia;
use image;
use image::RgbaImage;

pub type AtariAddressSpace = AddressSpace<Tia, AtariRam, Riot, AtariRom>;

pub struct Atari {
    cpu: Cpu<AtariAddressSpace>,
    frame_renderer: FrameRenderer,
    running: bool,
}

enum TickResult {
    FramePending,
    FrameComplete,
    Error,
}

impl Atari {
    pub fn new(address_space: Box<AtariAddressSpace>) -> Self {
        Atari {
            cpu: Cpu::new(address_space),
            frame_renderer: FrameRendererBuilder::new()
                .with_palette(colors::ntsc_palette())
                .build(),
            running: false,
        }
    }

    pub fn next_frame(&mut self) -> &RgbaImage {
        while self.running {
            match self.tick() {
                TickResult::FramePending => {}
                TickResult::FrameComplete => return self.frame_renderer.frame_image(),
                TickResult::Error => self.running = false,
            }
        }
        return self.frame_renderer.frame_image();
    }

    /// Performs a single clock tick. If it resulted in an error reported by the
    /// CPU, dump debug information on standard error stream and return
    /// `TickResult::Error`.
    fn tick(&mut self) -> TickResult {
        let tia_result = self.cpu.memory().tia.tick();
        if tia_result.cpu_tick {
            if let Err(e) = self.cpu.tick() {
                eprintln!("ERROR: {}. Atari halted.", e);
                eprintln!("{}", self.cpu);
                eprintln!("{}", self.cpu.memory());
                return TickResult::Error;
            }
        }
        return if self.frame_renderer.consume(tia_result.video) {
            TickResult::FrameComplete
        } else {
            TickResult::FramePending
        };
    }

    pub fn frame_image(&self) -> &RgbaImage {
        self.frame_renderer.frame_image()
    }

    pub fn reset(&mut self) {
        self.running = true;
        self.cpu.reset();
        for _ in 0..8 {
            self.tick();
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use super::*;
    use image::DynamicImage;
    use image::GenericImageView;
    use lcs_image_diff;
    use std::fs;
    use std::path::Path;
    use test::Bencher;

    fn read_test_rom(name: &str) -> Vec<u8> {
        std::fs::read(Path::new(env!("OUT_DIR")).join("roms").join(name)).unwrap()
    }

    fn read_test_image(name: &str) -> DynamicImage {
        image::open(Path::new("src").join("test_data").join(name)).unwrap()
    }

    fn atari_with_rom(file_name: &str) -> Atari {
        let rom = read_test_rom(file_name);
        let address_space = Box::new(AtariAddressSpace {
            tia: Tia::new(),
            ram: AtariRam::new(),
            riot: Riot::new(),
            rom: AtariRom::new(&rom).unwrap(),
        });
        let mut atari = Atari::new(address_space);
        atari.reset();
        return atari;
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
        let mut atari = atari_with_rom("horizontal_stripes.bin");

        let expected_image = read_test_image("horizontal_stripes_1.png");
        let actual_image = DynamicImage::ImageRgba8(atari.next_frame().clone());

        assert_images_equal(actual_image, expected_image, "shows_horizontal_stripes");
    }

    #[test]
    fn animates_horizontal_stripes() {
        let mut atari = atari_with_rom("horizontal_stripes_animated.bin");

        let expected_image_1 = read_test_image("horizontal_stripes_1.png");
        let expected_image_2 = read_test_image("horizontal_stripes_2.png");
        let actual_image_1 = DynamicImage::ImageRgba8(atari.next_frame().clone());
        let actual_image_2 = DynamicImage::ImageRgba8(atari.next_frame().clone());

        assert_images_equal(
            actual_image_1,
            expected_image_1,
            "animates_horizontal_stripes_1",
        );
        assert_images_equal(
            actual_image_2,
            expected_image_2,
            "animates_horizontal_stripes_2",
        );
    }

    #[test]
    fn stops_on_error() {
        let mut atari = atari_with_rom("halt.bin");

        let expected_image = read_test_image("stops_on_error.png");
        let actual_image = DynamicImage::ImageRgba8(atari.next_frame().clone());

        assert_images_equal(actual_image, expected_image, "stops_on_error");
    }

    #[bench]
    fn benchmark(b: &mut Bencher) {
        let rom = read_test_rom("horizontal_stripes.bin");
        b.iter(|| {
            let address_space = Box::new(AtariAddressSpace {
                tia: Tia::new(),
                ram: AtariRam::new(),
                riot: Riot::new(),
                rom: AtariRom::new(&rom).unwrap(),
            });
            let mut atari = Atari::new(address_space);

            atari.reset();
            atari.next_frame();
        });
    }
}
