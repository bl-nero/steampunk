use crate::memory::{Memory, ReadError, ReadResult, WriteError, WriteResult, Ram};
use std::fmt;

/// Dispatches read/write calls to various devices with memory-mapped interfaces:
/// TIA, RAM, RIOT (not yet implemented), and ROM.
#[derive(Debug)]
pub struct AddressSpace<T: Memory, RA: Memory, RO: Memory> {
    pub tia: T,
    pub ram: RA,
    pub rom: RO,
}

impl<T: Memory, RA: Memory, RO: Memory> Memory for AddressSpace<T, RA, RO> {
    fn read(&self, address: u16) -> ReadResult {
        match address {
            0x0000..=0x007F => self.tia.read(address),
            0x0080..=0x00FF => self.ram.read(address),
            0xF000..=0xFFFF => self.rom.read(address),
            _ => Err(ReadError { address }),
        }
    }

    fn write(&mut self, address: u16, value: u8) -> WriteResult {
        match address {
            0x0000..=0x007F => self.tia.write(address, value),
            0x0080..=0x00FF => self.ram.write(address, value),
            // Yeah, I know. Writing to ROM. But hey, it's not the
            // AddressSpace's job to tell what you can or can't do with a given
            // segment of memory.
            0xF000..=0xFFFF => self.rom.write(address, value),
            _ => Err(WriteError { address, value }),
        }
    }
}

impl<T: Memory, RO: Memory> fmt::Display for AddressSpace<T, Ram, RO> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let zero_page = self.ram.raw_read(0x0000, 0x0100);
        writeln!(f, "Zero page:")?;
        hexdump(f, 0x0000, zero_page)
    }
}

/// Prints out a sequence of bytes on a given formatter in a hex dump format.
fn hexdump(f: &mut fmt::Formatter, offset: u16, bytes: &[u8]) -> fmt::Result {
    const LINE_WIDTH: usize = 16;
    use itertools::Itertools;
    for (line_num, line) in bytes.chunks(LINE_WIDTH).enumerate() {
        writeln!(
            f,
            "{:04X}: {:02X}",
            offset as usize + line_num * LINE_WIDTH,
            line.iter().format(" ")
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Ram;
    use std::error;

    #[test]
    fn reads_and_writes() -> Result<(), Box<dyn error::Error>> {
        let mut address_space = AddressSpace {
            tia: Ram::new(),
            ram: Ram::new(),
            rom: Ram::new(),
        };
        address_space.write(0, 8)?; // Start of TIA
        address_space.write(0x7f, 5)?; // End of TIA
        address_space.write(0x80, 81)?; // Start of RAM
        address_space.write(0xff, 45)?; // End of RAM
        address_space.write(0xf000, 15)?; // Start of ROM
        address_space.write(0xffff, 25)?; // End of ROM

        assert_eq!(address_space.tia.read(0)?, 8);
        assert_eq!(address_space.tia.read(0x7f)?, 5);
        assert_eq!(address_space.read(0)?, 8);
        assert_eq!(address_space.read(0x7f)?, 5);

        assert_eq!(address_space.ram.read(0x80)?, 81);
        assert_eq!(address_space.ram.read(0xff)?, 45);
        assert_eq!(address_space.read(0x80)?, 81);
        assert_eq!(address_space.read(0xff)?, 45);

        assert_eq!(address_space.rom.read(0xf000)?, 15);
        assert_eq!(address_space.rom.read(0xffff)?, 25);
        assert_eq!(address_space.read(0xf000)?, 15);
        assert_eq!(address_space.read(0xffff)?, 25);

        Ok(())
    }
}
