use crate::port::Port;
use crate::timer::Timer;
use enum_map::{Enum, EnumMap};
use ya6502::memory::Memory;
use ya6502::memory::Read;
use ya6502::memory::ReadError;
use ya6502::memory::Write;
use ya6502::memory::WriteError;

/// A 6526 Complex Interface Adapter chip.
#[derive(Debug, Default)]
pub struct Cia {
    reg_interrupt_control: u8,

    ports: EnumMap<PortName, Port>,
    timer_a: Timer,
}

#[derive(Enum, Debug, Clone, Copy)]
pub enum PortName {
    A,
    B,
}

impl Cia {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(&mut self) {
        self.timer_a.tick();
    }

    /// Writes a given value to the pins of a given port.
    #[cfg(test)]
    pub fn write_port(&mut self, port_name: PortName, value: u8) {
        self.ports[port_name].pins = value;
    }

    /// Reads a value from the pins of a given port. The value takes into
    /// consideration the direction configuration for each particular bit.
    #[cfg(test)]
    pub fn read_port(&self, port_name: PortName) -> u8 {
        self.ports[port_name].read()
    }
}

impl Read for Cia {
    fn read(&self, address: u16) -> Result<u8, ReadError> {
        match address & 0b1111 {
            registers::PRA => Ok(self.ports[PortName::A].read()),
            registers::PRB => Ok(self.ports[PortName::B].read()),
            registers::DDRA => Ok(self.ports[PortName::A].direction),
            registers::DDRB => Ok(self.ports[PortName::B].direction),
            registers::TA_LO => Ok((self.timer_a.counter() & 0xFF) as u8),
            registers::TA_HI => Ok(((self.timer_a.counter() & 0xFF00) >> 8) as u8),
            registers::CRA => Ok(self.timer_a.control()),
            _ => Err(ReadError { address }),
        }
    }
}

impl Write for Cia {
    fn write(&mut self, address: u16, value: u8) -> Result<(), WriteError> {
        match address & 0b1111 {
            registers::PRA => self.ports[PortName::A].register = value,
            registers::PRB => self.ports[PortName::B].register = value,
            registers::DDRA => {
                self.ports[PortName::A].direction = value;
            }
            registers::DDRB => {
                self.ports[PortName::B].direction = value;
            }
            registers::TA_LO => self
                .timer_a
                .set_latch(self.timer_a.latch() & 0xFF00 | value as u16),
            registers::TA_HI => self
                .timer_a
                .set_latch(self.timer_a.latch() & 0xFF | (value as u16) << 8),
            registers::ICR => {
                // For now, only allow disabling the interrupts.
                if value & flags::ICR_SOURCE_BIT != 0 {
                    return Err(WriteError { address, value });
                }
            }
            registers::CRA => {
                if self.timer_a.set_control(value).is_err() {
                    return Err(WriteError { address, value });
                }
            }
            _ => return Err(WriteError { address, value }),
        };
        Ok(())
    }
}

impl Memory for Cia {}

#[allow(dead_code)]
mod registers {
    pub const PRA: u16 = 0x0;
    pub const PRB: u16 = 0x1;
    pub const DDRA: u16 = 0x2;
    pub const DDRB: u16 = 0x3;
    pub const TA_LO: u16 = 0x4;
    pub const TA_HI: u16 = 0x5;
    pub const ICR: u16 = 0xD;
    pub const CRA: u16 = 0xE;
    pub const CRB: u16 = 0xF;
}

mod flags {
    pub const ICR_SOURCE_BIT: u8 = 1 << 7;
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn disabling_interrupts() {}

    #[test]
    fn ports_output() {
        let mut cia = Cia::new();
        cia.write(registers::DDRA, 0b1111_1111).unwrap();
        cia.write(registers::PRA, 0b1010_1010).unwrap();
        assert_eq!(cia.read_port(PortName::A), 0b1010_1010);
        cia.write(registers::PRA, 0b0101_0101).unwrap();
        assert_eq!(cia.read_port(PortName::A), 0b0101_0101);

        cia.write(registers::DDRB, 0b1111_1111).unwrap();
        cia.write(registers::PRB, 0b1111_0000).unwrap();
        assert_eq!(cia.read_port(PortName::B), 0b1111_0000);
        cia.write(registers::PRB, 0b0000_1111).unwrap();
        assert_eq!(cia.read_port(PortName::B), 0b0000_1111);
    }

    #[test]
    fn ports_input() {
        let mut cia = Cia::new();
        cia.write(registers::DDRA, 0b0000_0000).unwrap();
        cia.write_port(PortName::A, 0b1100_1100);
        assert_eq!(cia.read(registers::PRA).unwrap(), 0b1100_1100);
        cia.write_port(PortName::A, 0b0011_0011);
        assert_eq!(cia.read(registers::PRA).unwrap(), 0b0011_0011);

        cia.write(registers::DDRB, 0b0000_0000).unwrap();
        cia.write_port(PortName::B, 0b1100_0011);
        assert_eq!(cia.read(registers::PRB).unwrap(), 0b1100_0011);
        cia.write_port(PortName::B, 0b0011_1100);
        assert_eq!(cia.read(registers::PRB).unwrap(), 0b0011_1100);
    }

    #[test]
    fn port_data_direction() {
        let mut cia = Cia::new();
        cia.write(registers::DDRA, 0b1111_0000).unwrap();
        cia.write(registers::PRA, 0b1010_1010).unwrap();
        cia.write_port(PortName::A, 0b0101_0101);
        assert_eq!(cia.read_port(PortName::A), 0b1010_0101);
        assert_eq!(cia.read(registers::PRA).unwrap(), 0b1010_0101);
        cia.write(registers::DDRA, 0b0000_1111).unwrap();
        assert_eq!(cia.read_port(PortName::A), 0b0101_1010);
        assert_eq!(cia.read(registers::PRA).unwrap(), 0b0101_1010);

        cia.write(registers::DDRB, 0b0011_1100).unwrap();
        cia.write(registers::PRB, 0b1010_1010).unwrap();
        assert_eq!(cia.read_port(PortName::B), 0b0010_1000);
    }

    #[test]
    fn address_mirroring() {
        let mut cia = Cia::new();
        cia.write(registers::DDRB, 0x12).unwrap();
        assert_eq!(cia.read(registers::DDRB + 0x0010).unwrap(), 0x12);
        assert_eq!(cia.read(registers::DDRB + 0x5A70).unwrap(), 0x12);
        assert_eq!(cia.read(registers::DDRB + 0xFFF0).unwrap(), 0x12);

        cia.write(registers::DDRA + 0x8740, 0x13).unwrap();
        assert_eq!(cia.read(registers::DDRA).unwrap(), 0x13);
        cia.write(registers::DDRA + 0xFFF0, 0x14).unwrap();
        assert_eq!(cia.read(registers::DDRA).unwrap(), 0x14);
    }

    #[test]
    fn timers() {
        use crate::timer::flags::*;

        let mut cia = Cia::new();
        cia.write(registers::TA_HI, 0x23).unwrap();
        cia.write(registers::TA_LO, 0x01).unwrap(); // Load 0x2301
        cia.write(registers::CRA, 0).unwrap(); // Don't start yet

        cia.write(registers::CRA, LOAD | START).unwrap();
        cia.tick();
        cia.tick();
        cia.tick();
        assert_eq!(cia.read(registers::CRA).unwrap(), START);
        assert_eq!(cia.read(registers::TA_HI).unwrap(), 0x22);
        assert_eq!(cia.read(registers::TA_LO).unwrap(), 0xFE);
    }
}
