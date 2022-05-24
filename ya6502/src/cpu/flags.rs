pub const N: u8 = 1 << 7;
pub const V: u8 = 1 << 6;
pub const UNUSED: u8 = 1 << 5;
pub const B: u8 = 1 << 4;
pub const D: u8 = 1 << 3;
pub const I: u8 = 1 << 2;
pub const Z: u8 = 1 << 1;
pub const C: u8 = 1;

/// Flags that are always on when the flag register is programmatically pushed
/// onto the stack.
pub const PUSHED: u8 = B | UNUSED;

pub enum FlagRepresentation {
    Stars,
    Letters,
}

const FLAGS_UNSET: [char; 8] = ['.', '.', '-', '.', '.', '.', '.', '.'];
const FLAGS_SET_STARS: [char; 8] = ['*', '*', '-', '*', '*', '*', '*', '*'];
const FLAGS_SET_LETTERS: [char; 8] = ['N', 'V', '-', 'B', 'D', 'I', 'Z', 'C'];

pub fn flags_to_string(flags: u8, representation: FlagRepresentation) -> String {
    let flags_set = match representation {
        FlagRepresentation::Stars => &FLAGS_SET_STARS,
        FlagRepresentation::Letters => &FLAGS_SET_LETTERS,
    };
    format!("{:08b}", flags)
        .chars()
        .enumerate()
        .map(|(i, ch)| match ch {
            '0' => FLAGS_UNSET[i],
            '1' => flags_set[i],
            _ => ch,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_to_string_stars() {
        use FlagRepresentation::Stars;
        assert_eq!(flags_to_string(0b1010_1010, Stars), "*.-.*.*.");
        assert_eq!(flags_to_string(0b0101_0101, Stars), ".*-*.*.*");
    }

    #[test]
    fn flags_to_string_letters() {
        use FlagRepresentation::Letters;
        assert_eq!(flags_to_string(0b1010_1010, Letters), "N.-.D.Z.");
        assert_eq!(flags_to_string(0b0101_0101, Letters), ".V-B.I.C");
    }
}
