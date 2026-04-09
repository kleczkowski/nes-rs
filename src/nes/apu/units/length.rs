//! Length counter — silences a channel after a programmable duration.

/// Lookup table mapping 5-bit index to length counter reload values.
#[rustfmt::skip]
const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20,  2, 40,  4, 80,  6, 160,  8, 60, 10, 14, 12, 26, 14,
    12,  16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30,
];

/// Length counter that silences a channel when it reaches zero.
#[derive(Debug, Clone)]
pub(in crate::nes) struct LengthCounter {
    /// Current counter value.
    counter: u8,
    /// Whether the counter is halted (does not decrement).
    pub(in crate::nes) halted: bool,
    /// Whether the channel is enabled (counter can be loaded).
    enabled: bool,
}

impl LengthCounter {
    /// Creates a length counter in its initial state.
    pub(in crate::nes) fn new() -> Self {
        Self {
            counter: 0,
            halted: false,
            enabled: false,
        }
    }

    /// Clocks the length counter (called by the frame sequencer half-frame).
    pub(in crate::nes) fn clock(&mut self) {
        if !self.halted && self.counter > 0 {
            self.counter -= 1;
        }
    }

    /// Returns `true` if the counter is non-zero (channel should produce output).
    pub(in crate::nes) fn active(&self) -> bool {
        self.counter > 0
    }

    /// Loads the counter from a 5-bit table index.
    pub(in crate::nes) fn load(&mut self, index: u8) {
        if self.enabled {
            let i = usize::from(index & 0x1F);
            self.counter = LENGTH_TABLE.get(i).copied().unwrap_or(0);
        }
    }

    /// Enables or disables the channel. Disabling clears the counter.
    pub(in crate::nes) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.counter = 0;
        }
    }

    /// Returns the raw counter value (used for $4015 status reads).
    pub(in crate::nes) fn value(&self) -> u8 {
        self.counter
    }
}
