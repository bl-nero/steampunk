use std::error;
use std::fmt;
use ya6502::memory::{Memory, Read, ReadResult, Rom, Write, WriteResult};

#[derive(Debug, Clone, PartialEq)]
pub struct RomSizeError {
    size: usize,
}
impl error::Error for RomSizeError {}
impl fmt::Display for RomSizeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Illegal ROM size: {} bytes. Valid sizes: 2048, 4096",
            self.size
        )
    }
}

pub fn new_rom(bytes: &[u8]) -> Result<Rom, RomSizeError> {
    match bytes.len() {
        2048 | 4096 => Ok(Rom::new(
            bytes,
            if bytes.len() == 0x1000 {
                0b0000_1111_1111_1111
            } else {
                0b0000_0111_1111_1111
            },
        )),
        _ => Err(RomSizeError { size: bytes.len() }),
    }
}

// A 128-byte memory structure that acts as Atari RAM and supports memory space
// mirroring.
#[derive(Debug)]
pub struct AtariRam {
    bytes: [u8; Self::SIZE],
}

impl AtariRam {
    const SIZE: usize = 0x80;
    pub fn new() -> AtariRam {
        AtariRam {
            bytes: [0; Self::SIZE],
        }
    }
}

impl Read for AtariRam {
    fn read(&self, address: u16) -> ReadResult {
        Ok(self.bytes[address as usize & 0b0111_1111])
    }
}

impl Write for AtariRam {
    fn write(&mut self, address: u16, value: u8) -> WriteResult {
        self.bytes[address as usize & 0b0111_1111] = value;
        Ok(())
    }
}

impl Memory for AtariRam {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atari_rom_4k() {
        let mut program = [0u8; 0x1000];
        program[5] = 1;
        let rom = new_rom(&program).unwrap();
        assert_eq!(rom.read(0x1000).unwrap(), 0);
        assert_eq!(rom.read(0x1005).unwrap(), 1);
        assert_eq!(rom.read(0x3005).unwrap(), 1);
        assert_eq!(rom.read(0xF005).unwrap(), 1);
    }

    #[test]
    fn atari_rom_2k() {
        let mut program = [0u8; 0x0800];
        program[5] = 1;
        let rom = new_rom(&program).unwrap();
        assert_eq!(rom.read(0x1000).unwrap(), 0);
        assert_eq!(rom.read(0x1005).unwrap(), 1);
        assert_eq!(rom.read(0x3005).unwrap(), 1);
        assert_eq!(rom.read(0xF005).unwrap(), 1);
        assert_eq!(rom.read(0xF805).unwrap(), 1);
    }

    #[test]
    fn atari_rom_illegal_size() {
        let rom = new_rom(&[0u8; 0x0900]);
        assert_eq!(rom.err(), Some(RomSizeError { size: 0x900 }));
    }

    #[test]
    fn atari_ram_read_write() {
        let mut ram = AtariRam::new();
        ram.write(0x00AB, 123).unwrap();
        ram.write(0x00AC, 234).unwrap();
        assert_eq!(ram.read(0x00AB).unwrap(), 123);
        assert_eq!(ram.read(0x00AC).unwrap(), 234);
    }

    #[test]
    fn atari_ram_mirroring() {
        let mut ram = AtariRam::new();
        ram.write(0x0080, 1).unwrap();
        assert_eq!(ram.read(0x0080).unwrap(), 1);
        assert_eq!(ram.read(0x2880).unwrap(), 1);
        assert_eq!(ram.read(0xCD80).unwrap(), 1);
    }
}
