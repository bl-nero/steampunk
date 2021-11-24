use std::error;
use std::fmt;
use std::result::Result;

pub trait Read {
    /// Reads a byte from given address. Returns the byte or error if the
    /// location is unsupported. In a release build, the errors should be
    /// ignored and the method should always return a successful result.
    fn read(&self, address: u16) -> ReadResult;
}

pub trait Write {
    /// Writes a byte to given address. Returns error if the location is
    /// unsupported. In a release build, the errors should be ignored and the
    /// method should always return a successful result.
    fn write(&mut self, address: u16, value: u8) -> WriteResult;
}

pub trait Memory: Read + Write {}

pub type ReadResult = Result<u8, ReadError>;

#[derive(Clone)]
pub struct ReadError {
    pub address: u16,
}

impl error::Error for ReadError {}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Unable to read from address ${:04X}", self.address)
    }
}

impl fmt::Debug for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("WriteError")
            .field("address", &format_args!("{:#06X}", self.address))
            .finish()
    }
}

pub type WriteResult = Result<(), WriteError>;

#[derive(Clone)]
pub struct WriteError {
    pub address: u16,
    pub value: u8,
}

impl error::Error for WriteError {}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Unable to write ${:02X} to address ${:04X}",
            self.value, self.address
        )
    }
}

impl fmt::Debug for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("WriteError")
            .field("address", &format_args!("{:#06X}", self.address))
            .field("value", &format_args!("{:#04X}", self.value))
            .finish()
    }
}

/// Random access memory.
pub struct Ram {
    pub bytes: Vec<u8>,
    /// Address mask used to access the underlying bytes. The byte index will be
    /// computed by using AND on address and the mask.
    address_mask: u16,
}

impl Ram {
    /// Creates a new RAM with an address bus of a given width (in bits). The
    /// total size of the RAM will be 2^address_width.
    pub fn new(address_width: u32) -> Ram {
        Self::initialized_with(0, address_width)
    }

    /// Creates a new RAM with an address bus of a given width (in bits),
    /// initialized with a given value. The total size of the RAM will be
    /// 2^address_width.
    pub fn initialized_with(value: u8, address_width: u32) -> Ram {
        Ram {
            bytes: vec![value; 1 << address_width],
            address_mask: ((1u32 << address_width) - 1) as u16,
        }
    }

    /// Creates 64KiB of `RAM`, putting given `program` at address 0xF000. It
    /// also sets the reset pointer to 0xF000.
    pub fn with_test_program(program: &[u8]) -> Ram {
        Self::with_test_program_at(0xF000, program)
    }

    /// Creates 64KiB of `RAM`, putting given `program` at a given address. It
    /// also sets the reset pointer to this address.
    pub fn with_test_program_at(address: u16, program: &[u8]) -> Ram {
        let mut ram = Ram::new(16);
        ram.bytes[address as usize..address as usize + program.len()].copy_from_slice(program);
        ram.bytes[0xFFFC] = address as u8; // least-significant byte
        ram.bytes[0xFFFD] = (address >> 8) as u8; // most-significant byte
        return ram;
    }
}

impl Read for Ram {
    fn read(&self, address: u16) -> ReadResult {
        // this arrow means we give u16 they return u8
        Ok(self.bytes[(address & self.address_mask) as usize])
    }
}

impl Write for Ram {
    fn write(&mut self, address: u16, value: u8) -> WriteResult {
        self.bytes[(address & self.address_mask) as usize] = value;
        Ok(())
    }
}

impl Memory for Ram {}

impl fmt::Debug for Ram {
    /// Prints out only the zero page, because come on, who would scroll through
    /// a dump of entire 64 kibibytes...
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use std::convert::TryInto;
        let zero_page: [u8; 255] = (&self.bytes[..255]).try_into().unwrap();
        return f
            .debug_struct("Ram")
            .field("zero page", &zero_page)
            .finish();
    }
}

/// Read-only memory.
pub struct Rom {
    bytes: Vec<u8>,
    /// Address mask used to access the underlying bytes. The byte index will be
    /// computed by using AND on address and the mask.
    address_mask: u16,
}

impl Rom {
    pub fn new(bytes: &[u8]) -> Result<Rom, MemorySizeError> {
        // Use the famous n & (n-1) trick to verify that the length of the bytes
        // array is a power of 2, and at the same time compute the address mask.
        let address_mask = bytes.len() - 1;
        return if address_mask > u16::MAX as usize || address_mask & bytes.len() != 0 {
            Err(MemorySizeError { size: bytes.len() })
        } else {
            Ok(Self {
                bytes: bytes.to_vec(),
                address_mask: address_mask as u16,
            })
        };
    }
}

impl Read for Rom {
    fn read(&self, address: u16) -> ReadResult {
        Ok(self.bytes[(address & self.address_mask) as usize])
    }
}

impl fmt::Debug for Rom {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> fmt::Result {
        f.debug_struct("Rom")
            .field("size", &self.bytes.len())
            .field("address_mask", &self.address_mask)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemorySizeError {
    size: usize,
}

impl error::Error for MemorySizeError {}

impl fmt::Display for MemorySizeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Illegal ROM size: {} bytes. Valid sizes: 2048, 4096",
            self.size
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creating_empty_ram() {
        let ram = Ram::with_test_program(&[]);
        assert_eq!(ram.bytes[..0xFFFC], [0u8; 0xFFFC][..]);
    }

    #[test]
    fn ram_read_write() {
        let mut ram = Ram::new(16);
        ram.write(0x00AB, 123).unwrap();
        ram.write(0x00AC, 234).unwrap();
        ram.write(0xE456, 34).unwrap();
        assert_eq!(ram.read(0x00AB).unwrap(), 123);
        assert_eq!(ram.read(0x00AC).unwrap(), 234);
        assert_eq!(ram.read(0xE456).unwrap(), 34);
    }

    #[test]
    fn ram_mirroring() {
        let mut ram = Ram::new(7);
        ram.write(0x0080, 1).unwrap();
        assert_eq!(ram.read(0x0080).unwrap(), 1);
        assert_eq!(ram.read(0x2880).unwrap(), 1);
        assert_eq!(ram.read(0xCD80).unwrap(), 1);
    }

    #[test]
    fn ram_with_test_program() {
        let ram = Ram::with_test_program(&[10, 56, 72, 255]);
        // Bytes until 0xF000 (exclusively) should have been zeroed.
        assert_eq!(ram.bytes[..0xF000], [0u8; 0xF000][..]);
        // Next, there should be our program.
        assert_eq!(ram.bytes[0xF000..0xF004], [10, 56, 72, 255][..]);
        // The rest, until 0xFFFC, should also be zeroed.
        assert_eq!(ram.bytes[0xF004..0xFFFC], [0u8; 0xFFFC - 0xF004][..]);
        // And finally, the reset vector.
        assert_eq!(ram.bytes[0xFFFC..0xFFFE], [0x00, 0xF0]);
    }

    #[test]
    fn ram_with_test_program_at() {
        let ram = Ram::with_test_program_at(0xF110, &[10, 56, 72, 255]);
        assert_eq!(ram.bytes[..0xF110], [0u8; 0xF110][..]);
        assert_eq!(ram.bytes[0xF110..0xF114], [10, 56, 72, 255][..]);
        assert_eq!(ram.bytes[0xF114..0xFFFC], [0u8; 0xFFFC - 0xF114][..]);
        assert_eq!(ram.bytes[0xFFFC..0xFFFE], [0x10, 0xF1]);
    }

    #[test]
    fn ram_with_test_program_sets_reset_address() {
        let ram = Ram::with_test_program(&[0xFF; 0x1000]);
        assert_eq!(ram.bytes[0xFFFC..0xFFFE], [0x00, 0xF0]); // 0xF000
    }

    #[test]
    fn rom_mirroring() {
        let mut program = [0u8; 0x1000];
        program[5] = 1;
        let rom = Rom::new(&program).unwrap();
        assert_eq!(rom.read(0x1000).unwrap(), 0);
        assert_eq!(rom.read(0x1005).unwrap(), 1);
        assert_eq!(rom.read(0x3005).unwrap(), 1);
        assert_eq!(rom.read(0xF005).unwrap(), 1);

        let mut program = [0u8; 0x0800];
        program[5] = 1;
        let rom = Rom::new(&program).unwrap();
        assert_eq!(rom.read(0x1000).unwrap(), 0);
        assert_eq!(rom.read(0x1005).unwrap(), 1);
        assert_eq!(rom.read(0x3005).unwrap(), 1);
        assert_eq!(rom.read(0xF005).unwrap(), 1);
        assert_eq!(rom.read(0xF805).unwrap(), 1);

        let rom = Rom::new(&[1, 2, 3, 4]).unwrap();
        assert_eq!(rom.read(0x01234).unwrap(), 1);
        assert_eq!(rom.read(0x01235).unwrap(), 2);
        assert_eq!(rom.read(0x01236).unwrap(), 3);
        assert_eq!(rom.read(0x01237).unwrap(), 4);
    }

    #[test]
    fn rom_illegal_sizes() {
        // Not a power of 2
        let rom = Rom::new(&[0u8; 0x09AB]);
        assert_eq!(rom.err(), Some(MemorySizeError { size: 0x9AB }));

        // Too large
        let rom = Rom::new(&[0u8; 0x20000]);
        assert_eq!(rom.err(), Some(MemorySizeError { size: 0x20000 }));
    }
}
