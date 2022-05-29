use crate::c64::C64;
use crate::keyboard::Key as C64Key;
use crate::keyboard::KeyState;
use common::app::AppController;
use common::app::MachineController;
use common::debugger::adapter::DebugAdapter;
use common::debugger::Debugger;
use image::RgbaImage;
use piston::Button;
use piston::ButtonArgs;
use piston::ButtonState;
use piston::Event;
use piston::Input;
use piston::Key;
use piston::Loop;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct C64Controller<'a, A: DebugAdapter> {
    machine_controller: MachineController<'a, C64, A>,
}

impl<'a, A: DebugAdapter> C64Controller<'a, A> {
    pub fn new(c64: &'a mut C64, debugger_adapter: Option<A>) -> Self {
        let debugger = debugger_adapter.map(Debugger::new);
        Self {
            machine_controller: MachineController::new(c64, debugger),
        }
    }
}

impl<'a, A: DebugAdapter> AppController for C64Controller<'a, A> {
    fn frame_image(&self) -> &RgbaImage {
        self.machine_controller.frame_image()
    }

    fn reset(&mut self) {
        self.machine_controller.reset();
    }

    fn interrupted(&self) -> Arc<AtomicBool> {
        self.machine_controller.interrupted()
    }

    fn event(&mut self, event: &Event) {
        match event {
            Event::Input(
                Input::Button(ButtonArgs {
                    button: Button::Keyboard(key),
                    state,
                    ..
                }),
                _timestamp,
            ) => {
                // println!("Key {:?}, state {:?}", key, state);
                if let Some(c64_key) = map_key(*key) {
                    let c64_key_state = match state {
                        ButtonState::Press => KeyState::Pressed,
                        ButtonState::Release => KeyState::Released,
                    };
                    self.machine_controller
                        .mut_machine()
                        .set_key_state(c64_key, c64_key_state);
                }
            }
            Event::Loop(Loop::Update(_)) => self.machine_controller.run_until_end_of_frame(),
            _ => {}
        }
    }

    fn display_machine_state(&self) -> String {
        self.machine_controller.display_state()
    }
}

fn map_key(key: Key) -> Option<C64Key> {
    match key {
        Key::Backquote => Some(C64Key::LeftArrow),
        Key::D1 => Some(C64Key::D1),
        Key::D2 => Some(C64Key::D2),
        Key::D3 => Some(C64Key::D3),
        Key::D4 => Some(C64Key::D4),
        Key::D5 => Some(C64Key::D5),
        Key::D6 => Some(C64Key::D6),
        Key::D7 => Some(C64Key::D7),
        Key::D8 => Some(C64Key::D8),
        Key::D9 => Some(C64Key::D9),
        Key::D0 => Some(C64Key::D0),
        Key::Minus => Some(C64Key::Plus),
        Key::Equals => Some(C64Key::Minus),
        // Key::Pound => Some(C64Key::Pound),
        Key::Home => Some(C64Key::ClrHome),
        Key::Backspace => Some(C64Key::InstDel),

        Key::Tab => Some(C64Key::Ctrl),
        Key::Q => Some(C64Key::Q),
        Key::W => Some(C64Key::W),
        Key::E => Some(C64Key::E),
        Key::R => Some(C64Key::R),
        Key::T => Some(C64Key::T),
        Key::Y => Some(C64Key::Y),
        Key::U => Some(C64Key::U),
        Key::I => Some(C64Key::I),
        Key::O => Some(C64Key::O),
        Key::P => Some(C64Key::P),
        Key::LeftBracket => Some(C64Key::At),
        Key::RightBracket => Some(C64Key::Asterisk),
        // Key::UpArrow => Some(C64Key::UpArrow),
        Key::F12 => Some(C64Key::Restore),

        Key::Escape => Some(C64Key::RunStop),
        // Key::ShiftLock => Some(C64Key::ShiftLock),
        Key::A => Some(C64Key::A),
        Key::S => Some(C64Key::S),
        Key::D => Some(C64Key::D),
        Key::F => Some(C64Key::F),
        Key::G => Some(C64Key::G),
        Key::H => Some(C64Key::H),
        Key::J => Some(C64Key::J),
        Key::K => Some(C64Key::K),
        Key::L => Some(C64Key::L),
        Key::Semicolon => Some(C64Key::Colon),
        Key::Quote => Some(C64Key::Semicolon),
        Key::Backslash => Some(C64Key::Equals),
        Key::Return => Some(C64Key::Return),

        Key::LCtrl => Some(C64Key::Commodore),
        Key::LShift => Some(C64Key::LShift),
        Key::Z => Some(C64Key::Z),
        Key::X => Some(C64Key::X),
        Key::C => Some(C64Key::C),
        Key::V => Some(C64Key::V),
        Key::B => Some(C64Key::B),
        Key::N => Some(C64Key::N),
        Key::M => Some(C64Key::M),
        Key::Comma => Some(C64Key::Comma),
        Key::Period => Some(C64Key::Period),
        Key::Slash => Some(C64Key::Slash),
        Key::RShift => Some(C64Key::RShift),
        Key::Down => Some(C64Key::CrsrUpDown),
        Key::Right => Some(C64Key::CrsrLeftRight),

        Key::Space => Some(C64Key::Space),

        Key::F1 => Some(C64Key::F1),
        Key::F3 => Some(C64Key::F3),
        Key::F5 => Some(C64Key::F5),
        Key::F7 => Some(C64Key::F7),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::assert_current_frame;
    use common::debugger::adapter::TcpDebugAdapter;
    use piston::UpdateArgs;

    use crate::test_utils::c64_with_cartridge;
    use piston::Button;
    use piston::ButtonState;

    fn send_key<A>(controller: &mut C64Controller<A>, key: Key, state: ButtonState)
    where
        A: DebugAdapter,
    {
        controller.event(&Event::from(ButtonArgs {
            button: Button::Keyboard(key),
            state,
            scancode: None,
        }));
    }

    #[test]
    fn keyboard() {
        let mut c64 = c64_with_cartridge("keyboard.bin");
        let mut controller = C64Controller::new(&mut c64, None::<TcpDebugAdapter>);
        controller.reset();
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        assert_current_frame(&mut controller, "app_keyboard_1.png", "app_keyboard_1");

        send_key(&mut controller, Key::C, ButtonState::Press);
        controller.event(&Event::from(UpdateArgs { dt: 1.0 / 60.0 }));
        assert_current_frame(&mut controller, "app_keyboard_1.png", "app_keyboard_2");
    }
}
