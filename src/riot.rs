use crate::memory::{Memory, ReadError, ReadResult, WriteError, WriteResult};
use rand::Rng;
use std::cell::Cell;

/// A MOS Technology 6532 RIOT chip. Note that originally, this chip also
/// included 128 bytes of RAM, but for the sake of single-responsibility
/// principle, it's been split out to a separate struct: `memory::AtariRam`.
#[derive(Debug)]
pub struct Riot {
    /// A divider that counts from 0 to `interval_length` and then wraps around.
    /// Each time it reaches 0, `reg_intim` is decreased.
    timer_divider: u32,
    /// A timer interval length, in CPU cycles.
    interval_length: u32,
    /// Direct input from port A. Note: According to MOS 6532 datasheet, "for
    /// any [PA] output pin, the data transferred into the processor will be the
    /// same as that contained in the Output Register if the voltage on the pin
    /// is allowed to go to 2.4v for a logic one." This means that if a standard
    /// joystick is connected to PA, whenever the switches are closed
    /// (grounded), the voltage is not allowed to remain high enough. Because of
    /// this, a low pin value on port always overrides the port register.
    port_a: u8,
    /// Direct input from port B. Note from the datasheet: "The primary
    /// difference between the PA and the PB ports is in the operation of the
    /// output buffers which drive these pins. The buffers are push-pull devices
    /// which are capable of sourcing 3 ma at 1.5v. This allows these pins to
    /// directly drive transistor switches. To assure that the microprocessor
    /// will read proper data on a “Read PB” operation, sufficient logic is
    /// provided in the chip to allow the microprocessor to read the Output
    /// Register instead of reading the peripheral pin as on the PA port."
    port_b: u8,

    /// Port A output register.
    reg_swcha: u8,
    /// Port A pin direction (0=read, 1=write)
    reg_swacnt: u8,
    /// Port B output register.
    reg_swchb: u8,
    /// Port B pin direction (0=read, 1=write)
    reg_swbcnt: u8,
    /// Current timer value.
    reg_intim: u8,
    /// Timer interrupt flag. It's a `Cell`, since it is also modified while
    /// reading, which is normally an operation that can be performed on an
    /// immutable object. Perhaps we should refacor the whole concept of reading
    /// instead?
    reg_timint: Cell<u8>,
}

pub enum Port {
    PA,
    PB,
}

impl Riot {
    pub fn new() -> Riot {
        let mut rng = rand::thread_rng();
        Riot {
            timer_divider: rng.gen(),
            interval_length: [1, 8, 64, 1024][rng.gen_range(0..4)],
            port_a: 0,
            port_b: 0,

            reg_swcha: 0xFF,
            reg_swacnt: 0x00,
            reg_swchb: 0xFF,
            reg_swbcnt: 0x00,
            reg_intim: rng.gen(),
            reg_timint: Cell::new(0),
        }
    }

    pub fn tick(&mut self) {
        if self.timer_divider == 0 || self.reg_timint.get() & flags::TIMINT_TIMER != 0 {
            self.reg_intim = self.reg_intim.wrapping_sub(1);
            if self.reg_intim == 0xFF {
                *self.reg_timint.get_mut() |= flags::TIMINT_TIMER;
            }
        }
        self.timer_divider = (self.timer_divider + 1) % self.interval_length;
    }

    fn reset_timer(&mut self, timer_value: u8, interval_length: u32) {
        self.reg_intim = timer_value;
        self.interval_length = interval_length;
        self.timer_divider = 0;
        *self.reg_timint.get_mut() &= !flags::TIMINT_TIMER;
    }

    pub fn set_port(&mut self, port: Port, value: u8) {
        match port {
            Port::PA => self.port_a = value,
            Port::PB => self.port_b = value,
        };
    }
}

impl Memory for Riot {
    fn read(&self, address: u16) -> ReadResult {
        match address {
            registers::SWCHA => {
                Ok((self.reg_swacnt & self.reg_swcha & self.port_a)
                    | (!self.reg_swacnt & self.port_a))
            }
            registers::SWACNT => Ok(self.reg_swacnt),
            registers::SWCHB => {
                Ok((self.reg_swbcnt & self.reg_swchb) | (!self.reg_swbcnt & self.port_b))
            }
            registers::SWBCNT => Ok(self.reg_swbcnt),
            registers::INTIM => {
                self.reg_timint
                    .set(self.reg_timint.get() & !flags::TIMINT_TIMER);
                return Ok(self.reg_intim);
            }
            registers::TIMINT => Ok(self.reg_timint.get()),
            _ => Err(ReadError { address }),
        }
    }

    fn write(&mut self, address: u16, value: u8) -> WriteResult {
        match address {
            registers::SWCHA => self.reg_swcha = value,
            registers::SWACNT => self.reg_swacnt = value,
            registers::SWCHB => self.reg_swchb = value,
            registers::SWBCNT => self.reg_swbcnt = value,
            registers::TIM1T => self.reset_timer(value, 1),
            registers::TIM8T => self.reset_timer(value, 8),
            registers::TIM64T => self.reset_timer(value, 64),
            registers::T1024T => self.reset_timer(value, 1024),
            _ => return Err(WriteError { address, value }),
        };
        Ok(())
    }
}

mod registers {
    pub const SWCHA: u16 = 0x280;
    pub const SWACNT: u16 = 0x281;
    pub const SWCHB: u16 = 0x282;
    pub const SWBCNT: u16 = 0x283;
    pub const INTIM: u16 = 0x284;
    pub const TIMINT: u16 = 0x285;
    pub const TIM1T: u16 = 0x294;
    pub const TIM8T: u16 = 0x295;
    pub const TIM64T: u16 = 0x296;
    pub const T1024T: u16 = 0x297;
}

mod flags {
    pub const TIMINT_TIMER: u8 = 1 << 7;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tim1t() {
        let mut riot = Riot::new();
        riot.write(registers::TIM1T, 0x03).unwrap();
        let intim_values = (0..4).map(|_| {
            riot.tick();
            riot.read(registers::INTIM).unwrap()
        });
        itertools::assert_equal(intim_values, [0x02, 0x01, 0x00, 0xFF].iter().copied());

        // Note: we don't just continue reading from INTIM, since this affects
        // the counter itself (details to be emulated later).
        riot.write(registers::TIM1T, 0x45).unwrap();
        for _ in 0..(0x45 + 0x5) {
            riot.tick();
        }
        assert_eq!(riot.read(registers::INTIM).unwrap(), 0xFB);
    }

    #[test]
    fn tim64t() {
        let mut riot = Riot::new();
        riot.write(registers::TIM64T, 0x03).unwrap();
        let intim_values = (0..193).map(|_| {
            riot.tick();
            riot.read(registers::INTIM).unwrap()
        });
        itertools::assert_equal(
            intim_values,
            itertools::repeat_n(2, 64)
                .chain(itertools::repeat_n(1, 64))
                .chain(itertools::repeat_n(0, 64))
                .chain(std::iter::once(0xFF)),
        );
    }

    #[test]
    fn t1024t() {
        let mut riot = Riot::new();
        riot.write(registers::T1024T, 0x02).unwrap();
        let intim_values = (0..2049).map(|_| {
            riot.tick();
            riot.read(registers::INTIM).unwrap()
        });
        itertools::assert_equal(
            intim_values,
            itertools::repeat_n(1, 1024)
                .chain(itertools::repeat_n(0, 1024))
                .chain(std::iter::once(0xFF)),
        );
    }

    #[test]
    fn timer_underflow() {
        let mut riot = Riot::new();
        riot.write(registers::TIM64T, 0x01).unwrap();
        for _ in 0..64 {
            riot.tick();
        }
        assert_eq!(riot.read(registers::TIMINT).unwrap(), 0);
        riot.tick();

        // After the underflow, we expect the timer interrupt flag to be set,
        // but we don't yet read INTIM, as it would immediately stop the fast
        // countdown.
        assert_eq!(riot.read(registers::TIMINT).unwrap(), flags::TIMINT_TIMER);
        riot.tick();
        riot.tick();
        riot.tick();
        assert_eq!(riot.read(registers::TIMINT).unwrap(), flags::TIMINT_TIMER);
        riot.tick();
        assert_eq!(riot.read(registers::INTIM).unwrap(), 0xFB);

        // After reading INTIM, the timer should go back to the regular mode of
        // operation.
        assert_eq!(riot.read(registers::TIMINT).unwrap(), 0);
        riot.tick();
        riot.tick();
        riot.tick();
        assert_eq!(riot.read(registers::INTIM).unwrap(), 0xFB);

        // Underflow after underflow
        riot.write(registers::TIM64T, 0x01).unwrap();
        for _ in 0..(64 + 256 + 6) {
            riot.tick();
        }
        assert_eq!(riot.read(registers::INTIM).unwrap(), 0xFA);
    }

    #[test]
    fn timer_reset() {
        let mut riot = Riot::new();
        riot.write(registers::TIM64T, 0x01).unwrap();
        for _ in 0..(64 + 2) {
            riot.tick();
        }
        riot.write(registers::TIM64T, 0x04).unwrap();
        riot.tick();
        riot.tick();
        riot.tick();
        assert_eq!(riot.read(registers::INTIM).unwrap(), 0x03);
    }

    #[test]
    fn address_mirroring() {}

    #[test]
    fn input_ports() {
        let mut riot = Riot::new();
        riot.set_port(Port::PA, 0x12);
        assert_eq!(riot.read(registers::SWCHA).unwrap(), 0x12);
        riot.set_port(Port::PA, 0x34);
        assert_eq!(riot.read(registers::SWCHA).unwrap(), 0x34);
        riot.set_port(Port::PB, 0x56);
        assert_eq!(riot.read(registers::SWCHB).unwrap(), 0x56);
        riot.set_port(Port::PB, 0x78);
        assert_eq!(riot.read(registers::SWCHB).unwrap(), 0x78);
    }

    #[test]
    fn input_port_b_direction() {
        let mut riot = Riot::new();

        // Reading from the bits set as output should return the register value
        // instead of port input.
        riot.set_port(Port::PB, 0b1100_1100);
        riot.write(registers::SWBCNT, 0b1111_0000).unwrap();
        riot.write(registers::SWCHB, 0b0101_0101).unwrap();
        assert_eq!(riot.read(registers::SWCHB).unwrap(), 0b0101_1100);

        // Data in the output register should be cached and return what we wrote
        // to bits previously set to act as inputs.
        riot.write(registers::SWBCNT, 0b0000_1111).unwrap();
        assert_eq!(riot.read(registers::SWCHB).unwrap(), 0b1100_0101);
    }

    #[test]
    fn input_port_a_direction() {
        let mut riot = Riot::new();

        // Reading from the bits set as output should return the register value
        // instead of port input, but only where the PA register pin is not
        // grounded. Grounded pins always return 0.
        riot.set_port(Port::PA, 0b1100_1100);
        riot.write(registers::SWACNT, 0b1111_0000).unwrap();
        riot.write(registers::SWCHA, 0b0101_0101).unwrap();
        assert_eq!(riot.read(registers::SWCHA).unwrap(), 0b0100_1100);

        // Data in the output register should be cached and return what we wrote
        // to bits previously set to act as inputs; however, the above grounding
        // rule still applies.
        riot.write(registers::SWACNT, 0b0000_1111).unwrap();
        assert_eq!(riot.read(registers::SWCHA).unwrap(), 0b1100_0100);
    }
}
