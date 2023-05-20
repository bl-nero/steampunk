use std::{io, vec};

/// A Commodore 1530 Datasette device emulator. It is capable of playing a
/// series of pulses that represent tape data.
pub struct Datasette {
    tape: vec::IntoIter<u32>,
    tick_countdown: Option<u32>,
    play_pressed: bool,
}

#[derive(PartialEq, Debug)]
pub struct TickResult {
    /// Indicates whether any of the player buttons have been pressed.
    pub button_pressed: bool,
    /// `true` if a falling edge pulse has been generated on the data output
    /// line.
    pub pulse: bool,
}

/// A vector of pulses that represent tape data. Each number represents the
/// number of CPU cycles until the pulse is generated.
type Tape = Vec<u32>;

impl Datasette {
    /// Creates a new `Datasette` with the given tape. To obtain a tape, use the
    /// [`read_tap_file`] function.
    pub fn new(tape: Tape) -> Self {
        Datasette {
            tape: tape.into_iter(),
            tick_countdown: None,
            play_pressed: false,
        }
    }

    pub fn tick(&mut self, motor_on: bool) -> TickResult {
        if !(self.play_pressed && motor_on) {
            return TickResult {
                button_pressed: self.play_pressed,
                pulse: false,
            };
        }
        self.tick_countdown = self
            .tick_countdown
            .or_else(|| self.tape.next())
            .map(|c| c - 1);
        let pulse = self.tick_countdown == Some(0);
        if pulse {
            self.tick_countdown = None;
        }
        return TickResult {
            button_pressed: true,
            pulse,
        };
    }

    /// Sets the state of the play button.
    pub fn set_play_pressed(&mut self, pressed: bool) {
        self.play_pressed = pressed;
    }
}

/// Reads a TAP file from the given reader and returns a vector of pulses. TAP
/// format versions 0 are 1 are supported.
pub fn read_tap_file(mut reader: impl io::Read) -> Result<Vec<u32>, TapFileError> {
    const HEADER_SIZE: usize = 0x14;
    const FORMAT_VERSION_OFFSET: usize = 0x0C;
    const PLATFORM_OFFSET: usize = 0x0D;
    const DATA_SIZE_OFFSET: usize = 0x10;
    let mut header = [0u8; HEADER_SIZE];

    reader.read_exact(&mut header)?;
    if !header.starts_with("C64-TAPE-RAW".as_bytes()) {
        return Err(TapFileError::InvalidSignature);
    }
    let format_version = header[FORMAT_VERSION_OFFSET];
    if format_version != 0 && format_version != 1 {
        return Err(TapFileError::UnsupportedFormatVersion(format_version));
    }
    if header[PLATFORM_OFFSET] != 0 {
        return Err(TapFileError::UnsupportedPlatform(header[PLATFORM_OFFSET]));
    }
    let file_size = u32::from_le_bytes(
        header[DATA_SIZE_OFFSET..DATA_SIZE_OFFSET + 4]
            .try_into()
            .unwrap(),
    );

    let mut pulses = Vec::with_capacity(file_size.try_into().unwrap());
    loop {
        let mut byte_buf = [0u8; 1];
        let result = reader.read_exact(&mut byte_buf);
        if let Err(e) = result {
            if e.kind() == io::ErrorKind::UnexpectedEof {
                break;
            } else {
                return Err(e.into());
            }
        }
        match byte_buf[0] {
            0 => match format_version {
                0 => pulses.push(256 * 8),
                1 => {
                    let mut u32_buf = [0u8; 4];
                    reader.read_exact(&mut u32_buf[0..3])?;
                    pulses.push(u32::from_le_bytes(u32_buf));
                }
                other => return Err(TapFileError::UnsupportedFormatVersion(other)),
            },
            other => pulses.push(u32::from(other) * 8),
        }
    }

    return Ok(pulses);
}

#[derive(thiserror::Error, Debug)]
pub enum TapFileError {
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    #[error("Invalid TAP file signature")]
    InvalidSignature,

    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(u8),

    #[error("Unsupported format version: {0}")]
    UnsupportedFormatVersion(u8),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches::assert_matches;

    #[test]
    fn playing_empty_tape() {
        let mut ds = Datasette::new(vec![]);
        ds.set_play_pressed(true);
        assert_eq!(
            ds.tick(true),
            TickResult {
                button_pressed: true,
                pulse: false
            }
        );
    }

    #[test]
    fn playing_non_empty_tape() {
        let mut ds = Datasette::new(vec![3, 2]);
        ds.set_play_pressed(true);
        let results: Vec<_> = std::iter::repeat_with(move || ds.tick(true))
            .take(6)
            .collect();
        assert_eq!(
            results.iter().map(|r| r.button_pressed).collect::<Vec<_>>(),
            vec![true, true, true, true, true, true]
        );
        assert_eq!(
            results.iter().map(|r| r.pulse).collect::<Vec<_>>(),
            vec![false, false, true, false, true, false],
        );
    }

    #[test]
    fn motor_control() {
        let mut ds = Datasette::new(vec![1]);
        ds.set_play_pressed(true);
        assert_eq!(
            ds.tick(false),
            TickResult {
                button_pressed: true,
                pulse: false,
            }
        );
        assert_eq!(
            ds.tick(true),
            TickResult {
                button_pressed: true,
                pulse: true,
            }
        );
    }

    #[test]
    fn play_button() {
        let mut ds = Datasette::new(vec![1]);
        ds.set_play_pressed(false);
        assert_eq!(
            ds.tick(true),
            TickResult {
                button_pressed: false,
                pulse: false,
            }
        );
        ds.set_play_pressed(true);
        assert_eq!(
            ds.tick(true),
            TickResult {
                button_pressed: true,
                pulse: true,
            }
        );
    }

    #[test]
    fn tap_file_reading_success() {
        let tape = [
            "C64-TAPE-RAW".as_bytes(),
            &[0, 0, 0, 0, 3, 0, 0, 0, 10, 20, 30],
        ]
        .concat();
        let reader = read_tap_file(tape.as_slice()).unwrap();
        itertools::assert_equal(reader, [80, 160, 240]);

        let tape = [
            "C64-TAPE-RAW".as_bytes(),
            &[0, 0, 0, 0, 4, 0, 0, 0, 11, 12, 13, 14],
        ]
        .concat();
        let reader = read_tap_file(tape.as_slice()).unwrap();
        itertools::assert_equal(reader, [88, 96, 104, 112]);
    }

    #[test]
    fn tap_file_reading_invalid_signature() {
        let tape = ["C16-TAPE-RAW".as_bytes(), &[0, 0, 0, 0, 1, 0, 0, 0, 1]].concat();
        assert_matches!(
            read_tap_file(tape.as_slice()),
            Err(TapFileError::InvalidSignature)
        );
    }

    #[test]
    fn tap_file_reading_unsupported_platform() {
        let tape = ["C64-TAPE-RAW".as_bytes(), &[0, 1, 0, 0, 1, 0, 0, 0, 10]].concat();
        assert_matches!(
            read_tap_file(tape.as_slice()),
            Err(TapFileError::UnsupportedPlatform(1))
        );
    }

    #[test]
    fn tap_file_reading_truncated_header() {
        let tape = [0; 5];
        assert_matches!(
            read_tap_file(tape.as_slice()),
            Err(TapFileError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn tap_file_v0() {
        let tape = [
            "C64-TAPE-RAW".as_bytes(),
            &[0, 0, 0, 0, 3, 0, 0, 0, 4, 0, 200],
        ]
        .concat();
        let reader = read_tap_file(tape.as_slice()).unwrap();
        itertools::assert_equal(reader, [32, 2048, 1600]);
    }

    #[test]
    fn tap_file_v1() {
        let tape = [
            "C64-TAPE-RAW".as_bytes(),
            &[1, 0, 0, 0, 6, 0, 0, 0, 4, 0, 1, 2, 3, 4],
        ]
        .concat();
        let reader = read_tap_file(tape.as_slice()).unwrap();
        itertools::assert_equal(reader, [32, 0x030201, 32]);
    }

    #[test]
    fn tap_file_v1_truncated_data() {
        let tape = [
            "C64-TAPE-RAW".as_bytes(),
            &[1, 0, 0, 0, 4, 0, 0, 0, 4, 0, 1, 2],
        ]
        .concat();
        assert_matches!(
            read_tap_file(tape.as_slice()),
            Err(TapFileError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn tap_file_unknown_version() {
        let tape = ["C64-TAPE-RAW".as_bytes(), &[2, 0, 0, 0, 1, 0, 0, 0, 10]].concat();
        assert_matches!(
            read_tap_file(tape.as_slice()),
            Err(TapFileError::UnsupportedFormatVersion(2)),
        );
    }
}
