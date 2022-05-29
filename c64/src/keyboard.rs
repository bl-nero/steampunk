use enum_map::{enum_map, Enum, EnumMap};

pub struct Keyboard {
    key_states: EnumMap<Key, KeyState>,
}

/// Emulates the C64 keyboard scanning matrix.
///
/// TODO: Support multiple key presses.
/// TODO: Support the RESTORE key.
/// TODO: Emulate ghosting.
impl Keyboard {
    pub fn new() -> Self {
        Self {
            key_states: enum_map!(_ => KeyState::Released),
        }
    }

    pub fn set_key_state(&mut self, key: Key, state: KeyState) {
        self.key_states[key] = state;
    }

    /// Simulates probing the keyboard state with given column bit mask. Returns
    /// row states as bits. The bit layout corresponds to appropriate CIA's port
    /// registers.
    pub fn scan(&self, mask: u8) -> u8 {
        for i in 0..=7 {
            let column_bit = 1 << i;
            if mask & column_bit == 0 {
                for j in 0..=7 {
                    if self.key_states[KEY_MATRIX[i][j]] == KeyState::Pressed {
                        return !(1 << j);
                    }
                }
            }
        }
        return 0xff;
    }
}

#[derive(Enum, Clone, Copy)]
pub enum Key {
    LeftArrow,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
    D9,
    D0,
    Plus,
    Minus,
    Pound,
    ClrHome,
    InstDel,

    Ctrl,
    Q,
    W,
    E,
    R,
    T,
    Y,
    U,
    I,
    O,
    P,
    At,
    Asterisk,
    UpArrow,
    Restore,

    RunStop,
    ShiftLock,
    A,
    S,
    D,
    F,
    G,
    H,
    J,
    K,
    L,
    Colon,
    Semicolon,
    Equals,
    Return,

    Commodore,
    LShift,
    Z,
    X,
    C,
    V,
    B,
    N,
    M,
    Comma,
    Period,
    Slash,
    RShift,
    CrsrUpDown,
    CrsrLeftRight,

    Space,

    F1,
    F3,
    F5,
    F7,
}

#[derive(PartialEq)]
pub enum KeyState {
    Pressed,
    Released,
}

const KEY_MATRIX: [[Key; 8]; 8] = [
    [
        Key::InstDel,
        Key::Return,
        Key::CrsrLeftRight,
        Key::F7,
        Key::F1,
        Key::F3,
        Key::F5,
        Key::CrsrUpDown,
    ],
    [
        Key::D3,
        Key::W,
        Key::A,
        Key::D4,
        Key::Z,
        Key::S,
        Key::E,
        Key::LShift,
    ],
    [
        Key::D5,
        Key::R,
        Key::D,
        Key::D6,
        Key::C,
        Key::F,
        Key::T,
        Key::X,
    ],
    [
        Key::D7,
        Key::Y,
        Key::G,
        Key::D8,
        Key::B,
        Key::H,
        Key::U,
        Key::V,
    ],
    [
        Key::D9,
        Key::I,
        Key::J,
        Key::D0,
        Key::M,
        Key::K,
        Key::O,
        Key::N,
    ],
    [
        Key::Plus,
        Key::P,
        Key::L,
        Key::Minus,
        Key::Period,
        Key::Colon,
        Key::At,
        Key::Comma,
    ],
    [
        Key::Pound,
        Key::Asterisk,
        Key::Semicolon,
        Key::ClrHome,
        Key::RShift,
        Key::Equals,
        Key::UpArrow,
        Key::Slash,
    ],
    [
        Key::D1,
        Key::LeftArrow,
        Key::Ctrl,
        Key::D2,
        Key::Space,
        Key::Commodore,
        Key::Q,
        Key::RunStop,
    ],
];

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_all_columns(keyboard: &Keyboard) -> [u8; 8] {
        let masks = [
            0b0111_1111,
            0b1011_1111,
            0b1101_1111,
            0b1110_1111,
            0b1111_0111,
            0b1111_1011,
            0b1111_1101,
            0b1111_1110,
        ];
        let mut result = [0; 8];
        for (i, mask) in masks.into_iter().enumerate() {
            result[i] = keyboard.scan(mask);
        }
        return result;
    }

    #[test]
    fn single_key_presses() {
        let mut k = Keyboard::new();
        k.set_key_state(Key::R, KeyState::Pressed);
        assert_eq!(
            scan_all_columns(&k),
            [!0, !0, !0, !0, !0, 0b1111_1101, !0, !0]
        );

        k.set_key_state(Key::R, KeyState::Released);
        k.set_key_state(Key::U, KeyState::Pressed);
        assert_eq!(
            scan_all_columns(&k),
            [!0, !0, !0, !0, 0b1011_1111, !0, !0, !0]
        );

        k.set_key_state(Key::U, KeyState::Released);
        k.set_key_state(Key::N, KeyState::Pressed);
        assert_eq!(
            scan_all_columns(&k),
            [!0, !0, !0, 0b0111_1111, !0, !0, !0, !0]
        );
    }
}
