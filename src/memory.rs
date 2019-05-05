use std::fmt;

const RAM_SIZE: usize = 0x10000; // 64 kB (64 * 1024)

/// A very simple memory structure. At the moment, it's just a 64-kilobyte chunk of RAM, for simplicity of addressing.
pub struct RAM {
    pub bytes: [u8; RAM_SIZE], // computer has RAM_SIZE (64k) bytes (unsigned 8-bit integers)
}

impl RAM {
    /// Creates a new `RAM`, putting given `program` at address 0xF000. It also sets the reset pointer to 0xF000.
    pub fn new(program: &[u8]) -> RAM {
        let mut ram = RAM {
            bytes: [0; RAM_SIZE], // Fill the entire RAM with 0x00.
        };

        // Copy the program into memory, starting from address 0xF000.
        for (i, byte) in program.iter().enumerate() {
            ram.bytes[0xF000 + i] = *byte;
        }

        // Initialize the reset address (stored at 0xFFFA) to 0xF000.
        ram.bytes[0xFFFA] = 0x00; // least-significant byte
        ram.bytes[0xFFFB] = 0xF0; // most-significant byte
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
    /// Prints out only the zero page, because come on, who would scroll through a dump of entire 64 kibibytes...
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
    fn it_places_program_in_memory() {
        let ram = RAM::new(&[10, 56, 72, 255]);
        // Bytes until 0xF000 (exclusively) should have been zeroed.
        assert_eq!(ram.bytes[..0xF000], [0u8; 0xF000][..]);
        // Next, there should be our program.
        assert_eq!(ram.bytes[0xF000..0xF004], [10, 56, 72, 255][..]);
        // The rest, until 0xFFFA, should also be zeroed.
        assert_eq!(ram.bytes[0xF004..0xFFFA], [0u8; 0xFFFA - 0xF004][..]);
    }

    #[test]
    fn it_sets_reset_address() {
        let ram = RAM::new(&[0xFF; 0x1000]);
        assert_eq!(ram.bytes[0xFFFA..0xFFFC], [0x00, 0xF0]); // 0xF000
    }
}
