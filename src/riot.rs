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
}

impl Riot {
    pub fn new() -> Riot {
        let mut rng = rand::thread_rng();
        Riot {
            timer: rng.gen(),
            divider: rng.gen(),
            interval_length: [1, 8, 64, 1024][rng.gen_range(0..4)],
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
}

impl Memory for Riot {
    fn read(&self, address: u16) -> ReadResult {
        match address {
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
}
