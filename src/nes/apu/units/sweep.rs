//! Sweep unit — periodically adjusts pulse channel period.

/// Sweep unit that modifies a pulse channel's timer period.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub(in crate::nes) struct Sweep {
    /// Reload the divider on next clock.
    reload: bool,
    /// Sweep is enabled.
    enabled: bool,
    /// Negate the period change.
    negate: bool,
    /// Divider period.
    period: u8,
    /// Current divider counter.
    divider: u8,
    /// Shift count for target period calculation.
    shift: u8,
    /// Use ones' complement for negate (pulse 1 = true, pulse 2 = false).
    ones_complement: bool,
}

impl Sweep {
    /// Creates a sweep unit.
    ///
    /// `ones_complement` should be `true` for pulse 1, `false` for pulse 2.
    pub(in crate::nes) fn new(ones_complement: bool) -> Self {
        Self {
            reload: false,
            enabled: false,
            negate: false,
            period: 0,
            divider: 0,
            shift: 0,
            ones_complement,
        }
    }

    /// Computes the target period from the current timer period.
    fn target_period(&self, timer_period: u16) -> u16 {
        let change = timer_period >> self.shift;
        if self.negate {
            if self.ones_complement {
                timer_period.wrapping_sub(change).wrapping_sub(1)
            } else {
                timer_period.wrapping_sub(change)
            }
        } else {
            timer_period.wrapping_add(change)
        }
    }

    /// Returns `true` if the sweep is muting the channel.
    pub(in crate::nes) fn muting(&self, timer_period: u16) -> bool {
        timer_period < 8 || self.target_period(timer_period) > 0x7FF
    }

    /// Clocks the sweep unit (called by the frame sequencer half-frame).
    pub(in crate::nes) fn clock(&mut self, timer_period: &mut u16) {
        let target = self.target_period(*timer_period);
        if self.divider == 0 && self.enabled && self.shift > 0 && !self.muting(*timer_period) {
            *timer_period = target;
        }
        if self.divider == 0 || self.reload {
            self.divider = self.period;
            self.reload = false;
        } else {
            self.divider -= 1;
        }
    }

    /// Writes the sweep register byte ($4001/$4005).
    pub(in crate::nes) fn write(&mut self, val: u8) {
        self.enabled = val & 0x80 != 0;
        self.period = (val >> 4) & 0x07;
        self.negate = val & 0x08 != 0;
        self.shift = val & 0x07;
        self.reload = true;
    }
}
