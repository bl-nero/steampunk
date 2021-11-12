use ya6502::memory::Memory;

pub struct Vic {
    reg_border_color: u8,
}

impl Vic {
    pub fn new() -> Self {
        Self {
            reg_border_color: 0,
        }
    }
    pub fn tick(&mut self) -> u8 {
        self.reg_border_color
    }
}

impl Memory for Vic {
    fn read(&self, _: u16) -> std::result::Result<u8, ya6502::memory::ReadError> {
        todo!()
    }
    fn write(&mut self, _: u16, value: u8) -> std::result::Result<(), ya6502::memory::WriteError> {
        self.reg_border_color = value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_border() {
        let mut vic = Vic::new();
        vic.write(0xD020, 0x00).unwrap();
        assert_eq!(vic.tick(), 0x00);

        vic.write(0xD020, 0x01).unwrap();
        assert_eq!(vic.tick(), 0x01);

        vic.write(0xD020, 0x0F).unwrap();
        assert_eq!(vic.tick(), 0x0F);
    }
}
