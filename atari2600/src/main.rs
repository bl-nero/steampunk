#![feature(test)]

mod address_space;
mod app;
mod atari;
mod colors;
mod delay_buffer;
mod frame_renderer;
mod riot;
mod tia;

mod test_utils;

use app::Application;
use atari::{Atari, AtariAddressSpace};
use frame_renderer::FrameRendererBuilder;
use riot::Riot;
use std::env;
use std::sync::atomic::Ordering;
use tia::Tia;
use ya6502::memory::{AtariRam, AtariRom};

fn main() {
    println!("Ready player ONE!");

    let args: Vec<String> = env::args().collect();
    // Load an example ROM image.
    let rom_bytes = std::fs::read(&args[1]).expect("Unable to read the ROM image file");
    // Create and initialize components of the emulated system.
    let address_space = Box::new(AtariAddressSpace {
        tia: Tia::new(),
        ram: AtariRam::new(),
        riot: Riot::new(),
        rom: AtariRom::new(&rom_bytes[..]).expect("Unable to load the ROM into Atari"),
    });
    let mut atari = Atari::new(
        address_space,
        FrameRendererBuilder::new()
            .with_palette(colors::ntsc_palette())
            .with_height(210)
            .build(),
    );

    let mut app = Application::new(&mut atari);
    let interrupted = app.interrupted();

    ctrlc::set_handler(move || {
        eprintln!("Terminating.");
        interrupted.store(true, Ordering::Relaxed);
    })
    .expect("Unable to set interrupt signal handler");

    app.run();
}
