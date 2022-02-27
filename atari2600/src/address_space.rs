use std::fmt;
use ya6502::memory::dump_zero_page;
use ya6502::memory::Inspect;
use ya6502::memory::Read;
use ya6502::memory::Write;
use ya6502::memory::{Memory, ReadError, ReadResult, WriteError, WriteResult};

/// Dispatches read/write calls to various devices with memory-mapped interfaces:
/// TIA, RAM, RIOT (not yet implemented), and ROM.
#[derive(Debug)]
pub struct AddressSpace<T, Ram, Riot, Rom>
where
    T: Memory,
    Ram: Memory,
    Riot: Memory,
    Rom: Read,
{
    pub tia: T,
    pub ram: Ram,
    pub riot: Riot,
    pub rom: Rom,
}

enum MemoryArea {
    Tia,
    Ram,
    Riot,
    Rom,
}

impl<T, Ram, Riot, Rom> Inspect for AddressSpace<T, Ram, Riot, Rom>
where
    T: Memory + Inspect,
    Ram: Memory + Inspect,
    Riot: Memory + Inspect,
    Rom: Read + Inspect,
{
    fn inspect(&self, address: u16) -> ReadResult {
        match map_address(address) {
            Some(MemoryArea::Tia) => self.tia.inspect(address),
            Some(MemoryArea::Ram) => self.ram.inspect(address),
            Some(MemoryArea::Rom) => self.rom.inspect(address),
            Some(MemoryArea::Riot) => self.riot.inspect(address),
            None => Err(ReadError { address }),
        }
    }
}

impl<T, Ram, Riot, Rom> Read for AddressSpace<T, Ram, Riot, Rom>
where
    T: Memory,
    Ram: Memory,
    Riot: Memory,
    Rom: Read,
{
    fn read(&mut self, address: u16) -> ReadResult {
        match map_address(address) {
            Some(MemoryArea::Tia) => self.tia.read(address),
            Some(MemoryArea::Ram) => self.ram.read(address),
            Some(MemoryArea::Rom) => self.rom.read(address),
            Some(MemoryArea::Riot) => self.riot.read(address),
            None => Err(ReadError { address }),
        }
    }
}

impl<T, Ram, Riot, Rom> Write for AddressSpace<T, Ram, Riot, Rom>
where
    T: Memory,
    Ram: Memory,
    Riot: Memory,
    Rom: Read,
{
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

impl<T, Ram, Riot, Rom> Memory for AddressSpace<T, Ram, Riot, Rom>
where
    T: Memory,
    Ram: Memory,
    Riot: Memory,
    Rom: Read,
{
}

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

impl<T, Ram, Riot, Rom> fmt::Display for AddressSpace<T, Ram, Riot, Rom>
where
    T: Memory + Inspect,
    Ram: Memory + Inspect,
    Riot: Memory + Inspect,
    Rom: Read + Inspect,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        dump_zero_page(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error;
    use ya6502::memory::Ram;

    #[test]
    fn reads_and_writes() -> Result<(), Box<dyn error::Error>> {
        let mut address_space = AddressSpace {
            tia: Ram::new(16),
            ram: Ram::new(16),
            riot: Ram::new(16),
            rom: Ram::new(16),
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
            tia: Ram::initialized_with(1, 16),
            ram: Ram::initialized_with(2, 16),
            riot: Ram::initialized_with(3, 16),
            rom: Ram::initialized_with(4, 16),
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
