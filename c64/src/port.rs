/// An internal state of an 8-bit I/O port.
#[derive(Debug, Default)]
pub struct Port {
    /// A direction register: each bit controls the direction of a given pin.
    /// 0=input, 1=input/output.
    pub direction: u8,
    /// The data register: holds the value as set from the inside of the chip.
    pub register: u8,
    /// Value set from the outside to the pins.
    pub pins: u8,
}

impl Port {
    /// Resolves the value on the pins. Assumes that bits where direction
    /// register is set to 1 are driven by the chip, and everything else is
    /// driven from outside.
    pub fn read(&self) -> u8 {
        (self.register & self.direction) | (self.pins & !self.direction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading() {
        let mut port = Port::default();
        port.direction = 0b1111_0000;
        port.register = 0b1100_1100;
        port.pins = 0b1010_1010;
        assert_eq!(port.read(), 0b1100_1010);

        port.direction = 0b0000_1111;
        assert_eq!(port.read(), 0b1010_1100);
    }
}
