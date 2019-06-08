pub mod address_space;
pub mod atari;
pub mod colors;
pub mod cpu;
pub mod frame_renderer;
pub mod memory;
pub mod tia;

pub mod test_utils;

use address_space::AddressSpace;
use atari::Atari;
use memory::RAM;
use piston::input::RenderEvent;
use piston_window::WindowSettings;
use piston_window::{PistonWindow, Texture, TextureSettings, Window};
use std::path::Path;
use tia::TIA;

fn main() {
    println!("Welcome player ONE!");
    let rom = std::fs::read(
        Path::new(env!("OUT_DIR"))
            .join("roms")
            .join("horizontal_stripes.bin"),
    )
    .unwrap();

    let mut address_space = AddressSpace {
        tia: TIA::new(),
        ram: RAM::new(),
        rom: RAM::with_program(&rom[..]),
    };
    let mut atari = Atari::new(&mut address_space);
    atari.reset();
    let screen_width = atari.frame_image().width();
    let screen_height = atari.frame_image().height();

    let window_settings =
        WindowSettings::new("Atari 2600", [screen_width, screen_height]).exit_on_esc(true);
    let mut window: PistonWindow = window_settings.build().expect("Could not build a window");
    // let mut texture_context = window.create_texture_context();
    let texture_settings = TextureSettings::new().mag(piston_window::Filter::Nearest);
    let mut texture =
        Texture::from_image(&mut window.factory, atari.frame_image(), &texture_settings)
            .expect("Could not create a texture");
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
