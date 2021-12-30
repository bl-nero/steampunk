use crate::c64::C64;
use common::app::AppController;
use common::app::MachineController;
use image::RgbaImage;
use piston::Event;
use piston::Loop;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct C64Controller<'a> {
    machine_controller: MachineController<'a, C64>,
}

impl<'a> C64Controller<'a> {
    pub fn new(c64: &'a mut C64) -> Self {
        Self {
            machine_controller: MachineController::new(c64),
        }
    }
}

impl<'a> AppController for C64Controller<'a> {
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
            Event::Loop(Loop::Update(_)) => self.machine_controller.run_until_end_of_frame(),
            _ => {}
        }
    }

    fn display_machine_state(&self) -> String {
        self.machine_controller.display_state()
    }
}

#[cfg(test)]
mod tests {}
