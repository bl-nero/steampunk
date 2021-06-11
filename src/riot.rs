use crate::memory::{Memory, ReadError, ReadResult, WriteError, WriteResult};
use rand::Rng;

// A MOS Technology 6532 RIOT chip. Note that originally, this chip also
// included 128 bytes of RAM, but for the sake of single-responsibility
// principle, it's been split out to a separate struct: `memory::AtariRam`.
#[derive(Debug)]
pub struct Riot {
    timer: u8,
    divider: u32,
    interval_length: u32,

    reg_swcha: u8,
    reg_swchb: u8,
}

pub enum Port {
    PA,
    PB,
}

impl Riot {
    pub fn new() -> Riot {
        let mut rng = rand::thread_rng();
        Riot {
            timer: rng.gen(),
            divider: rng.gen(),
            interval_length: [1, 8, 64, 1024][rng.gen_range(0..4)],
            reg_swcha: 0xFF,
            reg_swchb: 0xFF,
        }
    }

    pub fn tick(&mut self) {
        if self.divider == 0 {
            self.timer = self.timer.wrapping_sub(1);
        }
        self.divider = (self.divider + 1) % self.interval_length;
    }

    fn reset_timer(&mut self, timer_value: u8, interval_length: u32) {
        self.timer = timer_value;
        self.interval_length = interval_length;
        self.divider = 0;
    }

    pub fn set_port(&mut self, port: Port, value: u8) {
        match port {
            Port::PA => self.reg_swcha = value,
            Port::PB => self.reg_swchb = value,
        };
    }
}

impl Memory for Riot {
    fn read(&self, address: u16) -> ReadResult {
        match address {
            registers::SWCHA => Ok(self.reg_swcha),
            registers::SWCHB => Ok(self.reg_swchb),
            registers::INTIM => Ok(self.timer),
            _ => Err(ReadError { address }),
        }
    }

    fn write(&mut self, address: u16, value: u8) -> WriteResult {
        match address {
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
    pub const TIM1T: u16 = 0x294;
    pub const TIM8T: u16 = 0x295;
    pub const TIM64T: u16 = 0x296;
    pub const T1024T: u16 = 0x297;
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

    fn input_port_direction() {
        // riot.set_port_direction(Port::PA, 0x12);
    }
}
