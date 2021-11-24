use image::DynamicImage;
use std::fs::create_dir_all;
use std::path::Path;

pub fn as_single_hex_digit(n: u8) -> char {
    if n <= 0x0f {
        format!("{:X}", n)
            .chars()
            .last()
            .expect("Hex formatting error")
    } else {
        '?'
    }
}

/// Reads an image from the `src/test_data` directory of the binary project.
pub fn read_test_image(name: &str) -> DynamicImage {
    image::open(Path::new("src").join("test_data").join(name)).unwrap()
}

/// Compares the actual and expected image. If the images are different, it
/// saves the results on disk in the `results_dir_path` directory as a couple of
/// files named with given `test_name` and suffixes: `-actual.png`,
/// `-expected.png`, and `-diff.png`. It then panics to make the test fail.
pub fn assert_images_equal(
    actual: DynamicImage,
    expected: DynamicImage,
    test_name: &str,
    results_dir_path: &Path,
) {
    use image::GenericImageView;
    let equal = itertools::equal(actual.pixels(), expected.pixels());
    if equal {
        return;
    }

    create_dir_all(results_dir_path).unwrap();
    let actual_path = results_dir_path
        .join(String::from(test_name) + "-actual")
        .with_extension("png");
    let expected_path = results_dir_path
        .join(String::from(test_name) + "-expected")
        .with_extension("png");
    let diff_path = results_dir_path
        .join(String::from(test_name) + "-diff")
        .with_extension("png");

    let diff = image_diff::diff(&expected, &actual).unwrap();

    actual.save(&actual_path).unwrap();
    expected.save(&expected_path).unwrap();
    diff.save(&diff_path).unwrap();
    panic!(
        "Images differ for test {}\nExpected: {}\nActual: {}\nDiff: {}",
        test_name,
        expected_path.display(),
        actual_path.display(),
        diff_path.display(),
    );
}
