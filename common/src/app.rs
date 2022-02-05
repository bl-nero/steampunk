use crate::debugger::adapter::DebugAdapter;
use crate::debugger::Debugger;
use image::RgbaImage;
use piston::{Event, EventLoop, WindowSettings};
use piston_window::{
    Filter, G2d, G2dTexture, G2dTextureContext, GfxDevice, PistonWindow, Texture, TextureSettings,
};
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A generic interface that provides basic operations common to all emulated
/// machines.
pub trait Machine {
    fn reset(&mut self);
    fn tick(&mut self) -> MachineTickResult;
    fn frame_image(&self) -> &RgbaImage;
    fn display_state(&self) -> String;
}

pub type MachineTickResult = Result<FrameStatus, Box<dyn Error>>;

pub enum FrameStatus {
    Pending,
    Complete,
}

/// An auxiliary controller that handles the machine lifecycle.
pub struct MachineController<'a, M: Machine, A: DebugAdapter> {
    machine: &'a mut M,
    running: bool,
    interrupted: Arc<AtomicBool>,
    debugger: Option<Debugger<A>>,
}

impl<'a, M: Machine, A: DebugAdapter> MachineController<'a, M, A> {
    pub fn new(machine: &'a mut M, debugger: Option<Debugger<A>>) -> Self {
        return Self {
            machine,
            running: false,
            interrupted: Arc::new(AtomicBool::new(false)),
            debugger,
        };
    }

    pub fn machine(&self) -> &M {
        self.machine
    }

    pub fn mut_machine(&mut self) -> &mut M {
        self.machine
    }

    pub fn reset(&mut self) {
        self.machine.reset();
        self.running = true;
    }

    pub fn run_until_end_of_frame(&mut self) {
        if let Some(debugger) = &mut self.debugger {
            debugger.process_meessages();
        }
        while self.running && !self.interrupted.load(Ordering::Relaxed) {
            match self.machine.tick() {
                Ok(FrameStatus::Pending) => {}
                Ok(FrameStatus::Complete) => return,
                Err(e) => {
                    self.running = false;
                    eprintln!("ERROR: {}. Machine halted.", e);
                    eprintln!("{}", self.display_state());
                }
            }
        }
    }

    pub fn frame_image(&self) -> &RgbaImage {
        self.machine.frame_image()
    }

    pub fn interrupted(&self) -> Arc<AtomicBool> {
        self.interrupted.clone()
    }

    pub fn display_state(&self) -> String {
        self.machine().display_state()
    }
}

pub trait AppController {
    fn frame_image(&self) -> &RgbaImage;
    fn reset(&mut self);
    fn interrupted(&self) -> Arc<AtomicBool>;

    /// Handles Piston events.
    fn event(&mut self, event: &Event);
    fn display_machine_state(&self) -> String;
}

pub struct Application<C: AppController> {
    window: PistonWindow,
    controller: C,
    view: View,
}

impl<C: AppController> Application<C> {
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

    /// Starts the machine and runs the event loop until the user decides to
    /// quit.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debugger::adapter::TcpDebugAdapter;
    use image::Pixel;
    use image::Rgba;
    use std::fmt;

    /// A very simple machine. All it does is producing three gray pixels with
    /// increasing luminosity.
    struct TestMachine {
        x: u32,
        color: Rgba<u8>,
        image: RgbaImage,
        broken: bool,
    }

    impl TestMachine {
        pub fn new() -> Self {
            Self {
                x: 0,
                color: Rgba::from_channels(1, 1, 1, 255),
                image: RgbaImage::new(3, 1),
                broken: false,
            }
        }
    }

    #[derive(Debug)]
    struct SomeError {}
    impl fmt::Display for SomeError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "SomeError")
        }
    }
    impl Error for SomeError {}

    impl Machine for TestMachine {
        fn reset(&mut self) {
            self.x = 0;
            self.color = Rgba::from_channels(1, 1, 1, 255);
            self.broken = false;
        }
        fn tick(&mut self) -> MachineTickResult {
            if self.broken {
                return Err(Box::new(SomeError {}));
            }
            self.image.put_pixel(self.x, 0, self.color);
            if self.x >= 2 {
                self.x = 0;
                self.color.apply_with_alpha(|c| c + 1, |a| a);
                return Ok(FrameStatus::Complete);
            } else {
                self.x += 1;
                return Ok(FrameStatus::Pending);
            }
        }
        fn frame_image(&self) -> &RgbaImage {
            &self.image
        }
        fn display_state(&self) -> String {
            format!("x={}", self.x)
        }
    }

    #[test]
    fn machine_controller_generates_frame() {
        let mut machine = TestMachine::new();
        let mut controller =
            MachineController::new(&mut machine, None::<Debugger<TcpDebugAdapter>>);
        controller.reset();

        controller.run_until_end_of_frame();
        assert_eq!(
            controller.frame_image().clone().into_raw(),
            RgbaImage::from_pixel(3, 1, Rgba::from_channels(1, 1, 1, 255)).into_raw(),
        );

        controller.run_until_end_of_frame();
        assert_eq!(
            controller.frame_image().clone().into_raw(),
            RgbaImage::from_pixel(3, 1, Rgba::from_channels(2, 2, 2, 255)).into_raw(),
        );
    }

    #[test]
    fn machine_controller_resets() {
        let mut machine = TestMachine::new();
        let mut controller =
            MachineController::new(&mut machine, None::<Debugger<TcpDebugAdapter>>);
        controller.reset();
        controller.run_until_end_of_frame();
        controller.reset();
        controller.run_until_end_of_frame();
        assert_eq!(
            controller.frame_image().clone().into_raw(),
            RgbaImage::from_pixel(3, 1, Rgba::from_channels(1, 1, 1, 255)).into_raw(),
        );
    }

    #[test]
    fn machine_controller_produces_images_until_interrupted() {
        let mut machine = TestMachine::new();
        let mut controller =
            MachineController::new(&mut machine, None::<Debugger<TcpDebugAdapter>>);
        controller.reset();

        controller.run_until_end_of_frame();
        controller.run_until_end_of_frame();
        controller.run_until_end_of_frame();
        assert_eq!(
            controller.frame_image().clone().into_raw(),
            RgbaImage::from_pixel(3, 1, Rgba::from_channels(3, 3, 3, 255)).into_raw(),
        );

        controller.interrupted().store(true, Ordering::Relaxed);
        controller.run_until_end_of_frame();
        assert_eq!(
            controller.frame_image().clone().into_raw(),
            RgbaImage::from_pixel(3, 1, Rgba::from_channels(3, 3, 3, 255)).into_raw(),
        );
    }

    #[test]
    fn machine_controller_halts_on_error_until_reset() {
        let mut machine = TestMachine::new();
        let mut controller =
            MachineController::new(&mut machine, None::<Debugger<TcpDebugAdapter>>);
        controller.reset();

        controller.run_until_end_of_frame();
        controller.run_until_end_of_frame();
        controller.machine.broken = true;
        controller.run_until_end_of_frame();
        controller.run_until_end_of_frame();
        assert_eq!(
            controller.frame_image().clone().into_raw(),
            RgbaImage::from_pixel(3, 1, Rgba::from_channels(2, 2, 2, 255)).into_raw(),
        );

        controller.reset();
        controller.run_until_end_of_frame();
        assert_eq!(
            controller.frame_image().clone().into_raw(),
            RgbaImage::from_pixel(3, 1, Rgba::from_channels(1, 1, 1, 255)).into_raw(),
        );
    }
}
