use crate::colors::Palette;
use crate::tia;
use crate::tia::VideoOutput;
use image::{Pixel, Rgba, RgbaImage};

/// This structure simulates a TV display. It consumes
/// [`VideoOutput`](../tia/struct.VideoOutput.html) structures and renders them
/// on an image surface. Use
/// [`FrameRendererBuilder`](struct.FrameRendererBuilder.html) to create an
/// instance of this class.
pub struct FrameRenderer {
    // *** CONFIGURATION ***
    palette: Palette,
    first_visible_scanline_index: i32,

    // *** INTERNAL STATE ***
    frame: RgbaImage,

    /// The X coordinate (column) of the next pixel to be processed. 0 is the
    /// beginning of the "front porch" signal (before the HSYNC part). Visible
    /// pixels start from `tia::HBLANK_WIDTH` column.
    x: i32,

    /// The Y coordinate (scanline) of the next pixel to be processed. 0 is the
    /// first scanline after the VSYNC signal ends. Visible pixels start from the
    /// `self.first_visible_scanline_index`.
    y: i32,

    in_hsync: bool,
    in_vsync: bool,
    had_first_vsync: bool,
}

impl FrameRenderer {
    /// Consumes a single `VideoOutput` structure and interprets its contents.
    /// Returns `true` if this particular cycle marks the frame as ready to be
    /// rendered on screen.
    pub fn consume(&mut self, video_output: VideoOutput) -> bool {
        // Handle the VSYNC signal by resetting the CRT beam to point at the top
        // of the screen. If it's not the first time, we return `true` to mark
        // the completion of a single frame.
        if video_output.vsync {
            if !self.in_vsync {
                // This quirk is one reason why `self.y` is a signed number.
                // Because the "first visible scanline index" is counted
                // starting from the first line AFTER the VSYNC signal (which is
                // counted as scan line 0), we set the scanline counter to -1
                // here.
                self.y = -1;
                self.in_vsync = true;
                if !self.had_first_vsync {
                    self.had_first_vsync = true;
                    return false;
                }
                return true;
            }
            return false;
        }
        self.in_vsync = false;

        // Handle the HSYNC signal. If encountered, move to the next scanline.
        // Because HSYNC lasts for a couple of cycles, we use `self.in_hsync` to
        // make sure that we move vertically only once per given HSYNC signal.
        if video_output.hsync {
            if !self.in_hsync {
                self.y += 1;
                self.x = tia::HSYNC_END as i32;
            }
            self.in_hsync = true;
            return false;
        }
        self.in_hsync = false;

        // Actually handle pixel data.
        if let Some(pixel) = video_output.pixel {
            let color = self.palette[pixel as usize];
            // Calculate coordinates in the viewport space.
            let x = self.x - tia::HBLANK_WIDTH as i32;
            let y = self.y - self.first_visible_scanline_index;
            let x_within_viewport = x >= 0 && x < self.frame.width() as i32;
            let y_within_viewport = y >= 0 && y < self.frame.height() as i32;
            if x_within_viewport && y_within_viewport {
                self.frame.put_pixel(x as u32, y as u32, color);
            }
        }
        self.x += 1;
        return false;
    }

    /// Returns a reference to the underlying frame image.
    pub fn frame_image(&self) -> &RgbaImage {
        &self.frame
    }
}

/// A builder for [`FrameRenderer`](struct.FrameRenderer.html) instances.
///
/// # Examples
/// ## Creating a `FrameRenderer` with default settings
/// ```
/// let mut frame_renderer = FrameRendererBuilder::new().build();
/// ```
///
/// ## Creating a more customized version
/// ```
/// let mut frame_renderer = FrameRendererBuilder::new()
///     .with_palette(SECAM_palette)
///     .with_height(1)
///     .with_first_visible_scanline_index(0)
///     .build();
/// ```
pub struct FrameRendererBuilder {
    height: u32,
    palette: Palette,
    first_visible_scanline_index: i32,
}

impl FrameRendererBuilder {
    /// Creates a new `FrameRendererBuilder` with default settings.
    pub fn new() -> FrameRendererBuilder {
        FrameRendererBuilder {
            height: 192,
            palette: Palette::new(),
            first_visible_scanline_index: 37,
        }
    }

    /// Changes the color palette.
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        return self;
    }

    /// Changes the viewport height.
    pub fn with_height(mut self, height: u32) -> Self {
        self.height = height;
        return self;
    }

    /// Sets which scanline will be the first one visible in the viewport. 0
    /// means the scanline that occurs immediately after VSYNC signal ends.
    #[cfg(test)]
    pub fn with_first_visible_scanline_index(mut self, index: i32) -> Self {
        self.first_visible_scanline_index = index;
        return self;
    }

    /// Creates the `FrameRenderer`. The builder can later be reused.
    pub fn build(&self) -> FrameRenderer {
        FrameRenderer {
            palette: self.palette.clone(),
            frame: RgbaImage::from_pixel(
                tia::FRAME_WIDTH,
                self.height,
                Rgba::from_channels(0x00, 0x00, 0x00, 0xFF),
            ),
            first_visible_scanline_index: self.first_visible_scanline_index,

            x: 0,
            y: self.first_visible_scanline_index + self.height as i32,
            in_hsync: false,
            in_vsync: false,
            had_first_vsync: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colors;
    use crate::test_utils;
    use image::Pixel;
    use std::iter;

    /// Returns a simple, 3-color palette that is nowhere near the actual palette
    /// of Atari, but is very convenient for testing.
    fn simple_palette() -> Palette {
        colors::create_palette(&[0xFF1111, 0x22FF22, 0x3333FF])
    }

    /// Decodes a character-based representation of TIA video output signal and
    /// feeds it to a given `FrameRenderer`. For the record of the string
    /// representation, see `test_utils::decode_video_outputs`.
    fn decode_and_consume(renderer: &mut FrameRenderer, encoded_signal: &str) {
        for output in test_utils::decode_video_outputs(encoded_signal) {
            renderer.consume(output);
        }
    }

    /// Returns an iterator that produces a single `tia::FRAME_WIDTH`-sized row
    /// of pixels with a given color.
    fn line_of<'a>(r: u8, g: u8, b: u8, a: u8) -> impl Iterator<Item = Rgba<u8>> {
        iter::repeat(image::Rgba::from_channels(r, g, b, a)).take(tia::FRAME_WIDTH as usize)
    }

    #[test]
    fn renders_pixels() {
        let mut fr = FrameRendererBuilder::new()
            .with_palette(simple_palette())
            .with_height(1)
            .with_first_visible_scanline_index(0)
            .build();

        // Start the frame (VSYNC) and the line (HSYNC).
        decode_and_consume(
            &mut fr,
            "----------------++++++++++++++++------------------------------------\
             ================================================================================\
             ================================================================================\
             ................||||||||||||||||....................................",
        );

        // Consume the actual pixels for testing.
        fr.consume(VideoOutput::pixel(0x00));
        fr.consume(VideoOutput::pixel(0x04));
        fr.consume(VideoOutput::pixel(0x02));

        let img = fr.frame_image();
        assert_eq!(
            *img.get_pixel(0, 0),
            Rgba::from_channels(0xFF, 0x11, 0x11, 0xFF)
        );
        assert_eq!(
            *img.get_pixel(1, 0),
            Rgba::from_channels(0x33, 0x33, 0xFF, 0xFF)
        );
        assert_eq!(
            *img.get_pixel(2, 0),
            Rgba::from_channels(0x22, 0xFF, 0x22, 0xFF)
        );
    }

    #[test]
    fn renders_scanlines() {
        let mut fr = FrameRendererBuilder::new()
            .with_palette(simple_palette())
            .with_height(2)
            .with_first_visible_scanline_index(0)
            .build();
        decode_and_consume(
            &mut fr,
            "----------------++++++++++++++++------------------------------------\
             --------------------------------------------------------------------------------\
             --------------------------------------------------------------------------------\
             ................||||||||||||||||....................................\
             22222222222222222222222222222222222222222222222222222222222222222222222222222222\
             22222222222222222222222222222222222222222222222222222222222222222222222222222222",
        );
        itertools::assert_equal(
            fr.frame_image().pixels().cloned(),
            line_of(0x22, 0xFF, 0x22, 0xFF).chain(line_of(0x00, 0x00, 0x00, 0xFF)),
        );

        decode_and_consume(
            &mut fr,
            "................||||||||||||||||....................................\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000",
        );
        itertools::assert_equal(
            fr.frame_image().pixels().cloned(),
            line_of(0x22, 0xFF, 0x22, 0xFF).chain(line_of(0xFF, 0x11, 0x11, 0xFF)),
        );
    }

    #[test]
    fn renders_frames() {
        let mut fr = FrameRendererBuilder::new()
            .with_palette(simple_palette())
            .with_height(3)
            .with_first_visible_scanline_index(0)
            .build();

        // Send some scanlines without sending VSYNC first. They should be ignored.
        decode_and_consume(
            &mut fr,
            "................||||||||||||||||....................................\
             22222222222222222222222222222222222222222222222222222222222222222222222222222222\
             22222222222222222222222222222222222222222222222222222222222222222222222222222222\
             ................||||||||||||||||....................................\
             22222222222222222222222222222222222222222222222222222222222222222222222222222222\
             22222222222222222222222222222222222222222222222222222222222222222222222222222222",
        );
        itertools::assert_equal(
            fr.frame_image().pixels().cloned(),
            line_of(0x00, 0x00, 0x00, 0xFF)
                .chain(line_of(0x00, 0x00, 0x00, 0xFF))
                .chain(line_of(0x00, 0x00, 0x00, 0xFF)),
        );

        // Now some actual frame: VSYNC, one blank line, two lines with pixels.
        decode_and_consume(
            &mut fr,
            "----------------++++++++++++++++------------------------------------\
             --------------------------------------------------------------------------------\
             --------------------------------------------------------------------------------\
             ................||||||||||||||||....................................\
             ................................................................................\
             ................................................................................\
             ................||||||||||||||||....................................\
             22222222222222222222222222222222222222222222222222222222222222222222222222222222\
             22222222222222222222222222222222222222222222222222222222222222222222222222222222\
             ................||||||||||||||||....................................\
             44444444444444444444444444444444444444444444444444444444444444444444444444444444\
             44444444444444444444444444444444444444444444444444444444444444444444444444444444",
        );
        itertools::assert_equal(
            fr.frame_image().pixels().cloned(),
            line_of(0x00, 0x00, 0x00, 0xFF)
                .chain(line_of(0x22, 0xFF, 0x22, 0xFF))
                .chain(line_of(0x33, 0x33, 0xFF, 0xFF)),
        );

        // One more frame, to make sure that the renderer is capable of
        // rendering one after another.
        decode_and_consume(
            &mut fr,
            "----------------++++++++++++++++------------------------------------\
             --------------------------------------------------------------------------------\
             --------------------------------------------------------------------------------\
             ................||||||||||||||||....................................\
             ................................................................................\
             ................................................................................\
             ................||||||||||||||||....................................\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000\
             ................||||||||||||||||....................................\
             22222222222222222222222222222222222222222222222222222222222222222222222222222222\
             22222222222222222222222222222222222222222222222222222222222222222222222222222222",
        );
        itertools::assert_equal(
            fr.frame_image().pixels().cloned(),
            line_of(0x00, 0x00, 0x00, 0xFF)
                .chain(line_of(0xFF, 0x11, 0x11, 0xFF))
                .chain(line_of(0x22, 0xFF, 0x22, 0xFF)),
        );
    }

    #[test]
    fn signals_that_frame_is_ready() {
        let mut fr = FrameRendererBuilder::new()
            .with_palette(simple_palette())
            .with_height(1)
            .with_first_visible_scanline_index(0)
            .build();

        // This time, because we want to repeat the sequence more than once, we
        // collect it into a vector.
        let outputs: Vec<VideoOutput> = test_utils::decode_video_outputs(
            "----------------++++++++++++++++------------------------------------\
             --------------------------------------------------------------------------------\
             --------------------------------------------------------------------------------\
             ................||||||||||||||||....................................\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000",
        )
        .collect();

        // Consume the frame once. The frame should not be ready.
        for (i, output) in outputs.iter().enumerate() {
            assert_eq!(fr.consume(output.clone()), false, "at index {}", i);
        }

        // Consume it once more. Consuming the start of the VSYNC signal should
        // mark the frame as ready (`FrameRenderer::consume()` should return `true`).
        assert_eq!(fr.consume(outputs[0].clone()), true, "at index 0");
        for (i, output) in outputs[1..].iter().enumerate() {
            assert_eq!(fr.consume(output.clone()), false, "at index {}", i + 1);
        }
    }

    #[test]
    fn ignores_signals_outside_viewport() {
        let mut fr = FrameRendererBuilder::new()
            .with_palette(simple_palette())
            .with_height(4)
            .with_first_visible_scanline_index(3)
            .build();
        assert_eq!(fr.frame_image().width(), 160);
        assert_eq!(fr.frame_image().height(), 4);

        // This carefully crafted frame contains:
        // * 2 lines of VSYNC
        // * 2 lines of vertical blank
        // * 1 line of pixels that should NOT appear on screen (because it's outside the viewport)
        // * 3 lines of pixel data with specific colors on the top-left and bottom-right corners to make sure that both of these appear on screen
        // * 1 more line of superfluous pixel data
        // * 1 line of overscan
        decode_and_consume(
            &mut fr,
            "----------------++++++++++++++++------------------------------------\
             --------------------------------------------------------------------------------\
             --------------------------------------------------------------------------------\
             ----------------++++++++++++++++------------------------------------\
             --------------------------------------------------------------------------------\
             --------------------------------------------------------------------------------\
             ................||||||||||||||||....................................\
             ................................................................................\
             ................................................................................\
             ................||||||||||||||||....................................\
             ................................................................................\
             ................................................................................\
             ................||||||||||||||||....................................\
             44444444444444444444444444444444444444444444444444444444444444444444444444444444\
             44444444444444444444444444444444444444444444444444444444444444444444444444444444\
             ................||||||||||||||||....................................\
             20000000000000000000000000000000000000000000000000000000000000000000000000000000\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000\
             ................||||||||||||||||....................................\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000\
             ................||||||||||||||||....................................\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000\
             ................||||||||||||||||....................................\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000\
             00000000000000000000000000000000000000000000000000000000000000000000000000000004\
             ................||||||||||||||||....................................\
             44444444444444444444444444444444444444444444444444444444444444444444444444444444\
             44444444444444444444444444444444444444444444444444444444444444444444444444444444\
             ................||||||||||||||||....................................\
             ................................................................................\
             ................................................................................",
        );

        // Check that pixels with colors 0x02 and 0x04 are in the top-left and
        // bottom-right corners, respectively.
        assert_eq!(
            fr.frame_image().get_pixel(0, 0),
            &Rgba::from_channels(0x22, 0xFF, 0x22, 0xFF)
        );
        assert_eq!(
            fr.frame_image().get_pixel(159, 3),
            &Rgba::from_channels(0x33, 0x33, 0xFF, 0xFF)
        );
    }

    #[test]
    fn supports_hsync_oddities() {
        let mut fr = FrameRendererBuilder::new()
            .with_palette(simple_palette())
            .with_height(3)
            .with_first_visible_scanline_index(0)
            .build();

        // This case is "weird", but may occur if the program strobes the TIA
        // RSYNC register.
        //
        // Note that the frame renderer doesn't support the interlaced mode
        // detection just yet.
        decode_and_consume(
            &mut fr,
            "----------------++++++++++++++++------------------------------------\
             --------------------------------------------------------------------------------\
             --------------------------------------------------------------------------------\
             ................\
             ................||||||||||||||||....................................\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000\
             00000000000000000000000000000000000000000000000000000000000000000000000000000000\
             ................||||||||||||||||....................................\
             22222222222222222222222222222222222222222222222222222222222222222222222222222222\
             ................||||||||||||||||....................................\
             44444444444444444444444444444444444444444444444444444444444444444444444444444444\
             44444444444444444444444444444444444444444444444444444444444444444444444444444444",
        );
        itertools::assert_equal(
            fr.frame_image().pixels().cloned(),
            // The first line should be a typical, single-color line. Nothing
            // unusual here, apart from the fact that the HSYNC signal was
            // delayed.
            line_of(0xFF, 0x11, 0x11, 0xFF)
                // Second line: filled just in half and interrupted by an HSYNC
                // signal, so we have 80 pixels of color 0x02 and 80 black ones.
                .chain(iter::repeat(image::Rgba::from_channels(0x22, 0xFF, 0x22, 0xFF)).take(80))
                .chain(iter::repeat(image::Rgba::from_channels(0x00, 0x00, 0x00, 0xFF)).take(80))
                // Finally, another regular line.
                .chain(line_of(0x33, 0x33, 0xFF, 0xFF)),
        );
    }
}
