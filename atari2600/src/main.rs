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

use crate::app::AtariController;
use atari::{Atari, AtariAddressSpace};
use clap::Parser;
use common::app::Application;
use common::app::CommonCliArguments;
use common::debugger::adapter::TcpDebugAdapter;
use frame_renderer::FrameRendererBuilder;
use std::sync::atomic::Ordering;
use ya6502::memory::Rom;

#[derive(Parser)]
struct Args {
    #[clap(flatten)]
    common: CommonCliArguments,
    cartridge_file: String,
}

fn main() {
    let args = Args::parse();

    println!("Ready player ONE!");

    let rom_bytes = std::fs::read(args.cartridge_file).expect("Unable to read the ROM image file");
    // Create and initialize components of the emulated system.
    let address_space = Box::new(AtariAddressSpace::new(
        Rom::new(&rom_bytes[..]).expect("Unable to load the ROM into Atari"),
    ));
    let (audio_consumer, stream, _sink) = audio::initialize();
    let mut atari = Atari::new(
        address_space,
        FrameRendererBuilder::new()
            .with_palette(colors::ntsc_palette())
            .with_height(210)
            .build(),
        audio_consumer,
    );

    let debugger_adapter = if args.common.debugger {
        Some(TcpDebugAdapter::new(args.common.debugger_port))
    } else {
        None
    };

    let mut app = Application::new(
        AtariController::new(&mut atari, debugger_adapter),
        "Atari 2600",
        5,
        3,
    );
    let interrupted = app.interrupted();

    signal_hook::flag::register(signal_hook::consts::SIGINT, interrupted)
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
