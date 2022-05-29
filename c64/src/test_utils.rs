#![cfg(test)]

use crate::Cartridge;
use crate::CartridgeMode;
use crate::C64;
use common::app::AppController;
use common::app::FrameStatus;
use common::app::Machine;
use image::RgbaImage;
use std::error::Error;
use std::path::Path;
use ya6502::memory::Rom;

pub fn next_frame(c64: &mut C64) -> Result<RgbaImage, Box<dyn Error>> {
    loop {
        match c64.tick() {
            Ok(FrameStatus::Pending) => {}
            Ok(FrameStatus::Complete) => break,
            Err(e) => {
                eprintln!("ERROR: {}. Machine halted.", e);
                eprintln!("{}", c64.cpu());
                eprintln!("{}", c64.cpu().memory());
                return Err(e);
            }
        }
    }
    return Ok(c64.frame_image().clone());
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

pub fn read_test_rom(name: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("OUT_DIR")).join("test_roms").join(name)).unwrap()
}

pub fn c64_with_cartridge_uninitialized(file_name: &str) -> C64 {
    let mut c64 = C64::new().unwrap();
    c64.set_cartridge(Some(Cartridge {
        mode: CartridgeMode::Ultimax,
        rom: Rom::new(&read_test_rom(file_name)).unwrap(),
    }));
    c64.reset();
    return c64;
}

pub fn c64_with_cartridge(file_name: &str) -> C64 {
    let mut c64 = c64_with_cartridge_uninitialized(file_name);
    next_frame(&mut c64).unwrap(); // Skip the first partial frame.
    return c64;
}
