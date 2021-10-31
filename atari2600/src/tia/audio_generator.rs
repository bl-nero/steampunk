#[derive(Debug)]
pub struct AudioGenerator {
    volume: u8,
    pattern: u8,
    frequency_divider: u8,
    frequency_counter: u8,
    div2: u32,
    div3: u32,
    div31: u32,
    poly4: u32,
    poly5: u32,
    poly9: u32,
}

/// A single-channel TIA audio generator.
impl AudioGenerator {
    pub fn new() -> Self {
        Self {
            volume: 0,
            pattern: 0,
            frequency_divider: 0,
            frequency_counter: 0,
            div2: 1,
            div3: 0,
            div31: 0,
            poly4: 0b1111,
            poly5: 0b0001_1111,
            poly9: 0b0001_1111_1111,
        }
    }

    pub fn set_volume(&mut self, vol: u8) {
        self.volume = vol & 0b0000_1111;
    }

    pub fn set_pattern(&mut self, pattern: u8) {
        self.pattern = pattern & 0b0000_1111;
    }

    pub fn set_frequency_divider(&mut self, divider: u8) {
        self.frequency_divider = divider & 0b0001_1111;
    }

    /// Performs a single tick of audio generator. It's supposed to be called
    /// twice per scanline. Returns a sample from a [0,15] range.
    pub fn tick(&mut self) -> u8 {
        // Note: the implementation was based on
        // https://problemkaputt.de/2k6specs.htm#audio, but there are a couple
        // of places where it's contradicting or unclear; some test cases are
        // corrected based on my attempts to interpret the TIA schematics.
        // Unfortunately, the audio circuit looks like something a drunk
        // designer scribbled randomly during one Saturday night, and it's for a
        // good reason; after all, a lot of it is dedicated to generate noise.
        // But it also means I have no idea what I'm doing here, and let's
        // consider this implementation a "best effort".
        let output = match self.pattern {
            0x0 | 0xB => 1,
            0x1 | 0x2 | 0x3 => self.poly4 & 0b1,
            0x4 | 0x5 => self.div2,
            0x6 | 0xA | 0xE => {
                if self.div31 < 18 {
                    1
                } else {
                    0
                }
            }
            0x7 | 0x9 | 0xF => self.poly5 & 0b1,
            0x8 => self.poly9 & 0b1,
            0xC | 0xD => self.div2,
            _ => 0,
        } as u8
            * self.volume;

        if self.frequency_counter >= self.frequency_divider {
            self.frequency_counter = 0;
            self.div3 = (self.div3 + 1) % 3;
            if self.pattern != 0xE || self.div3 == 0 {
                self.div31 = (self.div31 + 1) % 31;
            }
            if (self.pattern != 0xC && self.pattern != 0xD) || self.div3 == 0 {
                self.div2 = (self.div2 + 1) % 2;
            }
            let poly5_carry = self.poly5 & 0b1 != 0;
            if self.pattern != 0xF || self.div3 == 0 {
                let new_bit = (((self.poly5 & 0b1) << 2) ^ (self.poly5 & 0b100)) << 2;
                self.poly5 = (self.poly5 >> 1) | new_bit;
            }

            if match self.pattern {
                0x2 => self.div31 == 0 || self.div31 == 18,
                0x3 => poly5_carry,
                _ => true,
            } {
                let new_bit = (((self.poly4 & 0b1) << 1) ^ (self.poly4 & 0b10)) << 2;
                self.poly4 = (self.poly4 >> 1) | new_bit;
            }

            let new_bit = (((self.poly9 & 0b1) << 4) ^ (self.poly9 & 0b1_0000)) << 4;
            self.poly9 = (self.poly9 >> 1) | new_bit;
        } else {
            self.frequency_counter += 1;
        }

        return output;
    }
}
