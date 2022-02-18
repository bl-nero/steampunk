/// The actual logic of the debugger, free of all of the communication noise.
pub struct DebuggerCore {
    paused: bool,
}

impl DebuggerCore {
    pub fn new() -> Self {
        Self { paused: true }
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_and_pauses() {
        let mut dc = DebuggerCore::new();
        assert!(dc.paused());

        dc.resume();
        assert!(!dc.paused());

        dc.pause();
        assert!(dc.paused());
    }
}
