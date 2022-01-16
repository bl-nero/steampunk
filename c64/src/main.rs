#![feature(test)]

mod address_space;
mod app;
mod c64;
mod cia;
mod frame_renderer;
mod port;
mod sid;
mod timer;
mod vic;

use crate::address_space::Cartridge;
use crate::address_space::CartridgeMode;
use crate::app::C64Controller;
use crate::c64::C64;
use common::app::Application;
use common::debugger::DebugAdapter;
use std::env;
use std::sync::atomic::Ordering;
use vic::Vic;
use ya6502::memory::Rom;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut c64 = C64::new().expect("Unable to initialize C64");

    // Load the cartridge ROM image, if specified. So far, only Ultimax mode is
    // supported.
    if args.len() >= 2 {
        let cartridge_bytes = std::fs::read(&args[1]).expect("Unable to read the cartridge file");
        c64.set_cartridge(Some(Cartridge {
            mode: CartridgeMode::Ultimax,
            rom: Rom::new(&cartridge_bytes).expect("Unable to create ROM cartridge"),
        }));
    }

    let debugger_adapter = DebugAdapter::new(1234);

    let mut app = Application::new(
        C64Controller::new(&mut c64, Some(debugger_adapter)),
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
