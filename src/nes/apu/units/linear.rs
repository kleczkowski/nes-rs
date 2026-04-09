//! Linear counter — triangle channel specific counter.

/// Linear counter that controls the triangle channel duration.
#[derive(Debug, Clone)]
pub(in crate::nes) struct LinearCounter {
    /// Current counter value.
    counter: u8,
    /// Reload value (set from register).
    pub(in crate::nes) reload_value: u8,
    /// Reload the counter on next clock.
    pub(in crate::nes) reload_flag: bool,
    /// Control flag (also halts the length counter).
    pub(in crate::nes) control: bool,
}

impl LinearCounter {
    /// Creates a linear counter in its initial state.
    pub(in crate::nes) fn new() -> Self {
        Self {
            counter: 0,
            reload_value: 0,
            reload_flag: false,
            control: false,
        }
    }

    /// Clocks the linear counter (called by the quarter-frame).
    pub(in crate::nes) fn clock(&mut self) {
        if self.reload_flag {
            self.counter = self.reload_value;
        } else if self.counter > 0 {
            self.counter -= 1;
        }
        if !self.control {
            self.reload_flag = false;
        }
    }

    /// Returns `true` if the counter is non-zero.
    pub(in crate::nes) fn active(&self) -> bool {
        self.counter > 0
    }
}
