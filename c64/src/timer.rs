/// A CIA timer
#[derive(Default, Debug)]
pub struct Timer {
    control: u8,
    latch: u16,
    counter: u16,
}

impl Timer {
    /// Reads the control register.
    pub fn control(&self) -> u8 {
        // The LOAD flag is write-only.
        self.control & !flags::LOAD
    }

    /// Writes to the control register.
    pub fn set_control(&mut self, value: u8) -> Result<(), ()> {
        // Not all modes are available just yet.
        if value & !(flags::START | flags::LOAD | flags::RUNMODE) != 0 {
            return Err(());
        }
        self.control = value;
        if self.control & flags::LOAD != 0 {
            self.counter = self.latch;
        }
        Ok(())
    }

    pub fn set_latch(&mut self, value: u16) {
        self.latch = value;
    }

    pub fn latch(&self) -> u16 {
        self.latch
    }

    pub fn counter(&self) -> u16 {
        self.counter
    }

    /// Performs a tick, returns `true` on underflow
    pub fn tick(&mut self) -> bool {
        if self.control & flags::START != 0 {
            if self.counter > 0 {
                self.counter -= 1;
            } else {
                self.counter = self.latch;
                if self.control & flags::RUNMODE == flags::RUNMODE_ONE_SHOT {
                    self.control &= !flags::START;
                }
                return true;
            }
        }
        return false;
    }
}

pub mod flags {
    pub const START: u8 = 1 << 0;
    pub const RUNMODE: u8 = 1 << 3;
    pub const LOAD: u8 = 1 << 4;

    pub const RUNMODE_ONE_SHOT: u8 = RUNMODE;
    pub const RUNMODE_CONTINUOUS: u8 = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_and_starting() {
        use super::flags::*;

        let mut timer = Timer::default();
        timer.set_latch(1234);
        timer.set_control(0).unwrap(); // Don't load or start yet

        timer.tick();
        assert_eq!(timer.control(), 0);
        assert_eq!(timer.counter(), 0);

        // Load, but don't start yet.
        timer.set_control(LOAD).unwrap();
        // The LOAD flag of the control register should be ignored while reading.
        assert_eq!(timer.control(), 0);
        assert_eq!(timer.counter(), 1234);

        timer.tick();
        assert_eq!(timer.counter(), 1234);

        // OK, now start it.
        timer.set_control(START).unwrap();
        assert_eq!(timer.control(), START);
        assert_eq!(timer.counter(), 1234);

        timer.tick();
        assert_eq!(timer.counter(), 1233);
        timer.tick();
        assert_eq!(timer.counter(), 1232);
    }

    #[test]
    fn underflow() {
        use super::flags::*;

        let mut timer = Timer::default();
        timer.set_latch(4);
        timer
            .set_control(LOAD | START | RUNMODE_CONTINUOUS)
            .unwrap();

        assert_eq!(timer.counter(), 4);
        assert_eq!(timer.tick(), false);
        assert_eq!(timer.tick(), false);
        assert_eq!(timer.tick(), false);
        assert_eq!(timer.tick(), false);
        assert_eq!(timer.counter(), 0);

        assert_eq!(timer.tick(), true);
        assert_eq!(timer.counter(), 4);
        assert_eq!(timer.tick(), false);
        assert_eq!(timer.counter(), 3);

        timer.set_control(LOAD | START | RUNMODE_ONE_SHOT).unwrap();
        assert_eq!(timer.tick(), false);
        assert_eq!(timer.tick(), false);
        assert_eq!(timer.tick(), false);
        assert_eq!(timer.counter(), 1);
        assert_eq!(timer.tick(), false);
        assert_eq!(timer.counter(), 0);

        assert_eq!(timer.tick(), true);
        assert_eq!(timer.counter(), 4);
        assert_eq!(timer.tick(), false);
        assert_eq!(timer.counter(), 4);
    }
}
