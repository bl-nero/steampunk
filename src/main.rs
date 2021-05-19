#![feature(test)]

mod address_space;
mod atari;
mod colors;
mod cpu;
mod frame_renderer;
mod memory;
mod tia;

mod test_utils;

use atari::{Atari, AtariAddressSpace};
use image::RgbaImage;
use memory::{AtariRam, AtariRom};
use piston::input::RenderEvent;
use piston_window::WindowSettings;
use piston_window::{PistonWindow, Texture, TextureSettings, Window};
use std::env;
use tia::Tia;

fn main() {
    println!("Ready player ONE!");

    let args: Vec<String> = env::args().collect();
    // Load an example ROM image.
    let rom_bytes = std::fs::read(&args[1]).unwrap();
    // Create and initialize components of the emulated system.
    let address_space = Box::new(AtariAddressSpace {
        tia: Tia::new(),
        ram: AtariRam::new(),
        rom: AtariRom::new(&rom_bytes[..]).unwrap(),
    });
    let mut atari = Atari::new(address_space);
    atari.reset();

    let mut window = build_window(atari.frame_image());

    // Create a texture.
    let texture_settings = TextureSettings::new().mag(piston_window::Filter::Nearest);
    let mut texture =
        Texture::from_image(&mut window.factory, atari.frame_image(), &texture_settings)
            .expect("Could not create a texture");

    // Main loop.
    while let Some(e) = window.next() {
        let window_size = window.size();
        if e.render_args().is_some() {
            let frame_image = atari.next_frame();
            texture
                .update(&mut window.encoder, frame_image)
                .expect("Unable to update texture");
            window.draw_2d(&e, |ctx, g| {
                graphics::clear([0.0, 0.0, 0.0, 1.0], g);
                graphics::Image::new()
                    .rect([0.0, 0.0, window_size.width, window_size.height])
                    .draw(&texture, &ctx.draw_state, ctx.transform, g);
            });
        }
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
