use image::RgbaImage;
use piston_window::{
    Button, ButtonState, Event, EventLoop, Filter, G2d, G2dTexture, G2dTextureContext, GfxDevice,
    Input, Key, Loop, PistonWindow, Texture, TextureSettings, WindowSettings,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::atari::{Atari, FrameStatus, JoystickInput, JoystickPort, Switch, SwitchPosition};

pub struct Application<'a> {
    window: PistonWindow,
    controller: Controller<'a>,
    view: View,
}

impl<'a> Application<'a> {
    /// Creates an Atari emulator application that runs given virtual Atari
    /// device.
    pub fn new(atari: &'a mut Atari) -> Self {
        let window_width = atari.frame_image().width() * 5;
        let window_height = atari.frame_image().height() * 3;
        let window_settings =
            WindowSettings::new("Atari 2600", [window_width, window_height]).exit_on_esc(true);
        let mut window: PistonWindow = window_settings.build().expect("Could not build a window");
        window.set_ups(60);
        let texture_context = window.create_texture_context();
        let view = View::new(texture_context, &atari);
        let interrupted = Arc::new(AtomicBool::new(false));

        Self {
            window,
            view,
            controller: Controller::new(atari, interrupted),
        }
    }

    /// Starts Atari and runs the event loop until the user decides to quit.
    pub fn run(&mut self) {
        self.controller.reset();
        while let Some(e) = self.window.next() {
            self.controller.event(&e);
            let view = &mut self.view;
            let frame_image = self.controller.frame_image();
            self.window.draw_2d(&e, |ctx, graphics, device| {
                view.draw(frame_image, ctx, graphics, device);
            });
            self.window.event(&e);
            if self.controller.interrupted.load(Ordering::Relaxed) {
                eprintln!("Interrupted!");
                eprintln!("{}", self.controller.atari.cpu());
                eprintln!("{}", self.controller.atari.cpu().memory());
                return;
            }
        }
    }

    /// Exposes a pointer to a thread-safe interruption flag. Once it's set to
    /// `true`, the main event loop finishes, allowing the program to quit
    /// gracefully.
    pub fn interrupted(&self) -> Arc<AtomicBool> {
        return self.controller.interrupted.clone();
    }
}

struct Controller<'a> {
    atari: &'a mut Atari,
    running: bool,
    interrupted: Arc<AtomicBool>,
}

impl<'a> Controller<'a> {
    fn new(atari: &'a mut Atari, interrupted: Arc<AtomicBool>) -> Self {
        return Controller {
            atari,
            running: false,
            interrupted,
        };
    }

    fn frame_image(&self) -> &RgbaImage {
        self.atari.frame_image()
    }

    fn reset(&mut self) {
        self.atari.reset().expect("Unable to reset Atari");
        self.running = true;
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
                    self.atari
                        .flip_switch(switch, !self.atari.switch_position(switch));
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
                    self.atari.flip_switch(
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
                    self.atari
                        .set_joystick_input_state(port, input, *state == ButtonState::Press);
                };
            }
            Event::Loop(Loop::Update(_)) => {
                if let Err(e) = self.run_until_end_of_frame() {
                    self.running = false;
                    eprintln!("ERROR: {}. Atari halted.", e);
                    eprintln!("{}", self.atari.cpu());
                    eprintln!("{}", self.atari.cpu().memory());
                };
            }
            _ => {}
        }
    }

    fn run_until_end_of_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        while self.running && !self.interrupted.load(Ordering::Relaxed) {
            match self.atari.tick() {
                Ok(FrameStatus::Pending) => {}
                Ok(FrameStatus::Complete) => return Ok(()),
                Err(e) => return Err(e),
            }
        }
        return Ok(());
    }
}

struct View {
    texture_context: G2dTextureContext,
    texture: G2dTexture,
}

impl View {
    fn new(mut texture_context: G2dTextureContext, atari: &Atari) -> Self {
        let texture_settings = TextureSettings::new().mag(Filter::Nearest);
        let texture =
            Texture::from_image(&mut texture_context, atari.frame_image(), &texture_settings)
                .expect("Could not create a texture");
        return Self {
            texture_context,
            texture,
        };
    }

    fn draw(
        &mut self,
        frame_image: &RgbaImage,
        ctx: piston_window::Context,
        g: &mut G2d,
        device: &mut GfxDevice,
    ) {
        let texture_context = &mut self.texture_context;
        let texture = &mut self.texture;
        let frame_image = frame_image;
        texture
            .update(texture_context, frame_image)
            .expect("Unable to update texture");
        graphics::clear([0.0, 0.0, 0.0, 1.0], g);
        let view_size = ctx.get_view_size();
        graphics::Image::new()
            .rect([0.0, 0.0, view_size[0], view_size[1]])
            .draw(texture, &ctx.draw_state, ctx.transform, g);
        texture_context.encoder.flush(device);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::assert_images_equal;
    use crate::test_utils::atari_with_rom;
    use crate::test_utils::read_test_image;
    use image::DynamicImage;
    use piston_window::ButtonArgs;
    use piston_window::UpdateArgs;

    fn assert_current_frame(controller: &mut Controller, test_image_name: &str, test_name: &str) {
        let actual_image = DynamicImage::ImageRgba8(controller.frame_image().clone());
        let expected_image = read_test_image(test_image_name);
        assert_images_equal(actual_image, expected_image, test_name);
    }

    #[test]
    fn controller_produces_images_until_interrupted() {
        let mut atari = atari_with_rom("horizontal_stripes_animated.bin");
        let mut controller = Controller::new(&mut atari, Arc::new(AtomicBool::new(false)));
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

        controller.interrupted.store(true, Ordering::Relaxed);
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        assert_current_frame(
            &mut controller,
            "horizontal_stripes_2.png",
            "controller_produces_image_until_interrupted_3",
        );
    }

    fn send_key(controller: &mut Controller, key: Key, state: ButtonState) {
        controller.event(&Event::from(ButtonArgs {
            button: Button::Keyboard(key),
            state,
            scancode: None,
        }));
    }

    #[test]
    fn console_switches() {
        let mut atari = atari_with_rom("io_monitor.bin");
        let mut controller = Controller::new(&mut atari, Arc::new(AtomicBool::new(false)));
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
        let mut controller = Controller::new(&mut atari, Arc::new(AtomicBool::new(false)));
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
