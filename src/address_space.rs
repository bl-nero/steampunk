use crate::memory::{Memory, ReadError, ReadResult, WriteError, WriteResult};
use std::fmt;

/// Dispatches read/write calls to various devices with memory-mapped interfaces:
/// TIA, RAM, RIOT (not yet implemented), and ROM.
#[derive(Debug)]
pub struct AddressSpace<T: Memory, RA: Memory, RO: Memory> {
    pub tia: T,
    pub ram: RA,
    pub rom: RO,
}

enum MemoryArea {
    Tia,
    Ram,
    Riot,
    Rom,
}

impl<T: Memory, RA: Memory, RO: Memory> Memory for AddressSpace<T, RA, RO> {
    fn read(&self, address: u16) -> ReadResult {
        match Self::map_address(address) {
            Some(MemoryArea::Tia) => self.tia.read(address),
            Some(MemoryArea::Ram) => self.ram.read(address),
            Some(MemoryArea::Rom) => self.rom.read(address),
            Some(MemoryArea::Riot) | None => Err(ReadError { address }),
        }
    }

    fn write(&mut self, address: u16, value: u8) -> WriteResult {
        match Self::map_address(address) {
            Some(MemoryArea::Tia) => self.tia.write(address, value),
            Some(MemoryArea::Ram) => self.ram.write(address, value),
            // Yeah, I know. Writing to ROM. But hey, it's not the
            // AddressSpace's job to tell what you can or can't do with a given
            // segment of memory.
            Some(MemoryArea::Rom) => self.rom.write(address, value),
            Some(MemoryArea::Riot) | None => Err(WriteError { address, value }),
        }
    }
}

impl<T: Memory, RA: Memory, RO: Memory> AddressSpace<T, RA, RO> {
    fn map_address(address: u16) -> Option<MemoryArea> {
        if address & 0b0001_0000_0000_0000 != 0 {
            Some(MemoryArea::Rom)
        } else if address & 0b0000_0000_1000_0000 == 0 {
            Some(MemoryArea::Tia)
        } else if address & 0b0000_0010_1000_0000 == 0b0000_0000_1000_0000 {
            Some(MemoryArea::Ram)
        } else if address & 0b0000_0010_1000_0000 == 0b0000_0010_1000_0000 {
            Some(MemoryArea::Riot)
        } else {
            None
        }
    }
}

impl<T: Memory, RA: Memory, RO: Memory> fmt::Display for AddressSpace<T, RA, RO> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut zero_page: [u8; 0x100] = [0; 0x100];
        for i in 0..0x100 {
            zero_page[i] = self.read(i as u16).unwrap_or(0);
        }
        writeln!(f, "Zero page:")?;
        hexdump(f, 0x0000, &zero_page)
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
    use crate::memory::SimpleRam;
    use std::error;

    #[test]
    fn reads_and_writes() -> Result<(), Box<dyn error::Error>> {
        let mut address_space = AddressSpace {
            tia: SimpleRam::new(),
            ram: SimpleRam::new(),
            rom: SimpleRam::new(),
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

    #[test]
    fn address_mapping() {
        let mut address_space = AddressSpace {
            tia: SimpleRam::initialized_with(1),
            ram: SimpleRam::initialized_with(2),
            // riot: SimpleRam::initialized_with(3),
            rom: SimpleRam::initialized_with(4),
        };

        assert_eq!(address_space.read(0x8F45).unwrap(), 1);
        assert_eq!(address_space.read(0x6CD3).unwrap(), 2);
        assert_eq!(address_space.read(0x56A2).unwrap(), 4);

        address_space.write(0xA33F, 11).unwrap();
        address_space.write(0xC59A, 12).unwrap();
        address_space.write(0x3A58, 14).unwrap();

        assert_eq!(address_space.tia.bytes[0xA33F], 11);
        assert_eq!(address_space.ram.bytes[0xC59A], 12);
        assert_eq!(address_space.rom.bytes[0x3A58], 14);
    }
}
