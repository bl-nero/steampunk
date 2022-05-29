#![feature(test)]

mod address_space;
mod app;
mod c64;
mod cia;
mod frame_renderer;
mod keyboard;
mod port;
mod sid;
mod timer;
mod vic;

mod test_utils;

use crate::address_space::Cartridge;
use crate::address_space::CartridgeMode;
use crate::app::C64Controller;
use crate::c64::C64;
use clap::Parser;
use common::app::Application;
use common::app::CommonCliArguments;
use common::debugger::adapter::TcpDebugAdapter;
use std::sync::atomic::Ordering;
use vic::Vic;
use ya6502::memory::Rom;

#[derive(Parser)]
struct Args {
    #[clap(flatten)]
    common: CommonCliArguments,
    cartridge_file: Option<String>,
}

fn main() {
    let args = Args::parse();

    let mut c64 = C64::new().expect("Unable to initialize C64");

    // Load the cartridge ROM image, if specified. So far, only Ultimax mode is
    // supported.
    if let Some(file) = args.cartridge_file {
        let cartridge_bytes = std::fs::read(file).expect("Unable to read the cartridge file");
        c64.set_cartridge(Some(Cartridge {
            mode: CartridgeMode::Ultimax,
            rom: Rom::new(&cartridge_bytes).expect("Unable to create ROM cartridge"),
        }));
    }

    let debugger_adapter = if args.common.debugger {
        Some(TcpDebugAdapter::new(args.common.debugger_port))
    } else {
        None
    };

    let mut app = Application::new(
        C64Controller::new(&mut c64, debugger_adapter),
        "Commodore 64",
        2,
        2,
    );

    let interrupted = app.interrupted();

    ctrlc::set_handler(move || {
        eprintln!("Terminating.");
        interrupted.store(true, Ordering::Relaxed);
    })
    .expect("Unable to set interrupt signal handler");

    app.run();
}
