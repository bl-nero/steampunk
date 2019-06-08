use image::Pixel;
use image::Rgba;

/// A color palette that maps 8-bit TIA color codes (see
/// [`tia::VideoOutput.pixel`](../tia/struct.VideoOutput.html#structfield.pixel))
/// to RGBA pixels.
pub type Palette = Vec<Rgba<u8>>;

/// Creates a TIA palette of RGBA colors out of an `u32` array slice. Each number
/// represents a 3-byte RGB color, where each channel is represented by 8 bits.
///
/// Note: TIA only uses 7 bits for representing colors, and bit 0 is unused. For
/// simplicity, we just store each color twice so that accessing the palette with
/// bit 0 set either to 0 or 1 yields the same RGBA pixel.
pub fn create_palette(colors: &[u32]) -> Palette {
    let mut palette = Palette::with_capacity(colors.len() * 2);
    for color in colors {
        let color_rgba = Rgba::from_channels(
            ((color & 0xFF0000) >> 16) as u8, // Red (most significant byte)
            ((color & 0xFF00) >> 8) as u8,    // Blue
            (color & 0xFF) as u8,             // Green (least significant byte)
            0xFF,                             // Alpha: always set to 100% opacity
        );
        palette.push(color_rgba);
        palette.push(color_rgba);
    }
    return palette;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creating_palette() {
        assert_eq!(create_palette(&[]), Palette::new());
        assert_eq!(
            create_palette(&[0x123456]),
            vec![
                *Rgba::from_slice(&[0x12, 0x34, 0x56, 0xFF]),
                *Rgba::from_slice(&[0x12, 0x34, 0x56, 0xFF]),
            ]
        );

        let three_color_palette = create_palette(&[0xFEDCBA, 0x5A0345, 0x12A5E4]);
        assert_eq!(
            three_color_palette,
            vec![
                *Rgba::from_slice(&[0xFE, 0xDC, 0xBA, 0xFF]),
                *Rgba::from_slice(&[0xFE, 0xDC, 0xBA, 0xFF]),
                *Rgba::from_slice(&[0x5A, 0x03, 0x45, 0xFF]),
                *Rgba::from_slice(&[0x5A, 0x03, 0x45, 0xFF]),
                *Rgba::from_slice(&[0x12, 0xA5, 0xE4, 0xFF]),
                *Rgba::from_slice(&[0x12, 0xA5, 0xE4, 0xFF]),
            ]
        );
    }
}
