pub mod address_space;
pub mod atari;
pub mod colors;
pub mod cpu;
pub mod frame_renderer;
pub mod memory;
pub mod tia;

pub mod test_utils;

use address_space::AddressSpace;
use memory::RAM;
use tia::TIA;
use atari::Atari;
use piston::window::Window;
use piston::window::WindowSettings;
use piston_window::{PistonWindow, Texture, TextureSettings};

fn main() {
    println!("Welcome player ONE!");
    let mut address_space = AddressSpace {
        tia: TIA::new(),
        ram: RAM::new(),
        rom: RAM::new(),
    };
    let mut atari = Atari::new(&mut address_space);
    let screen_width = atari.frame_image().width();
    let screen_height = atari.frame_image().height();

    let window_settings = WindowSettings::new("Atari 2600", [screen_width, screen_height]).exit_on_esc(true);
    let mut window: PistonWindow = window_settings.build().expect("Could not build a window");
    let mut texture_context = window.create_texture_context();
    let texture = Texture::empty(&mut texture_context).expect("Could not create a texture");
    while let Some(e) = window.next() {
        let window_size = window.size();
        window.draw_2d(&e, |ctx, g, _device| {
            let frame_image = atari.next_frame();
            texture.update(&mut texture_context, frame_image);
            graphics::clear([0.0, 0.0, 0.0, 1.0], g);
            graphics::Image::new()
                .rect([0.0, 0.0, window_size.width, window_size.height])
                .draw(&texture, &ctx.draw_state, ctx.transform, g);
        });
    }
    // let mut texture = Texture::from_image(
    //     &mut window.factory,
    //     &img,
    //     &TextureSettings::new().mag(piston_window::Filter::Nearest),
    // )
    // .unwrap();
}
