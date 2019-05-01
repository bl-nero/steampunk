#[derive(Debug)]
pub struct RAM<'a> {
    pub bytes: &'a mut [u8], //this means that computer has 25 u8's
}

impl<'a> RAM<'a> {
    pub fn read(&self, address: u16) -> u8 {
        // this arrow means we give u16 they return u8
        self.bytes[address as usize]
    }
    pub fn write(&mut self, address: u16, value: u8) {
        self.bytes[address as usize] = value;
    }
}
