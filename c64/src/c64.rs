use crate::address_space::AddressSpace;
use crate::address_space::Cartridge;
use crate::address_space::VicAddressSpace;
use crate::cia::Cia;
use crate::cia::PortName;
use crate::frame_renderer::FrameRenderer;
use crate::keyboard::Key;
use crate::keyboard::KeyState;
use crate::keyboard::Keyboard;
use crate::sid::Sid;
use crate::Vic;
use common::app::FrameStatus;
use common::app::Machine;
use delegate::delegate;
use image::RgbaImage;
use std::cell::RefCell;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use ya6502::cpu::Cpu;
use ya6502::cpu::MachineInspector;
use ya6502::memory::Ram;
use ya6502::memory::Rom;

pub type C64AddressSpace = AddressSpace<Vic<VicAddressSpace<Ram, Rom>, Ram>, Sid, Cia>;

pub struct C64 {
    cpu: Cpu<C64AddressSpace>,
    frame_renderer: FrameRenderer,

    cpu_clock_divider: u32,
    cia1_irq: bool,
    cia2_irq: bool,

    keyboard: Keyboard,
}

impl Machine for C64 {
    fn reset(&mut self) {
        let mem = self.cpu.mut_memory();
        mem.mut_cia1().write_port(PortName::A, 0b1111_1111);
        mem.mut_cia1().write_port(PortName::B, 0b1111_1111);
        mem.mut_cia2().write_port(PortName::A, 0b1111_1111);
        mem.mut_cia2().write_port(PortName::B, 0b1111_1111);
        self.cpu.reset();
    }

    fn tick(&mut self) -> Result<FrameStatus, Box<dyn Error>> {
        let vic_result = self.cpu.mut_memory().mut_vic().tick()?;
        let cia1 = self.cpu.mut_memory().mut_cia1();
        let keyboard_scan_result = self.keyboard.scan(cia1.read_port(PortName::A));
        cia1.write_port(PortName::B, keyboard_scan_result);
        if self.at_cpu_cycle() {
            self.cpu.tick()?;
            self.cia1_irq = self.cpu.mut_memory().mut_cia1().tick();
            self.cia2_irq = self.cpu.mut_memory().mut_cia2().tick();
        }
        self.cpu
            .set_irq_pin(vic_result.irq | self.cia1_irq | self.cia2_irq);
        self.cpu_clock_divider = (self.cpu_clock_divider + 1) % 8;
        return if self.frame_renderer.consume(vic_result.video_output) {
            Ok(FrameStatus::Complete)
        } else {
            Ok(FrameStatus::Pending)
        };
    }

    fn frame_image(&self) -> &RgbaImage {
        self.frame_renderer.frame_image()
    }

    fn display_state(&self) -> String {
        format!("{}\n{}", self.cpu(), self.cpu().memory())
    }
}

impl MachineInspector for C64 {
    delegate! {
        to self.cpu {
            fn reg_pc(&self) -> u16;
            fn reg_a(&self) -> u8;
            fn reg_x(&self) -> u8;
            fn reg_y(&self) -> u8;
            fn reg_sp(&self) -> u8;
            fn flags(&self) -> u8;
            fn inspect_memory(&self, address: u16) -> u8;
        }
    }

    fn at_instruction_start(&self) -> bool {
        self.at_cpu_cycle() && self.cpu.at_instruction_start()
    }
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

            cpu_clock_divider: 0,
            cia1_irq: false,
            cia2_irq: false,

            keyboard: Keyboard::new(),
        })
    }

    fn at_cpu_cycle(&self) -> bool {
        self.cpu_clock_divider == 0
    }

    pub fn set_cartridge(&mut self, cartridge: Option<Cartridge>) {
        self.cpu.mut_memory().cartridge = cartridge;
    }

    pub fn set_key_state(&mut self, key: Key, state: KeyState) {
        self.keyboard.set_key_state(key, state);
    }

    pub fn cpu(&self) -> &Cpu<C64AddressSpace> {
        &self.cpu
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::c64_with_cartridge;
    use crate::test_utils::c64_with_cartridge_uninitialized;
    use crate::test_utils::next_frame;
    use common::test_utils::read_test_image;
    use image::DynamicImage;

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

    #[test]
    fn shows_hello_world() {
        // Note: Once 6502 runs with its actual speed, we'll probably need to wait for a frame or two.
        let mut c64 = c64_with_cartridge("hello_world.bin");
        next_frame(&mut c64).unwrap(); // Allow 1 frame for initialization.
        assert_produces_frame(&mut c64, "hello_world.png", "shows_hello_world");
    }

    #[test]
    fn interrupts() {
        let mut c64 = c64_with_cartridge("interrupts.bin");
        next_frame(&mut c64).unwrap(); // Allow 1 frame for initialization.
        assert_produces_frame(&mut c64, "interrupts_1.png", "interrupts_1");
        assert_produces_frame(&mut c64, "interrupts_2.png", "interrupts_2");
        assert_produces_frame(&mut c64, "interrupts_3.png", "interrupts_3");
    }

    #[test]
    fn chip_timing() {
        let mut c64 = c64_with_cartridge("chip_timing.bin");
        // Allow 3 frames for initialization.
        next_frame(&mut c64).unwrap();
        next_frame(&mut c64).unwrap();
        next_frame(&mut c64).unwrap();
        assert_produces_frame(&mut c64, "chip_timing.png", "chip_timing");
    }

    #[test]
    fn next_instruction_detection() {
        // Make sure that we only report it once per machine cycle.
        let mut c64 = c64_with_cartridge_uninitialized("hello_world.bin");
        while !c64.at_instruction_start() {
            c64.tick().unwrap();
        }
        c64.tick().unwrap();
        assert!(!c64.at_instruction_start());
    }

    #[test]
    fn keyboard() {
        let mut c64 = c64_with_cartridge("keyboard.bin");
        next_frame(&mut c64).unwrap();
        next_frame(&mut c64).unwrap();
        assert_produces_frame(&mut c64, "c64_keyboard_1.png", "c64_keyboard_1");

        c64.set_key_state(Key::C, KeyState::Pressed);
        next_frame(&mut c64).unwrap();
        assert_produces_frame(&mut c64, "c64_keyboard_2.png", "c64_keyboard_2");

        c64.set_key_state(Key::C, KeyState::Released);
        c64.set_key_state(Key::D6, KeyState::Pressed);
        next_frame(&mut c64).unwrap();
        assert_produces_frame(&mut c64, "c64_keyboard_3.png", "c64_keyboard_3");

        c64.set_key_state(Key::D6, KeyState::Released);
        c64.set_key_state(Key::D4, KeyState::Pressed);
        next_frame(&mut c64).unwrap();
        assert_produces_frame(&mut c64, "c64_keyboard_4.png", "c64_keyboard_4");
    }
}
