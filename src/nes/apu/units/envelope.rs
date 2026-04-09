//! Envelope generator — 4-bit volume decay used by Pulse and Noise.

/// Envelope generator producing a decaying or constant volume.
#[derive(Debug, Clone)]
pub(in crate::nes) struct Envelope {
    /// Restart the envelope on next clock.
    pub(in crate::nes) start: bool,
    /// Divider period (set from volume/period register).
    period: u8,
    /// Current divider counter.
    divider: u8,
    /// Current decay level (counts down from 15).
    decay: u8,
    /// Loop the envelope (also halts the length counter).
    pub(in crate::nes) looping: bool,
    /// Use constant volume instead of decay.
    constant: bool,
    /// Constant volume value / envelope period.
    volume: u8,
}

impl Envelope {
    /// Creates an envelope in its initial state.
    pub(in crate::nes) fn new() -> Self {
        Self {
            start: false,
            period: 0,
            divider: 0,
            decay: 0,
            looping: false,
            constant: false,
            volume: 0,
        }
    }

    /// Clocks the envelope (called by the frame sequencer quarter-frame).
    pub(in crate::nes) fn clock(&mut self) {
        if self.start {
            self.start = false;
            self.decay = 15;
            self.divider = self.period;
            return;
        }
        if self.divider == 0 {
            self.divider = self.period;
            if self.decay > 0 {
                self.decay -= 1;
            } else if self.looping {
                self.decay = 15;
            }
        } else {
            self.divider -= 1;
        }
    }

    /// Returns the current output volume (0–15).
    pub(in crate::nes) fn output(&self) -> u8 {
        if self.constant {
            self.volume
        } else {
            self.decay
        }
    }

    /// Immediately resets the envelope to full volume.
    /// Punchier attack than waiting for the next quarter-frame.
    pub(in crate::nes) fn restart(&mut self) {
        self.start = false;
        self.decay = 15;
        self.divider = self.period;
    }

    /// Writes the envelope register byte ($4000/$4004/$400C bits 0–5).
    pub(in crate::nes) fn write(&mut self, val: u8) {
        self.volume = val & 0x0F;
        self.period = val & 0x0F;
        self.constant = val & 0x10 != 0;
        self.looping = val & 0x20 != 0;
    }
}
