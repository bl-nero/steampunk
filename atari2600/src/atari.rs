use crate::address_space::AddressSpace;
use crate::audio::AudioConsumer;
use crate::frame_renderer::FrameRenderer;
use crate::riot;
use crate::riot::Riot;
use crate::tia;
use crate::tia::Tia;
use enum_map::{enum_map, Enum, EnumMap};
use image;
use image::RgbaImage;
use std::error;
use ya6502::cpu::Cpu;
use ya6502::memory::{AtariRam, AtariRom};

pub type AtariAddressSpace = AddressSpace<Tia, AtariRam, Riot, AtariRom>;

pub struct Atari {
    cpu: Cpu<AtariAddressSpace>,
    frame_renderer: FrameRenderer,
    audio_consumer: AudioConsumer,
    switch_positions: EnumMap<Switch, SwitchPosition>,
    joysticks: EnumMap<JoystickPort, Joystick>,
}

pub enum FrameStatus {
    Pending,
    Complete,
}

impl Atari {
    pub fn new(
        address_space: Box<AtariAddressSpace>,
        frame_renderer: FrameRenderer,
        audio_consumer: AudioConsumer,
    ) -> Self {
        let mut atari = Atari {
            cpu: Cpu::new(address_space),
            frame_renderer,
            audio_consumer,
            switch_positions: enum_map! { _ => SwitchPosition::Up },
            joysticks: enum_map! { _ => Joystick::new() },
        };
        atari.update_switches_riot_port();
        atari.update_joystick_ports();
        return atari;
    }

    pub fn cpu(&self) -> &Cpu<AtariAddressSpace> {
        &self.cpu
    }

    fn mut_tia(&mut self) -> &mut Tia {
        return &mut self.cpu.mut_memory().tia;
    }

    fn mut_riot(&mut self) -> &mut Riot {
        return &mut self.cpu.mut_memory().riot;
    }

    /// Performs a single clock tick. If it resulted in an error reported by the
    /// CPU, dump debug information on standard error stream and return
    /// `TickResult::Error`.
    pub fn tick(&mut self) -> Result<FrameStatus, Box<dyn error::Error>> {
        let tia_result = self.mut_tia().tick();
        if tia_result.cpu_tick {
            if let Err(e) = self.cpu.tick() {
                return Err(e);
            }
        }
        if tia_result.riot_tick {
            self.mut_riot().tick();
        }
        if let Some(audio) = tia_result.audio {
            self.audio_consumer
                .consume((audio.au0 + audio.au1) as f32 / 30.0 - 0.5);
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
        self.switch_positions[switch]
    }

    pub fn flip_switch(&mut self, switch: Switch, position: SwitchPosition) {
        self.switch_positions[switch] = position;
        self.update_switches_riot_port();
    }

    fn update_switches_riot_port(&mut self) {
        let port_value = self
            .switch_positions
            .iter()
            .map(|(switch, pos)| switch.port_value_when(*pos))
            .fold(0b0011_0100, |acc, item| acc | item);
        self.mut_riot().set_port(riot::Port::PB, port_value);
    }

    pub fn set_joystick_input_state(
        &mut self,
        port: JoystickPort,
        input: JoystickInput,
        state: bool,
    ) {
        self.joysticks[port].set_state(input, state);
        self.update_joystick_ports();
    }

    fn update_joystick_ports(&mut self) {
        let (left_dir_port, left_fire_port) = self.joysticks[JoystickPort::Left].port_values();
        let (right_dir_port, right_fire_port) = self.joysticks[JoystickPort::Right].port_values();
        self.mut_riot()
            .set_port(riot::Port::PA, (left_dir_port << 4) | right_dir_port);
        self.mut_tia().set_port(tia::Port::Input4, left_fire_port);
        self.mut_tia().set_port(tia::Port::Input5, right_fire_port);
    }
}

#[derive(Debug, Copy, Clone, Enum)]
pub enum Switch {
    TvType,
    LeftDifficulty,
    RightDifficulty,
    GameSelect,
    GameReset,
}

impl Switch {
    fn port_value_when(&self, position: SwitchPosition) -> u8 {
        match position {
            SwitchPosition::Down => 0,
            SwitchPosition::Up => match self {
                Self::RightDifficulty => 1 << 7,
                Self::LeftDifficulty => 1 << 6,
                Self::TvType => 1 << 3,
                Self::GameSelect => 1 << 1,
                Self::GameReset => 1,
            },
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SwitchPosition {
    Up,
    Down,
}

impl std::ops::Not for SwitchPosition {
    type Output = SwitchPosition;
    fn not(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }
}

#[derive(Enum)]
pub enum JoystickInput {
    Up,
    Down,
    Left,
    Right,
    Fire,
}

impl JoystickInput {
    fn port_mask(&self) -> u8 {
        match *self {
            Self::Up => 1,
            Self::Down => 1 << 1,
            Self::Left => 1 << 2,
            Self::Right => 1 << 3,
            Self::Fire => 0,
        }
    }
    fn opposite(&self) -> Self {
        match *self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Fire => Self::Fire,
        }
    }
}

struct Joystick {
    direction_port: u8,
    fire_port: bool,
}

impl Joystick {
    fn new() -> Self {
        Joystick {
            direction_port: 0b1111,
            fire_port: true,
        }
    }

    fn set_state(&mut self, input: JoystickInput, state: bool) {
        match input {
            JoystickInput::Fire => self.fire_port = !state,
            _ => {
                if state {
                    self.direction_port &= !input.port_mask();
                    self.direction_port |= input.opposite().port_mask();
                } else {
                    self.direction_port |= input.port_mask();
                }
            }
        };
    }

    fn port_values(&self) -> (u8, bool) {
        (self.direction_port, self.fire_port)
    }
}

#[derive(Enum)]
pub enum JoystickPort {
    Left,
    Right,
}

#[cfg(test)]
mod tests {
    extern crate test;

    use super::*;
    use crate::audio::create_consumer_and_source;
    use crate::colors;
    use crate::frame_renderer::FrameRendererBuilder;
    use crate::test_utils::assert_images_equal;
    use crate::test_utils::atari_with_rom;
    use crate::test_utils::read_test_image;
    use crate::test_utils::read_test_rom;
    use image::DynamicImage;
    use test::Bencher;
    use ya6502::cpu::{opcodes, CpuHaltedError};

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
    fn sprite_timing() {
        let mut atari = atari_with_rom("sprite_timing.bin");
        assert_produces_frame(&mut atari, "sprite_timing.png", "sprite_timing");
    }

    #[test]
    fn missile_alignment() {
        let mut atari = atari_with_rom("missile_alignment.bin");
        assert_produces_frame(&mut atari, "missile_alignment.png", "missile_alignment");
    }

    #[test]
    fn input() {
        let mut atari = atari_with_rom("io_monitor.bin");
        assert_eq!(
            atari.switch_position(Switch::RightDifficulty),
            SwitchPosition::Up
        );
        assert_produces_frame(&mut atari, "input_1.png", "input_1");

        atari.flip_switch(Switch::RightDifficulty, SwitchPosition::Down);
        atari.flip_switch(Switch::LeftDifficulty, SwitchPosition::Down);
        atari.flip_switch(Switch::TvType, SwitchPosition::Down);
        atari.set_joystick_input_state(JoystickPort::Left, JoystickInput::Up, true);
        atari.set_joystick_input_state(JoystickPort::Left, JoystickInput::Right, true);
        atari.set_joystick_input_state(JoystickPort::Right, JoystickInput::Down, true);
        atari.set_joystick_input_state(JoystickPort::Right, JoystickInput::Right, true);
        atari.set_joystick_input_state(JoystickPort::Right, JoystickInput::Fire, true);
        assert_eq!(
            atari.switch_position(Switch::RightDifficulty),
            SwitchPosition::Down
        );
        assert_produces_frame(&mut atari, "input_2.png", "input_2");

        atari.flip_switch(Switch::TvType, SwitchPosition::Up);
        atari.flip_switch(Switch::GameSelect, SwitchPosition::Down);
        atari.set_joystick_input_state(JoystickPort::Left, JoystickInput::Up, false);
        atari.set_joystick_input_state(JoystickPort::Left, JoystickInput::Right, false);
        atari.set_joystick_input_state(JoystickPort::Left, JoystickInput::Down, true);
        atari.set_joystick_input_state(JoystickPort::Left, JoystickInput::Left, true);
        atari.set_joystick_input_state(JoystickPort::Left, JoystickInput::Fire, true);
        atari.set_joystick_input_state(JoystickPort::Right, JoystickInput::Down, false);
        atari.set_joystick_input_state(JoystickPort::Right, JoystickInput::Right, false);
        atari.set_joystick_input_state(JoystickPort::Right, JoystickInput::Up, true);
        atari.set_joystick_input_state(JoystickPort::Right, JoystickInput::Left, true);
        atari.set_joystick_input_state(JoystickPort::Right, JoystickInput::Fire, false);
        assert_produces_frame(&mut atari, "input_3.png", "input_3");

        atari.flip_switch(Switch::LeftDifficulty, SwitchPosition::Up);
        atari.flip_switch(Switch::GameReset, SwitchPosition::Down);
        atari.set_joystick_input_state(JoystickPort::Left, JoystickInput::Down, false);
        atari.set_joystick_input_state(JoystickPort::Left, JoystickInput::Left, false);
        atari.set_joystick_input_state(JoystickPort::Left, JoystickInput::Fire, false);
        atari.set_joystick_input_state(JoystickPort::Right, JoystickInput::Up, false);
        atari.set_joystick_input_state(JoystickPort::Right, JoystickInput::Left, false);
        assert_produces_frame(&mut atari, "input_4.png", "input_4");

        atari.flip_switch(Switch::RightDifficulty, SwitchPosition::Up);
        assert_eq!(
            atari.switch_position(Switch::RightDifficulty),
            SwitchPosition::Up
        );
        assert_produces_frame(&mut atari, "input_5.png", "input_5");
    }

    #[test]
    fn joystick_single_buttons() {
        let mut joystick = Joystick::new();
        assert_eq!(joystick.port_values(), (0b1111, true));
        joystick.set_state(JoystickInput::Up, true);
        assert_eq!(joystick.port_values(), (0b1110, true));
        joystick.set_state(JoystickInput::Up, false);
        joystick.set_state(JoystickInput::Down, true);
        assert_eq!(joystick.port_values(), (0b1101, true));
        joystick.set_state(JoystickInput::Down, false);
        joystick.set_state(JoystickInput::Left, true);
        assert_eq!(joystick.port_values(), (0b1011, true));
        joystick.set_state(JoystickInput::Left, false);
        joystick.set_state(JoystickInput::Right, true);
        assert_eq!(joystick.port_values(), (0b0111, true));
        joystick.set_state(JoystickInput::Right, false);
        assert_eq!(joystick.port_values(), (0b1111, true));
        joystick.set_state(JoystickInput::Fire, true);
        assert_eq!(joystick.port_values(), (0b1111, false));
        joystick.set_state(JoystickInput::Fire, false);
        assert_eq!(joystick.port_values(), (0b1111, true));
    }

    #[test]
    fn joystick_button_combinations() {
        let mut joystick = Joystick::new();
        joystick.set_state(JoystickInput::Up, true);
        joystick.set_state(JoystickInput::Left, true);
        assert_eq!(joystick.port_values(), (0b1010, true));
        joystick.set_state(JoystickInput::Up, false);
        joystick.set_state(JoystickInput::Left, false);
        joystick.set_state(JoystickInput::Right, true);
        joystick.set_state(JoystickInput::Down, true);
        assert_eq!(joystick.port_values(), (0b0101, true));
    }

    #[test]
    fn joystick_forbidden_combinations() {
        let mut joystick = Joystick::new();
        joystick.set_state(JoystickInput::Up, true);
        joystick.set_state(JoystickInput::Left, true);
        joystick.set_state(JoystickInput::Down, true);
        assert_eq!(joystick.port_values(), (0b1001, true));
        joystick.set_state(JoystickInput::Right, true);
        assert_eq!(joystick.port_values(), (0b0101, true));
        joystick.set_state(JoystickInput::Up, true);
        assert_eq!(joystick.port_values(), (0b0110, true));
        joystick.set_state(JoystickInput::Left, true);
        assert_eq!(joystick.port_values(), (0b1010, true));
    }

    #[test]
    fn sprites() {
        let mut atari = atari_with_rom("sprites.bin");
        assert_produces_frame(&mut atari, "sprites_1.png", "sprites_1");
        assert_produces_frame(&mut atari, "sprites_2.png", "sprites_2");
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
            let (consumer, _) = create_consumer_and_source();
            let mut atari = Atari::new(
                address_space,
                FrameRendererBuilder::new()
                    .with_palette(colors::ntsc_palette())
                    .build(),
                consumer,
            );

            atari.reset().unwrap();
            next_frame(&mut atari).unwrap();
        });
    }
}
