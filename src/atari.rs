use image;
use image::DynamicImage;
use image::GenericImageView;
use image::Pixel;
use image::Rgba;
use image::RgbaImage;
use lcs_image_diff;
use std::fs;
use std::path::Path;

struct Atari {
    img: RgbaImage,
}

impl Atari {
    pub fn new(rom: &[u8]) -> Atari {
        Atari {
            // img: RgbaImage::new(160, 192),
            img: RgbaImage::from_pixel(1970, 1540, Rgba::from_channels(0, 0, 0, 255)),
            // img: image::open("src/test_data/horizontal_stripes.png")
            //     .unwrap()
            //     .to_rgba(),
        }
    }

    pub fn next_frame(&mut self) -> &RgbaImage {
        &self.img
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_images_equal(mut actual: DynamicImage, mut expected: DynamicImage, test_name: &str) {
        let start = std::time::Instant::now();
        let equal = itertools::equal(actual.pixels(), expected.pixels());
        if equal {
            return;
        }

        let diff = lcs_image_diff::compare(&mut actual, &mut expected, 0.5).unwrap();
        let dir_path = Path::new(env!("OUT_DIR")).join("test_results");
        fs::create_dir_all(&dir_path).unwrap();

        let actual_path = dir_path
            .join(String::from(test_name) + "-actual")
            .with_extension("png");
        let expected_path = dir_path
            .join(String::from(test_name) + "-expected")
            .with_extension("png");
        let diff_path = dir_path
            .join(String::from(test_name) + "-diff")
            .with_extension("png");

        actual.save(&actual_path).unwrap();
        expected.save(&expected_path).unwrap();
        diff.save(&diff_path).unwrap();
        panic!(
            "Images differ for test {}\nActual: {}\nExpected: {}\nDiff: {}",
            test_name,
            actual_path.display(),
            expected_path.display(),
            diff_path.display()
        );
    }

    #[test]
    fn shows_horizontal_stripes() {
        let rom = std::fs::read(
            Path::new(env!("OUT_DIR"))
                .join("roms")
                .join("horizontal_stripes.bin"),
        )
        .unwrap();
        let mut atari = Atari::new(&rom[..]);
        let img1 = DynamicImage::ImageRgba8(atari.next_frame().clone());
        let img2 = image::open(
            Path::new("src")
                .join("test_data")
                .join("horizontal_stripes.png"),
        )
        .unwrap();
        assert_images_equal(img1, img2, "shows_horizontal_stripes");
    }
}
