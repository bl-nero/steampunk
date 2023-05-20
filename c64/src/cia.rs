use crate::port::Port;
use crate::timer::Timer;
use enum_map::{Enum, EnumMap};
use ya6502::memory::Inspect;
use ya6502::memory::Memory;
use ya6502::memory::Read;
use ya6502::memory::ReadError;
use ya6502::memory::Write;
use ya6502::memory::WriteError;

/// A 6526 Complex Interface Adapter chip.
#[derive(Debug, Default)]
pub struct Cia {
    reg_interrupt_control: u8,
    reg_interrupt_status: u8,

    ports: EnumMap<PortName, Port>,
    timer_a: Timer,
    timer_b: Timer,
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

    /// Performs a tick and returns `true` if an interrupt was triggered.
    pub fn tick(&mut self) -> bool {
        if self.timer_a.tick() {
            self.set_interrupt_flag(flags::ICR_TIMER_A);
        }
        if self.timer_b.tick() {
            self.set_interrupt_flag(flags::ICR_TIMER_B);
        }
        return self.reg_interrupt_status & flags::ICR_TRIGGERED != 0;
    }

    /// Writes a given value to the pins of a given port.
    pub fn write_port(&mut self, port_name: PortName, value: u8) {
        self.ports[port_name].pins = value;
    }

    /// Reads a value from the pins of a given port. The value takes into
    /// consideration the direction configuration for each particular bit.
    pub fn read_port(&self, port_name: PortName) -> u8 {
        self.ports[port_name].read()
    }

    /// Indicates a falling edge happening on the /FLAG pin.
    pub fn set_flag(&mut self) {
        self.set_interrupt_flag(flags::ICR_FLAG_SIGNAL);
    }

    /// Indicates that an interrupt condition indicated by the `icr_flag`
    /// parameter has been triggered. If the flag is allowed to trigger an
    /// interrupt, it will be triggered by setting appropriate bit in the
    /// interrupt status register.
    fn set_interrupt_flag(&mut self, icr_flag: u8) {
        let bits_to_set = if self.reg_interrupt_control & icr_flag != 0 {
            icr_flag | flags::ICR_TRIGGERED
        } else {
            icr_flag
        };
        self.reg_interrupt_status |= bits_to_set;
    }
}

impl Inspect for Cia {
    fn inspect(&self, address: u16) -> Result<u8, ReadError> {
        match address & 0b1111 {
            registers::PRA => Ok(self.ports[PortName::A].read()),
            registers::PRB => Ok(self.ports[PortName::B].read()),
            registers::DDRA => Ok(self.ports[PortName::A].direction),
            registers::DDRB => Ok(self.ports[PortName::B].direction),
            registers::TA_LO => Ok((self.timer_a.counter() & 0xFF) as u8),
            registers::TA_HI => Ok(((self.timer_a.counter() & 0xFF00) >> 8) as u8),
            registers::TB_LO => Ok((self.timer_b.counter() & 0xFF) as u8),
            registers::TB_HI => Ok(((self.timer_b.counter() & 0xFF00) >> 8) as u8),
            registers::ICR => Ok(self.reg_interrupt_status),
            registers::CRA => Ok(self.timer_a.control()),
            registers::CRB => Ok(self.timer_b.control()),
            _ => Err(ReadError { address }),
        }
    }
}

impl Read for Cia {
    fn read(&mut self, address: u16) -> Result<u8, ReadError> {
        match address & 0b1111 {
            registers::ICR => Ok(std::mem::take(&mut self.reg_interrupt_status)),
            _ => self.inspect(address),
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
            registers::TB_LO => self
                .timer_b
                .set_latch(self.timer_b.latch() & 0xFF00 | value as u16),
            registers::TB_HI => self
                .timer_b
                .set_latch(self.timer_b.latch() & 0xFF | (value as u16) << 8),
            registers::ICR => {
                if value & flags::ICR_SOURCE_BIT != 0 {
                    // Set mask bits.
                    // For now, only allow turning on timer and FLAG IRQs.
                    if value
                        & !(flags::ICR_TIMER_A
                            | flags::ICR_TIMER_B
                            | flags::ICR_FLAG_SIGNAL
                            | flags::ICR_SOURCE_BIT)
                        != 0
                    {
                        return Err(WriteError { address, value });
                    }
                    self.reg_interrupt_control |= value;
                } else {
                    self.reg_interrupt_control &= !value;
                }
            }
            registers::CRA => {
                if self.timer_a.set_control(value).is_err() {
                    return Err(WriteError { address, value });
                }
            }
            registers::CRB => {
                if self.timer_b.set_control(value).is_err() {
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
    pub const TB_LO: u16 = 0x6;
    pub const TB_HI: u16 = 0x7;
    pub const ICR: u16 = 0xD;
    pub const CRA: u16 = 0xE;
    pub const CRB: u16 = 0xF;
}

mod flags {
    pub const ICR_TIMER_A: u8 = 1 << 0;
    pub const ICR_TIMER_B: u8 = 1 << 1;
    pub const ICR_FLAG_SIGNAL: u8 = 1 << 4;
    pub const ICR_TRIGGERED: u8 = 1 << 7;
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

    macro_rules! test_timer {
        (
            $fn_name_basics:ident,
            $fn_name_underflow:ident,
            $fn_name_underflow_interrupt:ident,
            $reg_lo:expr,
            $reg_hi:expr,
            $reg_cr:expr,
            $icr_flag:expr
        ) => {
            #[test]
            fn $fn_name_basics() {
                use crate::timer::flags::*;

                let mut cia = Cia::new();
                cia.write($reg_hi, 0x23).unwrap();
                cia.write($reg_lo, 0x01).unwrap(); // Load 0x2301
                cia.write($reg_cr, LOAD | START).unwrap();

                cia.tick();
                cia.tick();
                cia.tick();
                assert_eq!(cia.read($reg_cr).unwrap(), START);
                assert_eq!(cia.read($reg_hi).unwrap(), 0x22);
                assert_eq!(cia.read($reg_lo).unwrap(), 0xFE);
            }

            #[test]
            fn $fn_name_underflow() {
                use crate::timer::flags::*;

                let mut cia = Cia::new();
                cia.write($reg_hi, 0x00).unwrap();
                cia.write($reg_lo, 0x01).unwrap(); // Load 0x0001
                cia.write($reg_cr, LOAD | START).unwrap();
                assert_eq!(cia.read(registers::ICR).unwrap(), 0);

                cia.tick();
                assert_eq!(cia.read($reg_lo).unwrap(), 0);
                assert_eq!(cia.read(registers::ICR).unwrap(), 0);

                cia.tick();
                assert_eq!(cia.read($reg_lo).unwrap(), 1);
                assert_eq!(cia.read(registers::ICR).unwrap(), $icr_flag);
                // Reading should have reset the register.
                assert_eq!(cia.read(registers::ICR).unwrap(), 0);
            }

            #[test]
            fn $fn_name_underflow_interrupt() {
                use crate::timer::flags::*;

                let mut cia = Cia::new();
                cia.write($reg_hi, 0x00).unwrap();
                cia.write($reg_lo, 0x01).unwrap(); // Load 0x0001

                // No interrupts.
                cia.write($reg_cr, LOAD | START | RUNMODE_ONE_SHOT).unwrap();
                cia.write(registers::ICR, $icr_flag).unwrap();
                assert_eq!(cia.read(registers::ICR).unwrap(), 0);
                assert_eq!(cia.tick(), false);
                assert_eq!(cia.tick(), false);
                assert_eq!(cia.read(registers::ICR).unwrap(), $icr_flag);

                // Enable interrupts.
                cia.write(registers::ICR, flags::ICR_SOURCE_BIT | $icr_flag)
                    .unwrap();
                assert_eq!(cia.read(registers::ICR).unwrap(), 0);
                cia.write($reg_cr, LOAD | START | RUNMODE_ONE_SHOT).unwrap();
                assert_eq!(cia.tick(), false);
                assert_eq!(cia.tick(), true);
                assert_eq!(cia.tick(), true); // Report IRQ until acknowledged.
                assert_eq!(
                    cia.read(registers::ICR).unwrap(),
                    flags::ICR_TRIGGERED | $icr_flag
                );
                assert_eq!(cia.tick(), false);
                assert_eq!(cia.read(registers::ICR).unwrap(), 0);

                // Disable interrupts again.
                cia.write(registers::ICR, $icr_flag).unwrap();
                cia.write($reg_cr, LOAD | START | RUNMODE_ONE_SHOT).unwrap();
                assert_eq!(cia.read(registers::ICR).unwrap(), 0);
                assert_eq!(cia.tick(), false);
                assert_eq!(cia.tick(), false);
                assert_eq!(cia.read(registers::ICR).unwrap(), $icr_flag);
            }
        };
    }

    test_timer!(
        timer_a,
        timer_a_underflow,
        timer_a_underflow_interrupt,
        registers::TA_LO,
        registers::TA_HI,
        registers::CRA,
        flags::ICR_TIMER_A
    );

    test_timer!(
        timer_b,
        timer_b_underflow,
        timer_b_underflow_interrupt,
        registers::TB_LO,
        registers::TB_HI,
        registers::CRB,
        flags::ICR_TIMER_B
    );

    #[test]
    fn test_flag() {
        let mut cia = Cia::new();
        assert_eq!(cia.read(registers::ICR).unwrap(), 0);
        cia.set_flag();
        assert_eq!(cia.read(registers::ICR).unwrap(), flags::ICR_FLAG_SIGNAL);
        assert_eq!(cia.read(registers::ICR).unwrap(), 0);
    }

    #[test]
    fn test_flag_interrupt() {
        let mut cia = Cia::new();
        cia.write(
            registers::ICR,
            flags::ICR_SOURCE_BIT | flags::ICR_FLAG_SIGNAL,
        )
        .unwrap();
        assert_eq!(cia.tick(), false);

        cia.set_flag();
        assert_eq!(cia.tick(), true);
        assert_eq!(cia.tick(), true); // Report IRQ until acknowledged.
        assert_eq!(
            cia.read(registers::ICR).unwrap(),
            flags::ICR_TRIGGERED | flags::ICR_FLAG_SIGNAL
        );
        assert_eq!(cia.tick(), false);
        assert_eq!(cia.read(registers::ICR).unwrap(), 0);
    }
}
