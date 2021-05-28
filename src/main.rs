#![feature(test)]
#![recursion_limit = "256"] // For assembly macros with long content

#[cfg(test)]
#[macro_use]
#[no_link]
extern crate rustasm6502;

mod address_space;
mod atari;
mod colors;
mod cpu;
mod frame_renderer;
mod memory;
mod riot;
mod tia;

mod test_utils;

use atari::{Atari, AtariAddressSpace, FrameStatus};
use image::RgbaImage;
use memory::{AtariRam, AtariRom};
use piston_window::WindowSettings;
use piston_window::{
    Button, ButtonState, Event, Filter, Input, Key, Loop, PistonWindow, Texture, TextureSettings,
    Window,
};
use riot::Riot;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tia::Tia;

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
    let mut atari = Atari::new(address_space);
    atari.reset().expect("Unable to reset Atari");

    let mut window = build_window(atari.frame_image());

    // Create a texture.
    let texture_settings = TextureSettings::new().mag(Filter::Nearest);
    let mut texture_context = window.create_texture_context();
    let mut texture =
        Texture::from_image(&mut texture_context, atari.frame_image(), &texture_settings)
            .expect("Could not create a texture");

    let interrupted = Arc::new(AtomicBool::new(false));
    {
        let interrupted = interrupted.clone();
        ctrlc::set_handler(move || {
            eprintln!("Terminating.");
            interrupted.store(true, Ordering::Relaxed);
        })
        .expect("Unable to set the Ctrl-C handler");
    }

    // Main loop.
    let mut running = true;
    while let Some(e) = window.next() {
        let window_size = window.size();
        match e {
            Event::Input(
                Input::Button(piston_window::ButtonArgs {
                    state: ButtonState::Press,
                    button: Button::Keyboard(key @ (Key::D1 | Key::D2 | Key::D3 | Key::D4)),
                    ..
                }),
                _timestamp,
            ) => {
                println!("A {:?} key was pressed!", key);
            }
            Event::Loop(Loop::Render(_)) => {
                // TODO: This code is a total mess. I need to clean it up.
                while running {
                    match atari.tick() {
                        Ok(FrameStatus::Pending) => {}
                        Ok(FrameStatus::Complete) => break,
                        Err(e) => {
                            running = false;
                            eprintln!("ERROR: {}. Atari halted.", e);
                            eprintln!("{}", atari.cpu());
                            eprintln!("{}", atari.cpu().memory());
                        }
                    }
                    if interrupted.load(Ordering::Relaxed) {
                        eprintln!("{}", atari.cpu());
                        eprintln!("{}", atari.cpu().memory());
                        std::process::exit(1);
                    }
                }
                if interrupted.load(Ordering::Relaxed) {
                    eprintln!("{}", atari.cpu());
                    eprintln!("{}", atari.cpu().memory());
                    std::process::exit(1);
                }
                window.draw_2d(&e, |ctx, g, device| {
                    let frame_image = atari.frame_image();
                    texture
                        .update(&mut texture_context, frame_image)
                        .expect("Unable to update texture");
                    graphics::clear([0.0, 0.0, 0.0, 1.0], g);
                    graphics::Image::new()
                        .rect([0.0, 0.0, window_size.width, window_size.height])
                        .draw(&texture, &ctx.draw_state, ctx.transform, g);
                    texture_context.encoder.flush(device);
                });
            }
            _ => {}
        };
        window.event(&e);
    }
}

fn build_window(frame_image: &RgbaImage) -> PistonWindow {
    // Build a window.
    let screen_width = frame_image.width();
    let screen_height = frame_image.height();
    let window_settings =
        WindowSettings::new("Atari 2600", [screen_width, screen_height]).exit_on_esc(true);
    return window_settings.build().expect("Could not build a window");
}
