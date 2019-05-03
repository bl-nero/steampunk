use std::fmt;

const RAM_SIZE: usize = 0x10000;

/// A very simple memory structure. At the moment, it's just a 64-kilobyte chunk of RAM, for simplicity of addressing.
pub struct RAM {
    pub bytes: [u8; RAM_SIZE], //this means that computer has 25 u8's
}

impl RAM {
    /// Creates a new `RAM`, putting given `contents` at address 0xF000. It also sets the reset vector to 0xF000.
    pub fn new(contents: &[u8]) -> RAM {
        let mut ram = RAM {
            bytes: [0; RAM_SIZE],
        };
        for (i, byte) in contents.iter().enumerate() {
            ram.bytes[0xF000 + i] = *byte;
        }
        ram.bytes[0xFFFA] = 0x00;
        ram.bytes[0xFFFB] = 0xF0;
        return ram;
    }
    pub fn read(&self, address: u16) -> u8 {
        // this arrow means we give u16 they return u8
        self.bytes[address as usize]
    }
    pub fn write(&mut self, address: u16, value: u8) {
        self.bytes[address as usize] = value;
    }
}

impl fmt::Debug for RAM {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{zero page: {:?}}}", &self.bytes[..255])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_creates_empty_ram() {
        let ram = RAM::new(&[]);
        assert_eq!(ram.bytes[..0xFFFA], [0u8; 0xFFFA][..]);
    }

    #[test]
    fn it_fills_initial_contents() {
        let ram = RAM::new(&[10, 56, 72, 255]);
        assert_eq!(ram.bytes[..0xF000], [0u8; 0xF000][..]);
        assert_eq!(ram.bytes[0xF000..0xF004], [10, 56, 72, 255][..]);
        assert_eq!(ram.bytes[0xF004..0xFFFA], [0u8; 0xFFFA - 0xF004][..]);
    }

    #[test]
    fn it_sets_reset_address() {
        let ram = RAM::new(&[0xFF; 0x1000]);
        assert_eq!(ram.bytes[0xFFFA..0xFFFC], [0x00, 0xF0]);
    }
}
