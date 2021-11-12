#![feature(test)]

mod address_space;
mod app;
mod atari;
mod audio;
mod colors;
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
    // Load the ROM image.
    if args.len() < 2 {
        eprintln!("Usage: atari2600 <ROM_file>");
        eprintln!("No ROM file given, exiting.");
        return;
    }
    let rom_bytes = std::fs::read(&args[1]).expect("Unable to read the ROM image file");
    // Create and initialize components of the emulated system.
    let address_space = Box::new(AtariAddressSpace {
        tia: Tia::new(),
        ram: AtariRam::new(),
        riot: Riot::new(),
        rom: AtariRom::new(&rom_bytes[..]).expect("Unable to load the ROM into Atari"),
    });
    let (audio_consumer, stream, _sink) = audio::initialize();
    let mut atari = Atari::new(
        address_space,
        FrameRendererBuilder::new()
            .with_palette(colors::ntsc_palette())
            .with_height(210)
            .build(),
        audio_consumer,
    );

    let mut app = Application::new(&mut atari);
    let interrupted = app.interrupted();

    ctrlc::set_handler(move || {
        eprintln!("Terminating.");
        interrupted.store(true, Ordering::Relaxed);
    })
    .expect("Unable to set interrupt signal handler");

    app.run();

    // Note: The order of dropping is important here, hence we make it explicit.
    // If we drop Atari before the audio stream, we'll end up with a potential
    // deadlock: the audio stream may not finish until a blocking read of the
    // audio sample is performed, and it won't be interrupted unless we "hang
    // up" on the writing side (the AudioConsumer), which owns an
    // mspc::SyncSender instance. Since the audio consumer is owned by Atari, we
    // need to drop it first.
    drop(atari);
    drop(stream);
}
