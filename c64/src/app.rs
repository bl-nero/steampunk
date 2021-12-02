use crate::vic::RASTER_LENGTH;
use crate::vic::TOTAL_HEIGHT;
use crate::C64;
use common::app::Controller;
use image::RgbaImage;
use piston::Event;
use piston::Loop;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct C64Controller {
    c64: C64,
    running: bool,
    interrupted: Arc<AtomicBool>,
}

impl C64Controller {
    pub fn new(c64: C64) -> Self {
        Self {
            c64,
            running: false,
            interrupted: Arc::new(AtomicBool::new(false)),
        }
    }

    fn run_frame(&mut self) {
        if !self.running {
            return;
        }
        for _ in 0..RASTER_LENGTH * TOTAL_HEIGHT {
            if let Err(e) = self.c64.tick() {
                eprintln!("ERROR: {}. C64 halted.", e);
                eprintln!("{}", self.display_machine_state());
                self.running = false;
                return;
            }
        }
    }
}

impl Controller for C64Controller {
    fn frame_image(&self) -> &RgbaImage {
        self.c64.frame_image()
    }
    fn reset(&mut self) {
        self.c64.reset();
        self.running = true;
    }
    fn interrupted(&self) -> Arc<AtomicBool> {
        self.interrupted.clone()
    }
    fn event(&mut self, event: &Event) {
        match event {
            Event::Loop(Loop::Update(_)) => {
                self.run_frame();
            }
            _ => {}
        }
    }
    fn display_machine_state(&self) -> String {
        format!("{}\n{}", self.c64.cpu(), self.c64.cpu().memory())
    }
}

#[cfg(test)]
mod tests {}
