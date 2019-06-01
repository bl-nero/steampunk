use crate::memory::RAM;
use crate::memory::Memory;
use crate::tia::TIA;

#[derive(Debug)]
pub struct AddressSpace {
    pub tia: RAM,
    pub ram: RAM,
    pub rom: RAM,
}

impl Memory for AddressSpace{
    fn read(&self, address: u16) -> u8 {
        if address <= 0x7f {
            return self.tia.read(address);
        }
        else{
            return self.ram.read(address);
        }
    }
    fn write(&mut self, address: u16, value: u8) {
        if address <= 0x7f {
            self.tia.write(address, value);
        }
        else{
        self.ram.write(address, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reads_and_writes_to_tia() {
        let mut address_space = AddressSpace{
            tia: RAM::new(&[]),
            ram: RAM::new(&[]),
            rom: RAM::new(&[]),
        };
        address_space.write(0, 8);
        address_space.write(0x7f,5);
        address_space.write(0x80, 81);
        address_space.write(0xff, 45);
        assert_eq!(address_space.tia.read(0),8);
        assert_eq!(address_space.tia.read(0x7f),5);
        assert_eq!(address_space.read(0),8);
        assert_eq!(address_space.read(0x7f),5);
        assert_eq!(address_space.ram.read(0x80),81);
        assert_eq!(address_space.ram.read(0xff),45);
        assert_eq!(address_space.read(0x80),81);
        assert_eq!(address_space.read(0xff),45);
    }
}
