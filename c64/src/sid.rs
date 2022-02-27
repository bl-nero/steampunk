use ya6502::memory::Inspect;
use ya6502::memory::Memory;
use ya6502::memory::Read;
use ya6502::memory::ReadError;
use ya6502::memory::ReadResult;
use ya6502::memory::Write;
use ya6502::memory::WriteResult;

/// A 6581 SID chip. So far, it's just a dumb address space that doesn't do
/// anything.
#[derive(Debug)]
pub struct Sid {}

impl Sid {
    pub fn new() -> Self {
        Sid {}
    }
}

impl Write for Sid {
    fn write(&mut self, _address: u16, _value: u8) -> WriteResult {
        Ok(())
    }
}

impl Inspect for Sid {
    fn inspect(&self, address: u16) -> ReadResult {
        Err(ReadError { address })
    }
}

impl Read for Sid {
    fn read(&mut self, address: u16) -> ReadResult {
        self.inspect(address)
    }
}

impl Memory for Sid {}
