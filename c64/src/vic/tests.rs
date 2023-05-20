#![cfg(test)]

use super::*;
use common::test_utils::as_single_hex_digit;
use ya6502::memory::Ram;

const CONTROL_1_DEFAULT: u8 = flags::CONTROL_1_SCREEN_ON | flags::CONTROL_1_RSEL | 3;

/// Creates a VIC backed by a simple RAM architecture, makes the screen visible,
/// and runs enough raster lines to end up at the beginning of the first visible
/// border line.
fn initialized_vic_for_testing() -> Vic<Ram, Ram> {
    let mut vic = vic_for_testing();
    vic.write(registers::CONTROL_1, CONTROL_1_DEFAULT).unwrap();
    for _ in 0..RASTER_LENGTH * TOP_BORDER_FIRST_LINE {
        vic.tick().unwrap();
    }
    return vic;
}

/// Creates a VIC backed by a simple RAM architecture.
fn vic_for_testing() -> Vic<Ram, Ram> {
    Vic::new(Box::new(Ram::new(16)), Rc::new(RefCell::new(Ram::new(16))))
}

/// Grabs a single visible raster line, discarding the blanking area. Note
/// that the visible area is established by convention, as we don't have to
/// pay attention to details too much here.
fn visible_raster_line<GM: Read, CM: Read>(vic: &mut Vic<GM, CM>) -> Vec<Color> {
    // Initialize to an illegal color to make sure that all pixels are
    // covered.
    let mut result = vec![0xFF; VISIBLE_PIXELS];
    for _ in 0..RASTER_LENGTH {
        let video_output = vic.tick().unwrap().video_output;
        if (LEFT_BORDER_START..BORDER_END).contains(&video_output.x) {
            result[video_output.x - LEFT_BORDER_START] = video_output.color;
        }
    }
    return result;
}

/// Grabs a raster line, and returns a range of pixels with given
/// coordinates relative to the left edge of the graphics display window.
fn grab_raster_line<GM: Read, CM: Read>(
    vic: &mut Vic<GM, CM>,
    left: isize,
    width: usize,
) -> Vec<Color> {
    let left = (DISPLAY_WINDOW_START as isize + left) as usize;
    let right = left + width;
    // Initialize to an illegal color to make sure that all pixels are
    // covered.
    let mut result = vec![0xFF; width];
    for _ in 0..RASTER_LENGTH {
        let video_output = vic.tick().unwrap().video_output;
        if (left..right).contains(&video_output.x) {
            result[video_output.x - left] = video_output.color;
        }
    }
    return result;
}

/// Skips a given number of full raster lines and discards results.
fn skip_raster_lines<GM: Read, CM: Read>(vic: &mut Vic<GM, CM>, n: usize) {
    for _ in 0..n * RASTER_LENGTH {
        vic.tick().unwrap();
    }
}

/// Skips to a beginning of a given raster line and discards results.
fn skip_to_raster_line<GM: Read, CM: Read>(vic: &mut Vic<GM, CM>, n: usize) {
    while vic.raster_counter == n {
        vic.tick().unwrap();
    }
    while vic.raster_counter != n {
        vic.tick().unwrap();
    }
}

/// Retrieves a full frame, including blank areas, and returns a rectangle
/// at given coordinates relative to the upper left corner of the graphics
/// display window.
fn grab_frame<GM: Read, FM: Read>(
    vic: &mut Vic<GM, FM>,
    left: isize,
    top: isize,
    width: usize,
    height: usize,
) -> Vec<Vec<Color>> {
    // We convert the raster line number to screen Y in order to create a
    // continuous range against which a screen Y coordinate can be tested.
    let top = raster_line_to_screen_y((DISPLAY_WINDOW_FIRST_LINE as isize + top) as usize);
    let left = (DISPLAY_WINDOW_START as isize + left) as usize;
    let bottom = top + height;
    let right = left + width;
    let mut result: Vec<Vec<Color>> = std::iter::repeat(vec![0xFF; width]).take(height).collect();
    for _ in 0..RASTER_LENGTH * TOTAL_HEIGHT {
        let video_output = vic.tick().unwrap().video_output;
        let (x, y) = (
            video_output.x,
            raster_line_to_screen_y(video_output.raster_line),
        );
        if (left..right).contains(&x) && (top..bottom).contains(&y) {
            result[y - top][x - left] = video_output.color;
        }
    }
    return result;
}

/// Encodes a sequence of colors into an easy to read string where each
/// color from a 4-bit palette is denoted by a single hexadecimal character.
/// The color 0 (black) is denoted as '.' for better readability.
fn encode_video<I: IntoIterator<Item = Color>>(outputs: I) -> String {
    outputs
        .into_iter()
        .map(|color| match color {
            0 => '.',
            c => as_single_hex_digit(c),
        })
        .collect()
}

fn encode_video_lines<Iter, IterIter>(outputs: IterIter) -> Vec<String>
where
    Iter: IntoIterator<Item = Color>,
    IterIter: IntoIterator<Item = Iter>,
{
    outputs.into_iter().map(encode_video).collect()
}

fn expect_no_interrupts_for<GM: Read, FM: Read>(n_ticks: usize, vic: &mut Vic<GM, FM>) {
    for _ in 0..n_ticks {
        let vic_output = vic.tick().unwrap();
        let video_output = vic_output.video_output;
        assert_eq!(
            vic_output.irq, false,
            "Unexpected IRQ at raster line {} pixel {}",
            video_output.raster_line, video_output.x,
        );
        assert_eq!(
            vic.read(registers::INTERRUPT).unwrap(),
            flags::INTERRUPT_UNUSED,
            "Unexpected IRQ at raster line {} pixel {}",
            video_output.raster_line,
            video_output.x,
        );
    }
}

/// Runs VIC until an IRQ is reported in [`VicOutput`][super::VicOutput].  Times
/// out after two screenfuls.
fn tick_until_irq<GM: Read, FM: Read>(vic: &mut Vic<GM, FM>) -> VicOutput {
    for _ in 0..2 * TOTAL_HEIGHT * RASTER_LENGTH {
        let tick_result = vic.tick().unwrap();
        let video_output = &tick_result.video_output;
        if tick_result.irq {
            assert_eq!(
                vic.read(registers::INTERRUPT).unwrap(),
                flags::INTERRUPT_UNUSED | flags::INTERRUPT_PENDING | flags::INTERRUPT_RASTER,
                "Inconsistent interrupt register at raster line {} pixel {}",
                video_output.raster_line,
                video_output.x,
            );
            return tick_result;
        }
        assert_eq!(
            vic.read(registers::INTERRUPT).unwrap(),
            flags::INTERRUPT_UNUSED,
            "Unexpected IRQ at raster line {} pixel {}",
            video_output.raster_line,
            video_output.x,
        );
    }
    panic!("IRQ not detected");
}

macro_rules! test_reg {
    ($fn_name: ident, $register:ident, $write:expr, $read:expr) => {
        #[test]
        fn $fn_name() {
            let mut vic = vic_for_testing();
            vic.write(registers::$register, $write).unwrap();
            assert_eq!(vic.read(registers::$register).unwrap(), $read);
        }
    };
}

test_reg!(rw_control_1_0, CONTROL_1, 0b0001_1011, 0b0001_1011);
test_reg!(rw_control_1_1, CONTROL_1, 0b1001_1011, 0b0001_1011);
test_reg!(rw_raster, RASTER, 0b1111_1111, 0b0000_0000);
test_reg!(rw_control_2_0, CONTROL_2, 0b1100_1011, 0b1100_1011);
test_reg!(rw_control_2_1, CONTROL_2, 0b0010_0100, 0b1110_0100);
test_reg!(rw_interrupt_0, INTERRUPT, 0b0000_0000, 0b0111_0000);
test_reg!(rw_interrupt_1, INTERRUPT, 0b0000_0001, 0b0111_0000);
test_reg!(
    rw_interrupt_mask_0,
    INTERRUPT_MASK,
    0b0000_0000,
    0b1111_0000
);
test_reg!(
    rw_interrupt_mask_1,
    INTERRUPT_MASK,
    0b0000_0001,
    0b1111_0001
);
test_reg!(rw_border_color_0, BORDER_COLOR, 0xF9, 0xF9);
test_reg!(rw_border_color_1, BORDER_COLOR, 0x06, 0xF6);
test_reg!(rw_background_color_0_0, BACKGROUND_COLOR_0, 0xF7, 0xF7);
test_reg!(rw_background_color_0_1, BACKGROUND_COLOR_0, 0x08, 0xF8);

#[test]
fn draws_border() {
    let mut vic = initialized_vic_for_testing();
    vic.write(registers::BORDER_COLOR, 0x00).unwrap();
    assert_eq!(vic.tick().unwrap().video_output.color, 0x00);

    vic.write(registers::BORDER_COLOR, 0x01).unwrap();
    assert_eq!(vic.tick().unwrap().video_output.color, 0x01);

    vic.write(registers::BORDER_COLOR, 0x0F).unwrap();
    assert_eq!(vic.tick().unwrap().video_output.color, 0x0F);
}

#[test]
fn draws_border_raster_lines() {
    let mut vic = initialized_vic_for_testing();
    vic.write(registers::BORDER_COLOR, 0x08).unwrap();
    vic.write(registers::BACKGROUND_COLOR_0, 0x0A).unwrap();
    vic.write(registers::CONTROL_2, flags::CONTROL_2_CSEL)
        .unwrap();
    let border_line = "8".repeat(VISIBLE_PIXELS);
    let border_and_display_line = "8".repeat(LEFT_BORDER_WIDTH)
        + &"A".repeat(DISPLAY_WINDOW_WIDTH)
        + &"8".repeat(RIGHT_BORDER_WIDTH);

    // Expect the first line of top border.
    assert_eq!(encode_video(visible_raster_line(&mut vic)), border_line);
    // Expect the last line of top border.
    skip_raster_lines(&mut vic, TOP_BORDER_HEIGHT - 2);
    assert_eq!(encode_video(visible_raster_line(&mut vic)), border_line);

    // Expect the first line of the display window.
    assert_eq!(
        encode_video(visible_raster_line(&mut vic)),
        border_and_display_line
    );

    // Last line of the display window and the first one of the bottom
    // border.
    skip_raster_lines(&mut vic, DISPLAY_WINDOW_HEIGHT - 2);
    assert_eq!(
        encode_video(visible_raster_line(&mut vic)),
        border_and_display_line
    );
    assert_eq!(encode_video(visible_raster_line(&mut vic)), border_line);

    // Last line of next frame's top border and first line of its display
    // window.
    skip_raster_lines(
        &mut vic,
        BOTTOM_BORDER_HEIGHT + BLANK_AREA_HEIGHT + TOP_BORDER_HEIGHT - 2,
    );
    assert_eq!(encode_video(visible_raster_line(&mut vic)), border_line);
    assert_eq!(
        encode_video(visible_raster_line(&mut vic)),
        border_and_display_line
    );
}

#[test]
fn draws_border_38_column_mode() {
    let mut vic = initialized_vic_for_testing();
    vic.write(registers::BORDER_COLOR, 0x05).unwrap();
    vic.write(registers::BACKGROUND_COLOR_0, 0x0C).unwrap();
    vic.write(registers::CONTROL_2, 0).unwrap();
    let narrow_display_line = "5".repeat(LEFT_BORDER_WIDTH + 8)
        + &"C".repeat(DISPLAY_WINDOW_WIDTH - 16)
        + &"5".repeat(RIGHT_BORDER_WIDTH + 8);

    skip_raster_lines(&mut vic, TOP_BORDER_HEIGHT);
    assert_eq!(
        encode_video(visible_raster_line(&mut vic)),
        narrow_display_line
    );
}

#[test]
fn draws_characters() {
    let mut vic = initialized_vic_for_testing();
    vic.write(registers::BORDER_COLOR, 0x01).unwrap();
    vic.write(registers::BACKGROUND_COLOR_0, 0x00).unwrap();
    vic.write(registers::CONTROL_2, flags::CONTROL_2_CSEL)
        .unwrap();

    // Set up characters
    vic.graphics_memory.bytes[0x1008..0x1028].copy_from_slice(&[
        0b11111111, 0b10000001, 0b10000001, 0b10000001, 0b10000001, 0b10000001, 0b10000001,
        0b11111111, 0b10000001, 0b01000010, 0b00100100, 0b00011000, 0b00011000, 0b00100100,
        0b01000010, 0b10000001, 0b00111100, 0b01000010, 0b10000001, 0b10000001, 0b10000001,
        0b10000001, 0b01000010, 0b00111100, 0b00011000, 0b00011000, 0b00100100, 0b00100100,
        0b01000010, 0b01000010, 0b10000001, 0b11111111,
    ]);
    // Set up screen
    vic.graphics_memory.bytes[0x0400] = 0x01;
    vic.graphics_memory.bytes[0x0401] = 0x02;
    vic.graphics_memory.bytes[0x0428] = 0x03;
    vic.graphics_memory.bytes[0x0429] = 0x04;
    // Set up colors
    {
        let mut color_memory = vic.color_memory.borrow_mut();
        color_memory.bytes[0xD800] = 0x0A;
        color_memory.bytes[0xD801] = 0x0B;
        color_memory.bytes[0xD828] = 0x0C;
        color_memory.bytes[0xD829] = 0x0D;
    }

    itertools::assert_equal(
        encode_video_lines(grab_frame(&mut vic, -1, -1, 17, 17)).iter(),
        &[
            "11111111111111111",
            "1AAAAAAAAB......B",
            "1A......A.B....B.",
            "1A......A..B..B..",
            "1A......A...BB...",
            "1A......A...BB...",
            "1A......A..B..B..",
            "1A......A.B....B.",
            "1AAAAAAAAB......B",
            "1..CCCC.....DD...",
            "1.C....C....DD...",
            "1C......C..D..D..",
            "1C......C..D..D..",
            "1C......C.D....D.",
            "1C......C.D....D.",
            "1.C....C.D......D",
            "1..CCCC..DDDDDDDD",
        ],
    );

    vic.graphics_memory.bytes[0x0400] = 0x04;
    vic.graphics_memory.bytes[0x0401] = 0x03;
    vic.graphics_memory.bytes[0x0428] = 0x02;
    vic.graphics_memory.bytes[0x0429] = 0x01;

    itertools::assert_equal(
        encode_video_lines(grab_frame(&mut vic, -1, -1, 17, 17)).iter(),
        &[
            "11111111111111111",
            "1...AA.....BBBB..",
            "1...AA....B....B.",
            "1..A..A..B......B",
            "1..A..A..B......B",
            "1.A....A.B......B",
            "1.A....A.B......B",
            "1A......A.B....B.",
            "1AAAAAAAA..BBBB..",
            "1C......CDDDDDDDD",
            "1.C....C.D......D",
            "1..C..C..D......D",
            "1...CC...D......D",
            "1...CC...D......D",
            "1..C..C..D......D",
            "1.C....C.D......D",
            "1C......CDDDDDDDD",
        ],
    );
}

#[test]
fn horizontal_scrolling() {
    let mut vic = initialized_vic_for_testing();
    vic.write(registers::BORDER_COLOR, 0x01).unwrap();
    vic.write(registers::BACKGROUND_COLOR_0, 0x00).unwrap();
    let grab_line_left = move |vic: &mut Vic<Ram, Ram>| encode_video(grab_raster_line(vic, -1, 17));

    // Character 1: a simple bit pattern
    vic.graphics_memory.bytes[0x1008..0x1010].copy_from_slice(&[0b1010_0101; 8]);
    vic.graphics_memory.bytes[0x0400] = 0x01;
    {
        vic.color_memory.borrow_mut().bytes[0xD800] = 0x0A;
    }

    // Skip top border
    skip_raster_lines(&mut vic, TOP_BORDER_HEIGHT);

    vic.write(0xD016, flags::CONTROL_2_CSEL).unwrap();
    assert_eq!(grab_line_left(&mut vic), "1A.A..A.A........");
    vic.write(0xD016, flags::CONTROL_2_CSEL | 1).unwrap();
    assert_eq!(grab_line_left(&mut vic), "1.A.A..A.A.......");
    vic.write(0xD016, flags::CONTROL_2_CSEL | 2).unwrap();
    assert_eq!(grab_line_left(&mut vic), "1..A.A..A.A......");
    vic.write(0xD016, flags::CONTROL_2_CSEL | 7).unwrap();
    assert_eq!(grab_line_left(&mut vic), "1.......A.A..A.A.");
}

#[test]
fn raster_counter() {
    let mut vic = initialized_vic_for_testing();
    const TOP: u8 = TOP_BORDER_FIRST_LINE as u8;
    let read_raster8 =
        |vic: &mut Vic<_, _>| vic.read(registers::CONTROL_1).unwrap() & flags::CONTROL_1_RASTER_8;
    assert_eq!(vic.read(registers::RASTER).unwrap(), TOP);
    assert_eq!(read_raster8(&mut vic), 0);

    skip_raster_lines(&mut vic, 1);
    assert_eq!(vic.read(registers::RASTER).unwrap(), TOP + 1);
    assert_eq!(read_raster8(&mut vic), 0);

    skip_raster_lines(&mut vic, 255 - TOP_BORDER_FIRST_LINE - 1);
    assert_eq!(vic.read(registers::RASTER).unwrap(), 255);
    assert_eq!(read_raster8(&mut vic), 0);

    skip_raster_lines(&mut vic, 1);
    assert_eq!(vic.read(registers::RASTER).unwrap(), 0);
    assert_eq!(read_raster8(&mut vic), flags::CONTROL_1_RASTER_8);

    skip_raster_lines(&mut vic, 1);
    assert_eq!(vic.read(registers::RASTER).unwrap(), 1);
    assert_eq!(read_raster8(&mut vic), flags::CONTROL_1_RASTER_8);
}

#[test]
fn raster_irq() {
    let mut vic = initialized_vic_for_testing();
    vic.write(registers::INTERRUPT, flags::INTERRUPT_RASTER)
        .unwrap(); // No IRQs expected, but acknowledge just in case.
    vic.write(registers::INTERRUPT_MASK, flags::INTERRUPT_RASTER)
        .unwrap();
    vic.write(registers::RASTER, 60).unwrap();
    vic.write(registers::CONTROL_1, CONTROL_1_DEFAULT).unwrap();

    let vic_output = tick_until_irq(&mut vic);
    assert_eq!(vic_output.video_output.raster_line, 60);
    assert_eq!(vic_output.video_output.x, 0);

    // Interrupt continues until it's acknowledged.
    skip_raster_lines(&mut vic, 2);
    assert_eq!(vic.tick().unwrap().irq, true);
    assert_eq!(
        vic.read(registers::INTERRUPT).unwrap(),
        flags::INTERRUPT_UNUSED | flags::INTERRUPT_PENDING | flags::INTERRUPT_RASTER,
    );

    // That's not a proper acknowledgement!
    vic.write(registers::INTERRUPT, 0).unwrap();
    assert_eq!(vic.tick().unwrap().irq, true);
    assert_eq!(
        vic.read(registers::INTERRUPT).unwrap(),
        flags::INTERRUPT_UNUSED | flags::INTERRUPT_PENDING | flags::INTERRUPT_RASTER,
    );

    // Actually acknowledge.
    vic.write(registers::INTERRUPT, flags::INTERRUPT_RASTER)
        .unwrap();
    assert_eq!(vic.tick().unwrap().irq, false);
    assert_eq!(
        vic.read(registers::INTERRUPT).unwrap(),
        flags::INTERRUPT_UNUSED,
    );

    // Trigger an interrupt at a different raster line.
    vic.write(registers::RASTER, 73).unwrap();
    let vic_output = tick_until_irq(&mut vic);
    assert_eq!(vic_output.video_output.raster_line, 73);
    assert_eq!(vic_output.video_output.x, 0);
    vic.write(registers::INTERRUPT, flags::INTERRUPT_RASTER)
        .unwrap(); // Acknowledge.

    // Disable raster IRQ.
    vic.write(registers::INTERRUPT_MASK, 0).unwrap();
    expect_no_interrupts_for(TOTAL_HEIGHT * RASTER_LENGTH, &mut vic);
}

#[test]
fn raster_irq_bit_8() {
    let mut vic = initialized_vic_for_testing();
    vic.write(registers::INTERRUPT, flags::INTERRUPT_RASTER)
        .unwrap(); // No IRQs expected, but acknowledge just in case.

    vic.write(registers::RASTER, 3).unwrap();
    vic.write(
        registers::CONTROL_1,
        CONTROL_1_DEFAULT | flags::CONTROL_1_RASTER_8,
    )
    .unwrap();
    vic.write(registers::INTERRUPT_MASK, flags::INTERRUPT_RASTER)
        .unwrap();

    let vic_output = tick_until_irq(&mut vic);
    assert_eq!(vic_output.video_output.raster_line, 259);

    vic.write(registers::INTERRUPT, flags::INTERRUPT_RASTER)
        .unwrap(); // Acknowledge.
    vic.write(registers::RASTER, 1).unwrap();
    let vic_output = tick_until_irq(&mut vic);
    assert_eq!(vic_output.video_output.raster_line, 257);

    vic.write(registers::INTERRUPT, flags::INTERRUPT_RASTER)
        .unwrap(); // Acknowledge.
    vic.write(registers::CONTROL_1, CONTROL_1_DEFAULT).unwrap();
    let vic_output = tick_until_irq(&mut vic);
    assert_eq!(vic_output.video_output.raster_line, 1);
}

#[test]
fn screen_on_off() {
    let mut vic = initialized_vic_for_testing();
    vic.write(registers::BORDER_COLOR, 0x08).unwrap();
    vic.write(registers::BACKGROUND_COLOR_0, 0x00).unwrap();
    vic.write(
        registers::CONTROL_1,
        CONTROL_1_DEFAULT & !flags::CONTROL_1_SCREEN_ON,
    )
    .unwrap();

    assert_eq!(
        encode_video_lines(grab_frame(&mut vic, 160, 100, 1, 1)),
        ["8"],
        "Displays border color in the middle of the screen"
    );
}

#[test]
fn screen_on_off_decides_on_raster_line_48() {
    let mut vic = initialized_vic_for_testing();
    vic.write(registers::BORDER_COLOR, 0x0A).unwrap();
    vic.write(registers::BACKGROUND_COLOR_0, 0x00).unwrap();
    skip_to_raster_line(&mut vic, 49);
    vic.write(
        registers::CONTROL_1,
        CONTROL_1_DEFAULT & !flags::CONTROL_1_SCREEN_ON,
    )
    .unwrap();
    assert_eq!(
        encode_video_lines(grab_frame(&mut vic, 160, 100, 1, 1)),
        ["."],
        "Displays screen color before switching screen off on line 48",
    );

    assert_eq!(
        encode_video_lines(grab_frame(&mut vic, 160, 100, 1, 1)),
        ["A"],
        "Displays border color after seeing the screen switched off on line 48",
    );
}
