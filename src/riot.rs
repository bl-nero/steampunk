use crate::memory::{Memory, ReadError, ReadResult, WriteError, WriteResult};

// A MOS Technology 6532 RIOT chip. Note that originally, this chip also
// included 128 bytes of RAM, but for the sake of single-responsibility
// principle, it's been split out to a separate struct: `memory::AtariRam`.
#[derive(Debug)]
pub struct Riot {}

impl Riot {
    pub fn new() -> Riot {
        Riot{}
    }
}

impl Memory for Riot {
    fn read(&self, address: u16) -> ReadResult {
        Err(ReadError { address })
    }

    fn write(&mut self, address: u16, value: u8) -> WriteResult {
        Err(WriteError { address, value })
    }
}
