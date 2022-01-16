use common::app::AppController;
use common::app::MachineController;
use image::RgbaImage;
use piston_window::{Button, ButtonState, Event, Input, Key, Loop};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::atari::{Atari, JoystickInput, JoystickPort, Switch, SwitchPosition};

pub struct AtariController<'a> {
    machine_controller: MachineController<'a, Atari>,
}

impl<'a> AtariController<'a> {
    pub fn new(atari: &'a mut Atari) -> Self {
        return AtariController {
            machine_controller: MachineController::new(atari, None),
        };
    }

    fn mut_atari(&mut self) -> &mut Atari {
        self.machine_controller.mut_machine()
    }
}

impl<'a> AppController for AtariController<'a> {
    fn frame_image(&self) -> &RgbaImage {
        self.machine_controller.frame_image()
    }

    fn reset(&mut self) {
        self.machine_controller.reset()
    }

    fn interrupted(&self) -> Arc<AtomicBool> {
        self.machine_controller.interrupted()
    }

    fn display_machine_state(&self) -> String {
        self.machine_controller.display_state()
    }

    /// Handles Piston events.
    fn event(&mut self, event: &Event) {
        match event {
            Event::Input(
                Input::Button(piston_window::ButtonArgs {
                    state: ButtonState::Press,
                    button: Button::Keyboard(key @ (Key::D1 | Key::D2 | Key::D3)),
                    ..
                }),
                _timestamp,
            ) => {
                if let Some(switch) = match key {
                    Key::D1 => Some(Switch::TvType),
                    Key::D2 => Some(Switch::LeftDifficulty),
                    Key::D3 => Some(Switch::RightDifficulty),
                    _ => None,
                } {
                    let atari = self.mut_atari();
                    atari.flip_switch(switch, !atari.switch_position(switch));
                }
            }
            Event::Input(
                Input::Button(piston_window::ButtonArgs {
                    state,
                    button: Button::Keyboard(key @ (Key::D4 | Key::D5)),
                    ..
                }),
                _timestamp,
            ) => {
                if let Some(switch) = match key {
                    Key::D4 => Some(Switch::GameSelect),
                    Key::D5 => Some(Switch::GameReset),
                    _ => None,
                } {
                    self.machine_controller.mut_machine().flip_switch(
                        switch,
                        match state {
                            ButtonState::Press => SwitchPosition::Down,
                            ButtonState::Release => SwitchPosition::Up,
                        },
                    );
                }
            }
            Event::Input(
                Input::Button(piston_window::ButtonArgs {
                    state,
                    button: Button::Keyboard(key),
                    ..
                }),
                _timestamp,
            ) => {
                if let Some((port, input)) = match key {
                    Key::W => Some((JoystickPort::Left, JoystickInput::Up)),
                    Key::A => Some((JoystickPort::Left, JoystickInput::Left)),
                    Key::S => Some((JoystickPort::Left, JoystickInput::Down)),
                    Key::D => Some((JoystickPort::Left, JoystickInput::Right)),
                    Key::LShift | Key::Space => Some((JoystickPort::Left, JoystickInput::Fire)),

                    Key::I | Key::Up => Some((JoystickPort::Right, JoystickInput::Up)),
                    Key::J | Key::Left => Some((JoystickPort::Right, JoystickInput::Left)),
                    Key::K | Key::Down => Some((JoystickPort::Right, JoystickInput::Down)),
                    Key::L | Key::Right => Some((JoystickPort::Right, JoystickInput::Right)),
                    Key::N | Key::Period => Some((JoystickPort::Right, JoystickInput::Fire)),
                    _ => None,
                } {
                    self.machine_controller
                        .mut_machine()
                        .set_joystick_input_state(port, input, *state == ButtonState::Press);
                };
            }
            Event::Loop(Loop::Update(_)) => self.machine_controller.run_until_end_of_frame(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::assert_images_equal;
    use crate::test_utils::atari_with_rom;
    use common::test_utils::read_test_image;
    use image::DynamicImage;
    use piston_window::ButtonArgs;
    use piston_window::UpdateArgs;
    use std::sync::atomic::Ordering;

    fn assert_current_frame(
        controller: &mut AtariController,
        test_image_name: &str,
        test_name: &str,
    ) {
        let actual_image = DynamicImage::ImageRgba8(controller.frame_image().clone());
        let expected_image = read_test_image(test_image_name);
        assert_images_equal(actual_image, expected_image, test_name);
    }

    #[test]
    fn controller_produces_images_until_interrupted() {
        let mut atari = atari_with_rom("horizontal_stripes_animated.bin");
        let mut controller = AtariController::new(&mut atari);
        controller.reset();

        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        assert_current_frame(
            &mut controller,
            "horizontal_stripes_1.png",
            "controller_produces_image_until_interrupted_1",
        );

        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        assert_current_frame(
            &mut controller,
            "horizontal_stripes_2.png",
            "controller_produces_image_until_interrupted_2",
        );

        controller.interrupted().store(true, Ordering::Relaxed);
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        assert_current_frame(
            &mut controller,
            "horizontal_stripes_2.png",
            "controller_produces_image_until_interrupted_3",
        );
    }

    fn send_key(controller: &mut AtariController, key: Key, state: ButtonState) {
        controller.event(&Event::from(ButtonArgs {
            button: Button::Keyboard(key),
            state,
            scancode: None,
        }));
    }

    #[test]
    fn console_switches() {
        let mut atari = atari_with_rom("io_monitor.bin");
        let mut controller = AtariController::new(&mut atari);
        controller.reset();
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        assert_current_frame(
            &mut controller,
            "console_switches_1.png",
            "console_switches_1",
        );

        send_key(&mut controller, Key::D1, ButtonState::Press);
        send_key(&mut controller, Key::D2, ButtonState::Press);
        send_key(&mut controller, Key::D3, ButtonState::Press);
        send_key(&mut controller, Key::D4, ButtonState::Press);
        send_key(&mut controller, Key::D5, ButtonState::Press);
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        assert_current_frame(
            &mut controller,
            "console_switches_2.png",
            "console_switches_2",
        );

        send_key(&mut controller, Key::D1, ButtonState::Release);
        send_key(&mut controller, Key::D2, ButtonState::Release);
        send_key(&mut controller, Key::D3, ButtonState::Release);
        send_key(&mut controller, Key::D4, ButtonState::Release);
        send_key(&mut controller, Key::D5, ButtonState::Release);
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        assert_current_frame(
            &mut controller,
            "console_switches_3.png",
            "console_switches_3",
        );

        send_key(&mut controller, Key::D1, ButtonState::Press);
        send_key(&mut controller, Key::D2, ButtonState::Press);
        send_key(&mut controller, Key::D3, ButtonState::Press);
        send_key(&mut controller, Key::D4, ButtonState::Press);
        send_key(&mut controller, Key::D5, ButtonState::Press);
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        assert_current_frame(
            &mut controller,
            "console_switches_4.png",
            "console_switches_4",
        );

        send_key(&mut controller, Key::D1, ButtonState::Release);
        send_key(&mut controller, Key::D2, ButtonState::Release);
        send_key(&mut controller, Key::D3, ButtonState::Release);
        send_key(&mut controller, Key::D4, ButtonState::Release);
        send_key(&mut controller, Key::D5, ButtonState::Release);
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        assert_current_frame(
            &mut controller,
            "console_switches_1.png",
            "console_switches_5",
        );
    }

    #[test]
    fn joysticks() {
        let mut atari = atari_with_rom("io_monitor.bin");
        let mut controller = AtariController::new(&mut atari);
        controller.reset();
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));

        send_key(&mut controller, Key::I, ButtonState::Press);
        send_key(&mut controller, Key::J, ButtonState::Press);
        send_key(&mut controller, Key::N, ButtonState::Press);
        send_key(&mut controller, Key::S, ButtonState::Press);
        send_key(&mut controller, Key::D, ButtonState::Press);
        send_key(&mut controller, Key::LShift, ButtonState::Press);
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        assert_current_frame(&mut controller, "joysticks_1.png", "joysticks_1");

        send_key(&mut controller, Key::K, ButtonState::Press);
        send_key(&mut controller, Key::L, ButtonState::Press);
        send_key(&mut controller, Key::N, ButtonState::Release);
        send_key(&mut controller, Key::A, ButtonState::Press);
        send_key(&mut controller, Key::W, ButtonState::Press);
        send_key(&mut controller, Key::LShift, ButtonState::Release);
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        assert_current_frame(&mut controller, "joysticks_2.png", "joysticks_2");
    }
}
