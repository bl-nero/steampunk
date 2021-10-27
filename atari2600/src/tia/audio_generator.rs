#[derive(Debug)]
pub struct AudioGenerator {
    volume: u8,
    frequency_divider: u8,
    counter: u8,
}

impl AudioGenerator {
    pub fn new() -> Self {
        Self {
            volume: 0,
            frequency_divider: 0,
            counter: 0,
        }
    }

    pub fn set_volume(&mut self, vol: u8) {
        self.volume = vol & 0b0000_1111;
    }

    pub fn set_frequency_divider(&mut self, divider: u8) {
        self.frequency_divider = divider;
    }

    pub fn tick(&mut self) -> u8 {
        let result = if self.counter < self.frequency_divider + 1 {
            0
        } else {
            self.volume
        };
        self.counter = (self.counter + 1) % ((self.frequency_divider + 1) * 2);
        return result;
    }
}
