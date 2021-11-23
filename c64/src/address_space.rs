use ya6502::memory::Ram;
use ya6502::memory::Read;
use ya6502::memory::ReadResult;
use ya6502::memory::Rom;
use ya6502::memory::Write;
use ya6502::memory::WriteResult;

pub struct AddressSpace {
    ram: Ram,
    // char_rom: Rom,
    cartridge_rom: Option<CartridgeRom>,
}

impl AddressSpace {
    pub fn new() -> Self {
        Self {
            ram: Ram::new(16),
            cartridge_rom: None,
        }
    }
}

impl Read for AddressSpace {
    fn read(&self, address: u16) -> ReadResult {
        match address {
            0x8000..=0x9FFF => match self.cartridge_rom.as_ref().unwrap() {
                CartridgeRom::Standard8k(rom) => rom.read(address),
                _ => todo!(),
            },
            _ => self.ram.read(address),
        }
    }
}

impl Write for AddressSpace {
    fn write(&mut self, address: u16, value: u8) -> WriteResult {
        self.ram.write(address, value)
    }
}

/// Types of cartridge ROM available in the C64 architecture.
pub enum CartridgeRom {
    /// Standard 8KiB cartridge ($8000-$9FFF)
    Standard8k(Rom),
    /// Standard 16KiB cartridge ($8000-$BFFF)
    Standard16k(Rom),
    /// Ultimax 16KiB cartridge ($8000-$9FFF, $E000-$FFFF).
    Ultimax(Rom),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes() {
        let mut address_space = AddressSpace::new();
        address_space.write(0x0002, 6).unwrap();

        assert_eq!(address_space.ram.read(0x0002).unwrap(), 6);
        assert_eq!(address_space.read(0x0002).unwrap(), 6);
    }

    // #[test]
    // fn cartridges() {
    //     let mut address_space = AddressSpace::new();
    //     address_space.cartridge_rom =
    //         Some(CartridgeRom::Standard8k(Rom::new(&[1; 0x2000], 0x7FFF)));

    //     assert_eq!(address_space.read(0x7FFF).unwrap(), 0);
    //     assert_eq!(address_space.read(0x8000).unwrap(), 1);
    //     assert_eq!(address_space.read(0x9FFF).unwrap(), 1);
    //     assert_eq!(address_space.read(0xA000).unwrap(), 0);
    // }
}
