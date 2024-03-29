use crate::vic::raster_line_to_screen_y;
use crate::vic::VideoOutput;
use crate::vic::{LEFT_BORDER_START, TOP_BORDER_FIRST_LINE, VISIBLE_LINES, VISIBLE_PIXELS};
use common::colors::create_palette;
use common::colors::Palette;
use graphics::types::Rectangle;
use image::{Pixel, Rgba, RgbaImage};

/// This structure simulates a TV display. It consumes
/// [`VicOutput`](../vic/struct.VicOutput.html) structures and renders them
/// on an image surface.
pub struct FrameRenderer {
    palette: Palette,
    viewport: Rectangle<usize>,
    frame: RgbaImage,
    vblank: bool,
}

impl FrameRenderer {
    pub fn new(palette: Palette, viewport: Rectangle<usize>) -> Self {
        Self {
            palette,
            viewport,
            frame: RgbaImage::from_pixel(
                viewport[2] as u32,
                viewport[3] as u32,
                Rgba::from_channels(0x00, 0x00, 0x00, 0xFF),
            ),
            vblank: false,
        }
    }

    pub fn consume(&mut self, vic_output: VideoOutput) -> bool {
        // We convert the raster line number to screen Y in order to create a
        // continuous range against which a screen Y coordinate can be tested.
        let x_range = self.viewport[0]..self.viewport[0] + self.viewport[2];
        let y_range = self.viewport[1]..self.viewport[1] + self.viewport[3];
        let (x, y) = (
            vic_output.x,
            raster_line_to_screen_y(vic_output.raster_line),
        );
        let in_y_range = y_range.contains(&y);
        if x_range.contains(&x) && in_y_range {
            self.frame.put_pixel(
                (x - x_range.start) as u32,
                (y - y_range.start) as u32,
                self.palette[vic_output.color as usize],
            );
        }
        let frame_complete = !self.vblank && !in_y_range;
        self.vblank = !in_y_range;
        return frame_complete;
    }

    pub fn frame_image(&self) -> &RgbaImage {
        &self.frame
    }
}

impl Default for FrameRenderer {
    fn default() -> Self {
        // Colors generated using the Colodore algorithm described on
        // https://www.pepto.de/projects/colorvic/.
        let palette = create_palette(&[
            0x000000, 0xffffff, 0x813338, 0x75cec8, 0x8e3c97, 0x56ac4d, 0x2e2c9b, 0xedf171,
            0x8e5029, 0x553800, 0xc46c71, 0x4a4a4a, 0x7b7b7b, 0xa9ff9f, 0x706deb, 0xb2b2b2,
        ]);
        let viewport = [
            LEFT_BORDER_START,
            raster_line_to_screen_y(TOP_BORDER_FIRST_LINE),
            VISIBLE_PIXELS,
            VISIBLE_LINES,
        ];
        Self::new(palette, viewport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vic::screen_y_to_raster_line;
    use crate::vic::Color;
    use common::colors::create_palette;

    /// Returns a simple palette that is useful for testing.
    fn simple_palette() -> Palette {
        create_palette(&[0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF])
    }

    fn video_output(x: usize, y: usize, color: Color) -> VideoOutput {
        VideoOutput {
            x,
            raster_line: screen_y_to_raster_line(y),
            color,
        }
    }

    #[test]
    fn draws_pixels() {
        let mut fr = FrameRenderer::new(simple_palette(), [0, 0, 10, 10]);
        fr.consume(video_output(0, 0, 2));
        fr.consume(video_output(9, 0, 3));
        fr.consume(video_output(0, 9, 4));
        fr.consume(video_output(9, 9, 1));

        assert_eq!(
            fr.frame_image().get_pixel(0, 0),
            &Rgba::from_channels(0xFF, 0x00, 0x00, 0xFF)
        );
        assert_eq!(
            fr.frame_image().get_pixel(9, 0),
            &Rgba::from_channels(0x00, 0xFF, 0x00, 0xFF)
        );
        assert_eq!(
            fr.frame_image().get_pixel(0, 9),
            &Rgba::from_channels(0x00, 0x00, 0xFF, 0xFF)
        );
        assert_eq!(
            fr.frame_image().get_pixel(9, 9),
            &Rgba::from_channels(0xFF, 0xFF, 0xFF, 0xFF)
        );
    }

    #[test]
    fn uses_viewport() {
        let mut fr = FrameRenderer::new(simple_palette(), [4, 5, 6, 7]);
        // Red, green, and blue pixels
        fr.consume(video_output(4, 5, 2));
        fr.consume(video_output(7, 8, 3));
        fr.consume(video_output(9, 11, 4));

        // White pixels, right outside the viewport
        fr.consume(video_output(3, 8, 1));
        fr.consume(video_output(7, 4, 1));
        fr.consume(video_output(10, 8, 1));
        fr.consume(video_output(7, 12, 1));

        // Red, green, and blue all should appear within the viewport.
        assert_eq!(
            fr.frame_image().get_pixel(0, 0),
            &Rgba::from_channels(0xFF, 0x00, 0x00, 0xFF)
        );
        assert_eq!(
            fr.frame_image().get_pixel(3, 3),
            &Rgba::from_channels(0x00, 0xFF, 0x00, 0xFF)
        );
        assert_eq!(
            fr.frame_image().get_pixel(5, 6),
            &Rgba::from_channels(0x00, 0x00, 0xFF, 0xFF)
        );

        // No whites expected, they are outside.
        assert!(!fr
            .frame_image()
            .pixels()
            .any(|pixel| pixel == &Rgba::from_channels(0xFF, 0xFF, 0xFF, 0xFF)));
    }

    #[test]
    fn reports_end_of_frame() {
        // Create a 4x5 screen starting at (2, 3).
        let mut fr = FrameRenderer::new(simple_palette(), [2, 3, 4, 5]);
        // Starting from the middle of the screen, report false.
        assert_eq!(fr.consume(video_output(3, 5, 0)), false);
        assert_eq!(fr.consume(video_output(4, 5, 0)), false);
        // Moving to the bottom, report true.
        assert_eq!(fr.consume(video_output(4, 8, 0)), true);
        // Subsequent calls in the vertical blanking area should return false.
        assert_eq!(fr.consume(video_output(5, 8, 0)), false);
        // Left and right should still be false (horizontal blanking).
        fr.consume(video_output(4, 5, 0));
        assert_eq!(fr.consume(video_output(1, 5, 0)), false);
        assert_eq!(fr.consume(video_output(6, 5, 0)), false);
        // Moving to top should also switch to true.
        assert_eq!(fr.consume(video_output(4, 2, 0)), true);
    }
}
