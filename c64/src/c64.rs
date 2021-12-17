use crate::address_space::AddressSpace;
use crate::address_space::Cartridge;
use crate::address_space::VicAddressSpace;
use crate::cia::Cia;
use crate::frame_renderer::FrameRenderer;
use crate::sid::Sid;
use crate::Vic;
use image::RgbaImage;
use std::cell::RefCell;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use ya6502::cpu::Cpu;
use ya6502::memory::Ram;
use ya6502::memory::Rom;

pub type C64AddressSpace = AddressSpace<Vic<VicAddressSpace<Ram, Rom>, Ram>, Sid, Cia>;

pub struct C64 {
    cpu: Cpu<C64AddressSpace>,
    frame_renderer: FrameRenderer,
}

impl C64 {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let basic_rom = fs::read(Path::new(env!("OUT_DIR")).join("roms").join("basic.bin"))?;
        let char_rom = fs::read(Path::new(env!("OUT_DIR")).join("roms").join("char.bin"))?;
        let kernal_rom = fs::read(Path::new(env!("OUT_DIR")).join("roms").join("kernal.bin"))?;
        let ram = Rc::new(RefCell::new(Ram::new(16)));
        let color_ram = Rc::new(RefCell::new(Ram::new(10)));
        Ok(C64 {
            cpu: Cpu::new(Box::new(C64AddressSpace::new(
                ram.clone(),
                Rom::new(&basic_rom)?,
                Vic::new(
                    Box::new(VicAddressSpace::new(
                        ram,
                        Rc::new(RefCell::new(Rom::new(&char_rom)?)),
                    )),
                    color_ram.clone(),
                ),
                Sid::new(),
                color_ram,
                Cia::new(),
                Cia::new(),
                Rom::new(&kernal_rom)?,
            ))),
            frame_renderer: FrameRenderer::default(),
        })
    }

    pub fn frame_image(&self) -> &RgbaImage {
        self.frame_renderer.frame_image()
    }

    pub fn cpu(&self) -> &Cpu<C64AddressSpace> {
        &self.cpu
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
    }

    pub fn tick(&mut self) -> Result<(), Box<dyn Error>> {
        self.frame_renderer
            .consume(self.cpu.mut_memory().mut_vic().tick()?);
        // OK, that's 8 times faster than the _actual_ 6510 CPU. So we don't
        // care about timing for now, big deal. We have bigger problems.
        self.cpu.tick()?;
        Ok(())
    }

    pub fn set_cartridge(&mut self, cartridge: Option<Cartridge>) {
        self.cpu.mut_memory().cartridge = cartridge;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address_space::CartridgeMode;
    use crate::vic::RASTER_LENGTH;
    use crate::vic::TOTAL_HEIGHT;
    use common::test_utils::read_test_image;
    use image::DynamicImage;
    use std::error::Error;

    fn next_frame(c64: &mut C64) -> Result<RgbaImage, Box<dyn Error>> {
        for _ in 0..RASTER_LENGTH * TOTAL_HEIGHT {
            c64.tick()?;
        }
        Ok(c64.frame_image().clone())
    }

    pub fn assert_images_equal(actual: DynamicImage, expected: DynamicImage, test_name: &str) {
        common::test_utils::assert_images_equal(
            actual,
            expected,
            test_name,
            &Path::new(env!("OUT_DIR")).join("test_results"),
        )
    }

    fn assert_produces_frame(c64: &mut C64, test_image_name: &str, test_name: &str) {
        let actual_image = DynamicImage::ImageRgba8(next_frame(c64).unwrap());
        let expected_image = read_test_image(test_image_name);
        assert_images_equal(actual_image, expected_image, test_name);
    }

    pub fn read_test_rom(name: &str) -> Vec<u8> {
        std::fs::read(Path::new(env!("OUT_DIR")).join("test_roms").join(name)).unwrap()
    }

    pub fn c64_with_cartridge(file_name: &str) -> C64 {
        let mut c64 = C64::new().unwrap();
        c64.set_cartridge(Some(Cartridge {
            mode: CartridgeMode::Ultimax,
            rom: Rom::new(&read_test_rom(file_name)).unwrap(),
        }));
        c64.reset();
        return c64;
    }

    #[test]
    fn shows_hello_world() {
        // Note: Once 6502 runs with its actual speed, we'll probably need to wait for a frame or two.
        let mut c64 = c64_with_cartridge("hello_world.bin");
        assert_produces_frame(&mut c64, "hello_world.png", "shows_hello_world");
    }
}
