use crate::memory::Memory;

/// Dispatches read/write calls to various devices with memory-mapped interfaces:
/// TIA, RAM, RIOT (not yet implemented), and ROM.
#[derive(Debug)]
pub struct AddressSpace<T: Memory, RA: Memory, RO: Memory> {
    pub tia: T,
    pub ram: RA,
    pub rom: RO,
}

impl<T: Memory, RA: Memory, RO: Memory> Memory for AddressSpace<T, RA, RO> {
    fn read(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x007F => self.tia.read(address),
            0x0080..=0x00FF => self.ram.read(address),
            0xF000..=0xFFFF => self.rom.read(address),
            _ => {
                println!("Attempt to read from unsupported address ${:04X}", address);
                0
            }
        }
    }

    fn write(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x007F => self.tia.write(address, value),
            0x0080..=0x00FF => self.ram.write(address, value),
            // Yeah, I know. Writing to ROM. But hey, it's not the
            // AddressSpace's job to tell what you can or can't do with a given
            // segment of memory.
            0xF000..=0xFFFF => self.rom.write(address, value),
            _ => println!(
                "Attempt to write ${:02X} to unsupported address ${:04X}",
                value, address
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::RAM;

    #[test]
    fn reads_and_writes() {
        let mut address_space = AddressSpace {
            tia: RAM::new(),
            ram: RAM::new(),
            rom: RAM::new(),
        };
        address_space.write(0, 8); // Start of TIA
        address_space.write(0x7f, 5); // End of TIA
        address_space.write(0x80, 81); // Start of RAM
        address_space.write(0xff, 45); // End of RAM
        address_space.write(0xf000, 15); // Start of ROM
        address_space.write(0xffff, 25); // End of ROM

        assert_eq!(address_space.tia.read(0), 8);
        assert_eq!(address_space.tia.read(0x7f), 5);
        assert_eq!(address_space.read(0), 8);
        assert_eq!(address_space.read(0x7f), 5);

        assert_eq!(address_space.ram.read(0x80), 81);
        assert_eq!(address_space.ram.read(0xff), 45);
        assert_eq!(address_space.read(0x80), 81);
        assert_eq!(address_space.read(0xff), 45);

        assert_eq!(address_space.rom.read(0xf000), 15);
        assert_eq!(address_space.rom.read(0xffff), 25);
        assert_eq!(address_space.read(0xf000), 15);
        assert_eq!(address_space.read(0xffff), 25);
    }

    struct NoMemoryAtAll {}
    impl Memory for NoMemoryAtAll {
        fn read(&self, address: u16) -> u8 {
            panic!("Illegal attempt to read from address 0x{:04X}", address);
        }
        fn write(&mut self, address: u16, _value: u8) {
            panic!("Illegal attempt to write to address 0x{:04X}", address);
        }
    }

    #[test]
    fn does_not_access_illegal_addresses() {
        let mut address_space = AddressSpace {
            tia: NoMemoryAtAll {},
            ram: NoMemoryAtAll {},
            rom: NoMemoryAtAll {},
        };

        // These operations should silently pass without panic.
        address_space.write(0x0123, 12);
        address_space.write(0x089A, 13);
        address_space.write(0x2345, 14);

        assert_eq!(address_space.read(0x0123), 0);
        assert_eq!(address_space.read(0x089A), 0);
        assert_eq!(address_space.read(0x2345), 0);
    }
}
