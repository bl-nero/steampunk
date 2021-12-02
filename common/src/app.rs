use image::RgbaImage;
use piston::{Event, EventLoop, WindowSettings};
use piston_window::{
    Filter, G2d, G2dTexture, G2dTextureContext, GfxDevice, PistonWindow, Texture, TextureSettings,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct Application<C: Controller> {
    window: PistonWindow,
    controller: C,
    view: View,
}

impl<C: Controller> Application<C> {
    /// Creates an emulator application that processes input using a given
    /// controller.
    pub fn new(controller: C, window_title: &str, pixel_width: u32, pixel_height: u32) -> Self {
        let initial_frame_image = controller.frame_image();
        let window_width = initial_frame_image.width() * pixel_width;
        let window_height = initial_frame_image.height() * pixel_height;
        let window_settings =
            WindowSettings::new(window_title, [window_width, window_height]).exit_on_esc(true);
        let mut window: PistonWindow = window_settings.build().expect("Could not build a window");
        window.set_ups(60);
        let texture_context = window.create_texture_context();
        let view = View::new(texture_context, initial_frame_image);

        Self {
            window,
            view,
            controller,
        }
    }

    /// Starts Atari and runs the event loop until the user decides to quit.
    pub fn run(&mut self) {
        self.controller.reset();
        while let Some(e) = self.window.next() {
            self.controller.event(&e);
            let view = &mut self.view;
            let frame_image = self.controller.frame_image();
            self.window.draw_2d(&e, |ctx, graphics, device| {
                view.draw(frame_image, ctx, graphics, device);
            });
            self.window.event(&e);
            if self.controller.interrupted().load(Ordering::Relaxed) {
                eprintln!("Interrupted!");
                eprintln!("{}", self.controller.display_machine_state());
                return;
            }
        }
    }

    /// Exposes a pointer to a thread-safe interruption flag. Once it's set to
    /// `true`, the main event loop finishes, allowing the program to quit
    /// gracefully.
    pub fn interrupted(&self) -> Arc<AtomicBool> {
        self.controller.interrupted()
    }
}

pub trait Controller {
    fn frame_image(&self) -> &RgbaImage;
    fn reset(&mut self);
    fn interrupted(&self) -> Arc<AtomicBool>;

    /// Handles Piston events.
    fn event(&mut self, event: &Event);

    fn display_machine_state(&self) -> String;
}

struct View {
    texture_context: G2dTextureContext,
    texture: G2dTexture,
}

impl View {
    fn new(mut texture_context: G2dTextureContext, initial_frame_image: &RgbaImage) -> Self {
        let texture_settings = TextureSettings::new().mag(Filter::Nearest);
        let texture =
            Texture::from_image(&mut texture_context, initial_frame_image, &texture_settings)
                .expect("Could not create a texture");
        return Self {
            texture_context,
            texture,
        };
    }

    fn draw(
        &mut self,
        frame_image: &RgbaImage,
        ctx: piston_window::Context,
        g: &mut G2d,
        device: &mut GfxDevice,
    ) {
        let texture_context = &mut self.texture_context;
        let texture = &mut self.texture;
        let frame_image = frame_image;
        texture
            .update(texture_context, frame_image)
            .expect("Unable to update texture");
        graphics::clear([0.0, 0.0, 0.0, 1.0], g);
        let view_size = ctx.get_view_size();
        graphics::Image::new()
            .rect([0.0, 0.0, view_size[0], view_size[1]])
            .draw(texture, &ctx.draw_state, ctx.transform, g);
        texture_context.encoder.flush(device);
    }
}
