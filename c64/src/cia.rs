use enum_map::{enum_map, Enum, EnumMap};
use ya6502::memory::Memory;
use ya6502::memory::Read;
use ya6502::memory::ReadError;
use ya6502::memory::Write;
use ya6502::memory::WriteError;

/// A 6526 Complex Interface Adapter chip.
#[derive(Debug, Default)]
pub struct Cia {
    reg_interrupt_control: u8,

    ports: EnumMap<Port, PortState>,
}

#[derive(Enum, Debug, Clone, Copy)]
pub enum Port {
    A,
    B,
}

impl Cia {
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes a given value to the pins of a given port.
    pub fn write_port(&mut self, port: Port, value: u8) {
        self.ports[port].pins = value;
    }

    /// Reads a value from the pins of a given port. The value takes into
    /// consideration the direction configuration for each particular bit.
    pub fn read_port(&self, port: Port) -> u8 {
        self.ports[port].read()
    }
}

impl Read for Cia {
    fn read(&self, address: u16) -> Result<u8, ReadError> {
        match address & 0b1111 {
            registers::PRA => Ok(self.ports[Port::A].read()),
            registers::PRB => Ok(self.ports[Port::B].read()),
            registers::DDRA => Ok(self.ports[Port::A].direction),
            registers::DDRB => Ok(self.ports[Port::B].direction),
            _ => Err(ReadError { address }),
        }
    }
}

impl Write for Cia {
    fn write(&mut self, address: u16, value: u8) -> Result<(), WriteError> {
        match address & 0b1111 {
            registers::PRA => self.ports[Port::A].register = value,
            registers::PRB => self.ports[Port::B].register = value,
            registers::DDRA => {
                self.ports[Port::A].direction = value;
            }
            registers::DDRB => {
                self.ports[Port::B].direction = value;
            }
            registers::ICR => {
                // For now, only allow disabling the interrupts.
                if value & flags::ICR_SOURCE_BIT != 0 {
                    return Err(WriteError { address, value });
                }
            }
            registers::CRA | registers::CRB => {
                // For now, only allow stopping timers.
                if value & flags::CRX_START != 0 {
                    return Err(WriteError { address, value });
                }
            }
            _ => return Err(WriteError { address, value }),
        };
        Ok(())
    }
}

impl Memory for Cia {}

/// An internal state of a CIA port.
#[derive(Debug, Default)]
struct PortState {
    /// A direction register: each bit controls the direction of a given pin.
    /// 0=input, 1=input/output.
    direction: u8,
    /// The peripheral data register: holds the value as set from the inside of
    /// the chip.
    register: u8,
    /// Value set from the outside to the pins.
    pins: u8,
}

impl PortState {
    /// Resolves the value on the pins. Assumes that bits where direction
    /// register is set to 1 are driven by the CIA chip, and everything else is
    /// driven from outside.
    fn read(&self) -> u8 {
        (self.register & self.direction) | (self.pins & !self.direction)
    }
}

mod registers {
    pub const PRA: u16 = 0x0;
    pub const PRB: u16 = 0x1;
    pub const DDRA: u16 = 0x2;
    pub const DDRB: u16 = 0x3;
    pub const ICR: u16 = 0xD;
    pub const CRA: u16 = 0xE;
    pub const CRB: u16 = 0xF;
}

mod flags {
    pub const ICR_SOURCE_BIT: u8 = 1 << 7;
    pub const CRX_START: u8 = 1 << 0;
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
        assert_eq!(cia.read_port(Port::A), 0b1010_1010);
        cia.write(registers::PRA, 0b0101_0101).unwrap();
        assert_eq!(cia.read_port(Port::A), 0b0101_0101);

        cia.write(registers::DDRB, 0b1111_1111).unwrap();
        cia.write(registers::PRB, 0b1111_0000).unwrap();
        assert_eq!(cia.read_port(Port::B), 0b1111_0000);
        cia.write(registers::PRB, 0b0000_1111).unwrap();
        assert_eq!(cia.read_port(Port::B), 0b0000_1111);
    }

    #[test]
    fn ports_input() {
        let mut cia = Cia::new();
        cia.write(registers::DDRA, 0b0000_0000).unwrap();
        cia.write_port(Port::A, 0b1100_1100);
        assert_eq!(cia.read(registers::PRA).unwrap(), 0b1100_1100);
        cia.write_port(Port::A, 0b0011_0011);
        assert_eq!(cia.read(registers::PRA).unwrap(), 0b0011_0011);

        cia.write(registers::DDRB, 0b0000_0000).unwrap();
        cia.write_port(Port::B, 0b1100_0011);
        assert_eq!(cia.read(registers::PRB).unwrap(), 0b1100_0011);
        cia.write_port(Port::B, 0b0011_1100);
        assert_eq!(cia.read(registers::PRB).unwrap(), 0b0011_1100);
    }

    #[test]
    fn port_data_direction() {
        let mut cia = Cia::new();
        cia.write(registers::DDRA, 0b1111_0000).unwrap();
        cia.write(registers::PRA, 0b1010_1010).unwrap();
        cia.write_port(Port::A, 0b0101_0101);
        assert_eq!(cia.read_port(Port::A), 0b1010_0101);
        assert_eq!(cia.read(registers::PRA).unwrap(), 0b1010_0101);
        cia.write(registers::DDRA, 0b0000_1111).unwrap();
        assert_eq!(cia.read_port(Port::A), 0b0101_1010);
        assert_eq!(cia.read(registers::PRA).unwrap(), 0b0101_1010);

        cia.write(registers::DDRB, 0b0011_1100).unwrap();
        cia.write(registers::PRB, 0b1010_1010).unwrap();
        assert_eq!(cia.read_port(Port::B), 0b0010_1000);
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
}
