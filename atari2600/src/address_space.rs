use std::fmt;
use ya6502::memory::Read;
use ya6502::memory::Write;
use ya6502::memory::{Memory, ReadError, ReadResult, WriteError, WriteResult};

/// Dispatches read/write calls to various devices with memory-mapped interfaces:
/// TIA, RAM, RIOT (not yet implemented), and ROM.
#[derive(Debug)]
pub struct AddressSpace<T: Memory, RA: Memory, RI: Memory, RO: Read> {
    pub tia: T,
    pub ram: RA,
    pub riot: RI,
    pub rom: RO,
}

enum MemoryArea {
    Tia,
    Ram,
    Riot,
    Rom,
}

impl<T: Memory, RA: Memory, RI: Memory, RO: Read> Read for AddressSpace<T, RA, RI, RO> {
    fn read(&self, address: u16) -> ReadResult {
        match map_address(address) {
            Some(MemoryArea::Tia) => self.tia.read(address),
            Some(MemoryArea::Ram) => self.ram.read(address),
            Some(MemoryArea::Rom) => self.rom.read(address),
            Some(MemoryArea::Riot) => self.riot.read(address),
            None => Err(ReadError { address }),
        }
    }
}

impl<T: Memory, RA: Memory, RI: Memory, RO: Read> Write for AddressSpace<T, RA, RI, RO> {
    fn write(&mut self, address: u16, value: u8) -> WriteResult {
        match map_address(address) {
            Some(MemoryArea::Tia) => self.tia.write(address, value),
            Some(MemoryArea::Ram) => self.ram.write(address, value),
            Some(MemoryArea::Rom) => Ok(()),
            Some(MemoryArea::Riot) => self.riot.write(address, value),
            None => Err(WriteError { address, value }),
        }
    }
}

impl<T: Memory, RA: Memory, RI: Memory, RO: Read> Memory for AddressSpace<T, RA, RI, RO> {}

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

impl<T: Memory, RA: Memory, RI: Memory, RO: Read> fmt::Display for AddressSpace<T, RA, RI, RO> {
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
    use std::error;
    use ya6502::memory::SimpleRam;

    #[test]
    fn reads_and_writes() -> Result<(), Box<dyn error::Error>> {
        let mut address_space = AddressSpace {
            tia: SimpleRam::new(),
            ram: SimpleRam::new(),
            riot: SimpleRam::new(),
            rom: SimpleRam::new(),
        };
        address_space.write(0, 8)?; // Start of TIA
        address_space.write(0x7F, 5)?; // End of TIA
        address_space.write(0x80, 81)?; // Start of RAM
        address_space.write(0xFF, 45)?; // End of RAM
        address_space.write(0x280, 67)?; // Start of RIOT
        address_space.write(0x29F, 68)?; // End of RIOT

        // Note: we can't "officially" write to ROM using an AddressSpace.
        address_space.rom.bytes[0xF000] = 15; // Start of ROM
        address_space.rom.bytes[0xFFFF] = 25; // End of ROM

        assert_eq!(address_space.tia.read(0)?, 8);
        assert_eq!(address_space.tia.read(0x7F)?, 5);
        assert_eq!(address_space.read(0)?, 8);
        assert_eq!(address_space.read(0x7F)?, 5);

        assert_eq!(address_space.ram.read(0x80)?, 81);
        assert_eq!(address_space.ram.read(0xFF)?, 45);
        assert_eq!(address_space.read(0x80)?, 81);
        assert_eq!(address_space.read(0xFF)?, 45);

        assert_eq!(address_space.riot.read(0x280)?, 67);
        assert_eq!(address_space.riot.read(0x29F)?, 68);
        assert_eq!(address_space.read(0x280)?, 67);
        assert_eq!(address_space.read(0x29F)?, 68);

        assert_eq!(address_space.rom.read(0xF000)?, 15);
        assert_eq!(address_space.rom.read(0xFFFF)?, 25);
        assert_eq!(address_space.read(0xF000)?, 15);
        assert_eq!(address_space.read(0xFFFF)?, 25);

        Ok(())
    }

    #[test]
    fn address_mapping() {
        let mut address_space = AddressSpace {
            tia: SimpleRam::initialized_with(1),
            ram: SimpleRam::initialized_with(2),
            riot: SimpleRam::initialized_with(3),
            rom: SimpleRam::initialized_with(4),
        };

        assert_eq!(address_space.read(0x8F45).unwrap(), 1);
        assert_eq!(address_space.read(0x6CD3).unwrap(), 2);
        assert_eq!(address_space.read(0x2ABC).unwrap(), 3);
        assert_eq!(address_space.read(0x56A2).unwrap(), 4);

        address_space.write(0xA33F, 11).unwrap();
        address_space.write(0xC59A, 12).unwrap();
        address_space.write(0x86AB, 13).unwrap();

        assert_eq!(address_space.tia.bytes[0xA33F], 11);
        assert_eq!(address_space.ram.bytes[0xC59A], 12);
        assert_eq!(address_space.riot.bytes[0x86AB], 13);
    }
}
