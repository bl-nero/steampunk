use crate::port::Port;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use ya6502::memory::dump_zero_page;
use ya6502::memory::Memory;
use ya6502::memory::Ram;
use ya6502::memory::Read;
use ya6502::memory::ReadError;
use ya6502::memory::ReadResult;
use ya6502::memory::Rom;
use ya6502::memory::Write;
use ya6502::memory::WriteError;
use ya6502::memory::WriteResult;

/// A C64 address space, as visible from the 6510 CPU perspective, through the
/// C64 PLA chip. Note that technically, it also will handle the CPU port
/// (addresses 0x0000 and 0x0001), although it should technically be handled by
/// the CPU itself. This is because the CPU port controls the address space
/// layout.
#[derive(Debug)]
pub struct AddressSpace<VIC: Memory, SID: Memory, CIA: Memory> {
    cpu_port: Port,
    ram: Rc<RefCell<Ram>>,
    basic_rom: Rom,
    vic: VIC,
    sid: SID,
    color_ram: Rc<RefCell<Ram>>, // TODO: replace with an actual single-nibble RAM
    cia1: CIA,
    cia2: CIA,
    kernal_rom: Rom,
    pub cartridge: Option<Cartridge>,
}

impl<VIC: Memory, SID: Memory, CIA: Memory> AddressSpace<VIC, SID, CIA> {
    pub fn mut_vic(&mut self) -> &mut VIC {
        &mut self.vic
    }
    pub fn mut_cia1(&mut self) -> &mut CIA {
        &mut self.cia1
    }
}

impl<VIC: Memory, SID: Memory, CIA: Memory> AddressSpace<VIC, SID, CIA> {
    pub fn new(
        ram: Rc<RefCell<Ram>>,
        basic_rom: Rom,
        vic: VIC,
        sid: SID,
        color_ram: Rc<RefCell<Ram>>,
        cia1: CIA,
        cia2: CIA,
        kernal_rom: Rom,
    ) -> Self {
        let mut cpu_port = Port::default();
        // Set the default values of the CPU port pins. Bits 0-2 and 4 are set
        // to 1 by pull-up registers. Note that the behavior of bits 3 (dangling
        // if no Datasette) and 5 (attempting to read from the motor output
        // driver) are just wild guess, but mostly irrelevant.
        cpu_port.pins = 0b0011_0111;
        return Self {
            cpu_port,
            ram,
            basic_rom,
            vic,
            sid,
            color_ram,
            cia1,
            cia2,
            kernal_rom,
            cartridge: None,
        };
    }
}

impl<VIC: Memory, SID: Memory, CIA: Memory> Read for AddressSpace<VIC, SID, CIA> {
    fn read(&self, address: u16) -> ReadResult {
        match address {
            0x0000 => Ok(self.cpu_port.direction),
            0x0001 => Ok(self.cpu_port.read()),
            0x8000..=0x9FFF => match &self.cartridge {
                Some(Cartridge { mode: _, rom }) => rom.read(address),
                _ => self.ram.borrow().read(address),
            },
            0xA000..=0xBFFF => match &self.cartridge {
                Some(Cartridge {
                    mode: CartridgeMode::Standard16k,
                    rom,
                }) => rom.read(address),
                _ => self.basic_rom.read(address),
            },
            0xD000..=0xD3FF => self.vic.read(address),
            0xD400..=0xD7FF => self.sid.read(address),
            0xD800..=0xDBFF => self.color_ram.borrow().read(address),
            0xDC00..=0xDCFF => self.cia1.read(address),
            0xDD00..=0xDDFF => self.cia2.read(address),
            0xDE00..=0xDFFF => Err(ReadError { address }),
            0xE000..=0xFFFF => match &self.cartridge {
                Some(Cartridge {
                    mode: CartridgeMode::Ultimax,
                    rom,
                }) => rom.read(address),
                _ => self.kernal_rom.read(address),
            },
            _ => self.ram.borrow().read(address),
        }
    }
}

impl<VIC: Memory, SID: Memory, CIA: Memory> Write for AddressSpace<VIC, SID, CIA> {
    fn write(&mut self, address: u16, value: u8) -> WriteResult {
        match address {
            0x0000 => Ok(self.cpu_port.direction = value),
            0x0001 => {
                // For now, only allow one memory layout and don't allow turning
                // Datasette on.
                if value & 0b0010_0111 == 0b0010_0111 {
                    Ok(self.cpu_port.register = value)
                } else {
                    Err(WriteError { address, value })
                }
            }
            0xD000..=0xD3FF => self.vic.write(address, value),
            0xD400..=0xD7FF => self.sid.write(address, value),
            0xD800..=0xDBFF => self.color_ram.borrow_mut().write(address, value),
            0xDC00..=0xDCFF => self.cia1.write(address, value),
            0xDD00..=0xDDFF => self.cia2.write(address, value),
            0xDE00..=0xDFFF => Err(WriteError { address, value }),
            _ => self.ram.borrow_mut().write(address, value),
        }
    }
}

impl<VIC: Memory, SID: Memory, CIA: Memory> Memory for AddressSpace<VIC, SID, CIA> {}

impl<VIC: Memory, SID: Memory, CIA: Memory> fmt::Display for AddressSpace<VIC, SID, CIA> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        dump_zero_page(self, f)
    }
}

#[derive(Debug)]
pub struct Cartridge {
    pub mode: CartridgeMode,
    pub rom: Rom,
}

/// Types of cartridge ROM available in the C64 architecture.
#[derive(Debug)]
pub enum CartridgeMode {
    /// Standard 8KiB cartridge ($8000-$9FFF)
    #[allow(dead_code)]
    Standard8k,
    /// Standard 16KiB cartridge ($8000-$BFFF)
    #[allow(dead_code)]
    Standard16k,
    /// Ultimax 16KiB cartridge ($8000-$9FFF, $E000-$FFFF).
    Ultimax,
}

/// An address space, as visible by the VIC-II chip. Note that it doesn't
/// include the Color RAM, since it's addressed using a separate address line.
#[derive(Debug)]
pub struct VicAddressSpace<RAM: Read, CHR: Read> {
    ram: Rc<RefCell<RAM>>,
    char_rom: Rc<RefCell<CHR>>,
}

impl<RAM: Read, CHR: Read> VicAddressSpace<RAM, CHR> {
    pub fn new(ram: Rc<RefCell<RAM>>, char_rom: Rc<RefCell<CHR>>) -> Self {
        Self { ram, char_rom }
    }
}

impl<RAM: Read, CHR: Read> Read for VicAddressSpace<RAM, CHR> {
    fn read(&self, address: u16) -> ReadResult {
        let address = address & 0x3FFF;
        match address {
            0x1000..=0x1FFF => self.char_rom.borrow().read(address),
            _ => self.ram.borrow().read(address),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_address_space() -> AddressSpace<Ram, Ram, Ram> {
        AddressSpace::new(
            Rc::new(RefCell::new(Ram::new(16))),
            Rom::new(&[0xBA; 0x2000]).unwrap(),
            Ram::new(10),
            Ram::new(10),
            Rc::new(RefCell::new(Ram::new(10))),
            Ram::new(8),
            Ram::new(8),
            Rom::new(&[0xA1; 0x2000]).unwrap(),
        )
    }

    fn new_vic_address_space() -> VicAddressSpace<Ram, Rom> {
        VicAddressSpace::new(
            Rc::new(RefCell::new(Ram::new(16))),
            Rc::new(RefCell::new(Rom::new(&[0xCC; 0x1000]).unwrap())),
        )
    }

    #[test]
    fn reads_and_writes() {
        let mut address_space = new_address_space();
        address_space.write(0x0002, 33).unwrap(); // RAM
        address_space.write(0x9FFF, 65).unwrap(); // RAM
        address_space.write(0xA000, 82).unwrap(); // RAM under BASIC ROM
        address_space.write(0xBFFF, 67).unwrap(); // RAM under BASIC ROM
        address_space.write(0xC000, 143).unwrap(); // RAM
        address_space.write(0xCFFF, 213).unwrap(); // RAM
        address_space.write(0xD000, 73).unwrap(); // VIC
        address_space.write(0xD3FF, 11).unwrap(); // VIC
        address_space.write(0xD400, 178).unwrap(); // SID
        address_space.write(0xD7FF, 132).unwrap(); // SID
        address_space.write(0xD800, 5).unwrap(); // Color RAM
        address_space.write(0xDBFF, 15).unwrap(); // Color RAM
        address_space.write(0xDC00, 78).unwrap(); // CIA1
        address_space.write(0xDCFF, 79).unwrap(); // CIA1
        address_space.write(0xDD00, 88).unwrap(); // CIA2
        address_space.write(0xDDFF, 89).unwrap(); // CIA2
        address_space.write(0xE000, 87).unwrap(); // RAM under KERNEL ROM
        address_space.write(0xFFFF, 45).unwrap(); // RAM under KERNEL ROM

        // RAM
        assert_eq!(address_space.ram.borrow().read(0x0002).unwrap(), 33);
        assert_eq!(address_space.read(0x0002).unwrap(), 33);
        assert_eq!(address_space.ram.borrow().read(0x9FFF).unwrap(), 65);
        assert_eq!(address_space.read(0x9FFF).unwrap(), 65);

        // BASIC ROM
        assert_eq!(address_space.read(0xA000).unwrap(), 0xBA);
        assert_eq!(address_space.read(0xBFFF).unwrap(), 0xBA);

        // RAM under BASIC ROM
        assert_eq!(address_space.ram.borrow().read(0xA000).unwrap(), 82);
        assert_eq!(address_space.ram.borrow().read(0xBFFF).unwrap(), 67);

        // RAM
        assert_eq!(address_space.ram.borrow().read(0xC000).unwrap(), 143);
        assert_eq!(address_space.read(0xC000).unwrap(), 143);
        assert_eq!(address_space.ram.borrow().read(0xCFFF).unwrap(), 213);
        assert_eq!(address_space.read(0xCFFF).unwrap(), 213);

        // VIC
        assert_eq!(address_space.vic.read(0x0).unwrap(), 73);
        assert_eq!(address_space.read(0xD000).unwrap(), 73);
        assert_eq!(address_space.vic.read(0x3FF).unwrap(), 11);
        assert_eq!(address_space.read(0xD3FF).unwrap(), 11);

        // SID
        assert_eq!(address_space.sid.read(0x0).unwrap(), 178);
        assert_eq!(address_space.read(0xD400).unwrap(), 178);
        assert_eq!(address_space.sid.read(0x3FF).unwrap(), 132);
        assert_eq!(address_space.read(0xD7FF).unwrap(), 132);

        // Color RAM
        assert_eq!(address_space.color_ram.borrow().read(0xD800).unwrap(), 5);
        assert_eq!(address_space.read(0xD800).unwrap(), 5);
        assert_eq!(address_space.color_ram.borrow().read(0xDBFF).unwrap(), 15);
        assert_eq!(address_space.read(0xDBFF).unwrap(), 15);

        // CIA1
        assert_eq!(address_space.cia1.read(0x0).unwrap(), 78);
        assert_eq!(address_space.read(0xDC00).unwrap(), 78);
        assert_eq!(address_space.cia1.read(0xFF).unwrap(), 79);
        assert_eq!(address_space.read(0xDCFF).unwrap(), 79);

        // CIA2
        assert_eq!(address_space.cia2.read(0x0).unwrap(), 88);
        assert_eq!(address_space.read(0xDD00).unwrap(), 88);
        assert_eq!(address_space.cia2.read(0xFF).unwrap(), 89);
        assert_eq!(address_space.read(0xDDFF).unwrap(), 89);

        // KERNEL ROM
        assert_eq!(address_space.read(0xE000).unwrap(), 0xA1);
        assert_eq!(address_space.read(0xFFFF).unwrap(), 0xA1);

        // RAM under KERNEL ROM
        assert_eq!(address_space.ram.borrow().read(0xE000).unwrap(), 87);
        assert_eq!(address_space.ram.borrow().read(0xFFFF).unwrap(), 45);
    }

    #[test]
    fn cartridge_8k() {
        let mut address_space = new_address_space();
        address_space.cartridge = Some(Cartridge {
            mode: CartridgeMode::Standard8k,
            rom: Rom::new(&[1; 0x10000]).unwrap(),
        });

        assert_eq!(address_space.read(0x7FFF).unwrap(), 0);
        assert_eq!(address_space.read(0x8000).unwrap(), 1);
        assert_eq!(address_space.read(0x9FFF).unwrap(), 1);
        assert_eq!(address_space.read(0xA000).unwrap(), 0xBA);
    }

    #[test]
    fn cartridge_16k() {
        let mut address_space = new_address_space();
        address_space.cartridge = Some(Cartridge {
            mode: CartridgeMode::Standard16k,
            rom: Rom::new(&[2; 0x10000]).unwrap(),
        });

        assert_eq!(address_space.read(0x7FFF).unwrap(), 0);
        assert_eq!(address_space.read(0x8000).unwrap(), 2);
        assert_eq!(address_space.read(0xA000).unwrap(), 2);
        assert_eq!(address_space.read(0xBFFF).unwrap(), 2);
        assert_eq!(address_space.read(0xC000).unwrap(), 0);
    }

    #[test]
    fn cartridge_ultimax() {
        let mut address_space = new_address_space();
        address_space.cartridge = Some(Cartridge {
            mode: CartridgeMode::Ultimax,
            rom: Rom::new(&[3; 0x10000]).unwrap(),
        });

        assert_eq!(address_space.read(0x7FFF).unwrap(), 0);
        assert_eq!(address_space.read(0x8000).unwrap(), 3);
        assert_eq!(address_space.read(0x9FFF).unwrap(), 3);
        assert_eq!(address_space.read(0xA000).unwrap(), 0xBA);
        // assert_eq!(address_space.read(0xDFFF).unwrap(), 0);
        assert_eq!(address_space.read(0xE000).unwrap(), 3);
        assert_eq!(address_space.read(0xFFFF).unwrap(), 3);
        assert_eq!(address_space.read(0x0000).unwrap(), 0);
    }

    #[test]
    fn cpu_port_direction() {
        let mut address_space = new_address_space();
        // Set the data direction to "all inputs". The external pull-up
        // resistors should keep some of the bits high.
        address_space.write(0x0000, 0b0000_0000).unwrap();
        assert_eq!(address_space.read(0x0001).unwrap(), 0b0011_0111);

        // Force bit 4 to 0.
        address_space.write(0x0001, 0b0010_0111).unwrap();
        address_space.write(0x0000, 0b0001_0000).unwrap();
        assert_eq!(address_space.read(0x0001).unwrap(), 0b0010_0111);
    }

    #[test]
    fn vic_reads() {
        let address_space = new_vic_address_space();
        address_space.ram.borrow_mut().write(0x0000, 165).unwrap(); // RAM
        address_space.ram.borrow_mut().write(0x0FFF, 212).unwrap(); // RAM
        address_space.ram.borrow_mut().write(0x2000, 96).unwrap(); // RAM
        address_space.ram.borrow_mut().write(0x3FFF, 68).unwrap(); // RAM

        // RAM
        assert_eq!(address_space.read(0x0000).unwrap(), 165);
        assert_eq!(address_space.read(0x0FFF).unwrap(), 212);

        // Char ROM
        assert_eq!(address_space.read(0x1000).unwrap(), 0xCC);
        assert_eq!(address_space.read(0x1FFF).unwrap(), 0xCC);

        // RAM
        assert_eq!(address_space.read(0x2000).unwrap(), 96);
        assert_eq!(address_space.read(0x3FFF).unwrap(), 68);
    }

    #[test]
    fn vic_mirroring() {
        let address_space = new_vic_address_space();
        address_space.ram.borrow_mut().write(0x2345, 12).unwrap();
        assert_eq!(address_space.read(0x6345).unwrap(), 12);
        assert_eq!(address_space.read(0xA345).unwrap(), 12);
        assert_eq!(address_space.read(0xE345).unwrap(), 12);
    }
}
