use crate::address_space::AddressSpace;
use crate::colors;
use crate::cpu::Cpu;
use crate::frame_renderer::FrameRenderer;
use crate::frame_renderer::FrameRendererBuilder;
use crate::memory::{AtariRam, AtariRom};
use crate::riot::{Riot, Switch, SwitchPosition};
use crate::tia::Tia;
use image;
use image::RgbaImage;
use std::error;

pub type AtariAddressSpace = AddressSpace<Tia, AtariRam, Riot, AtariRom>;

pub struct Atari {
    cpu: Cpu<AtariAddressSpace>,
    frame_renderer: FrameRenderer,
}

pub enum FrameStatus {
    Pending,
    Complete,
}

impl Atari {
    pub fn new(address_space: Box<AtariAddressSpace>) -> Self {
        Atari {
            cpu: Cpu::new(address_space),
            frame_renderer: FrameRendererBuilder::new()
                .with_palette(colors::ntsc_palette())
                .build(),
        }
    }

    pub fn cpu(&self) -> &Cpu<AtariAddressSpace> {
        &self.cpu
    }

    /// Performs a single clock tick. If it resulted in an error reported by the
    /// CPU, dump debug information on standard error stream and return
    /// `TickResult::Error`.
    pub fn tick(&mut self) -> Result<FrameStatus, Box<dyn error::Error>> {
        let tia_result = self.cpu.mut_memory().tia.tick();
        if tia_result.cpu_tick {
            if let Err(e) = self.cpu.tick() {
                return Err(e);
            }
            self.cpu.mut_memory().riot.tick();
        }
        return if self.frame_renderer.consume(tia_result.video) {
            Ok(FrameStatus::Complete)
        } else {
            Ok(FrameStatus::Pending)
        };
    }

    pub fn frame_image(&self) -> &RgbaImage {
        self.frame_renderer.frame_image()
    }

    pub fn reset(&mut self) -> Result<(), Box<dyn error::Error>> {
        self.cpu.reset();
        for _ in 0..8 {
            self.tick()?;
        }
        Ok(())
    }

    pub fn switch_position(&self, switch: Switch) -> SwitchPosition {
        self.cpu.memory().riot.switch_position(switch)
    }

    pub fn flip_switch(&mut self, switch: Switch, position: SwitchPosition) {
        self.cpu.mut_memory().riot.flip_switch(switch, position);
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use super::*;
    use crate::cpu::{opcodes, CpuHaltedError};
    use image::DynamicImage;
    use image::GenericImageView;
    use image_diff;
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
        atari.reset().unwrap();
        return atari;
    }

    fn next_frame(atari: &mut Atari) -> Result<RgbaImage, Box<dyn error::Error>> {
        loop {
            match atari.tick() {
                Ok(FrameStatus::Pending) => {}
                Ok(FrameStatus::Complete) => break,
                Err(e) => {
                    eprintln!("ERROR: {}. Atari halted.", e);
                    eprintln!("{}", atari.cpu);
                    eprintln!("{}", atari.cpu.memory());
                    return Err(e);
                }
            }
        }
        return Ok(atari.frame_renderer.frame_image().clone());
    }

    fn assert_images_equal(actual: DynamicImage, expected: DynamicImage, test_name: &str) {
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

        let diff = image_diff::diff(&expected, &actual).unwrap();
        // let diff = lcs_image_diff::compare(&mut actual, &mut expected, 0.8).unwrap();

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

    fn assert_produces_frame(atari: &mut Atari, test_image_name: &str, test_name: &str) {
        let actual_image = DynamicImage::ImageRgba8(next_frame(atari).unwrap());
        let expected_image = read_test_image(test_image_name);
        assert_images_equal(actual_image, expected_image, test_name);
    }

    #[test]
    fn shows_horizontal_stripes() {
        let mut atari = atari_with_rom("horizontal_stripes.bin");
        assert_produces_frame(
            &mut atari,
            "horizontal_stripes_1.png",
            "shows_horizontal_stripes",
        );
    }

    #[test]
    fn animates_horizontal_stripes() {
        let mut atari = atari_with_rom("horizontal_stripes_animated.bin");
        assert_produces_frame(
            &mut atari,
            "horizontal_stripes_1.png",
            "animates_horizontal_stripes_1",
        );
        assert_produces_frame(
            &mut atari,
            "horizontal_stripes_2.png",
            "animates_horizontal_stripes_2",
        );
    }

    #[test]
    fn uses_riot_timer_for_waiting() {
        let mut atari = atari_with_rom("skipping_stripes.bin");
        assert_produces_frame(
            &mut atari,
            "uses_riot_timer_for_waiting.png",
            "uses_riot_timer_for_waiting",
        );
    }

    #[test]
    fn reports_halt() {
        let mut atari = atari_with_rom("halt.bin");

        let expected_image = read_test_image("reports_halt.png");
        assert_eq!(
            *(*next_frame(&mut atari).unwrap_err())
                .downcast_ref::<CpuHaltedError>()
                .unwrap(),
            CpuHaltedError {
                opcode: opcodes::HLT1,
                address: 0xF2BA
            }
        );
        let actual_image = DynamicImage::ImageRgba8(atari.frame_image().clone());
        assert_images_equal(actual_image, expected_image, "reports_halt");
    }

    #[test]
    fn playfield_timing() {
        let mut atari = atari_with_rom("playfield_timing.bin");
        assert_produces_frame(&mut atari, "playfield_timing.png", "playfield_timing");
    }

    #[test]
    fn input() {
        let mut atari = atari_with_rom("io_monitor.bin");
        assert_produces_frame(&mut atari, "input_1.png", "input_1");

        atari.flip_switch(Switch::RightDifficulty, SwitchPosition::Down);
        atari.flip_switch(Switch::LeftDifficulty, SwitchPosition::Down);
        atari.flip_switch(Switch::TvType, SwitchPosition::Down);
        assert_produces_frame(&mut atari, "input_2.png", "input_2");

        atari.flip_switch(Switch::TvType, SwitchPosition::Up);
        atari.flip_switch(Switch::GameSelect, SwitchPosition::Down);
        assert_produces_frame(&mut atari, "input_3.png", "input_3");

        atari.flip_switch(Switch::LeftDifficulty, SwitchPosition::Up);
        atari.flip_switch(Switch::GameReset, SwitchPosition::Down);
        assert_produces_frame(&mut atari, "input_4.png", "input_4");

        atari.flip_switch(Switch::RightDifficulty, SwitchPosition::Up);
        assert_produces_frame(&mut atari, "input_5.png", "input_5");
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

            atari.reset().unwrap();
            next_frame(&mut atari).unwrap();
        });
    }
}
