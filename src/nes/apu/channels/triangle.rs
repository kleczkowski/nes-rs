//! Triangle wave channel ($4008–$400B).

use crate::nes::apu::units::length::LengthCounter;
use crate::nes::apu::units::linear::LinearCounter;

/// 32-step triangle wave sequence.
#[rustfmt::skip]
const TRIANGLE_SEQUENCE: [u8; 32] = [
    15, 14, 13, 12, 11, 10,  9,  8,  7,  6,  5,  4,  3,  2,  1,  0,
     0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15,
];

/// Triangle channel state.
#[derive(Debug, Clone)]
pub(in crate::nes) struct Triangle {
    /// Length counter.
    pub(in crate::nes) length: LengthCounter,
    /// Linear counter.
    pub(in crate::nes) linear: LinearCounter,
    /// Timer period (11-bit).
    timer_period: u16,
    /// Current timer countdown.
    timer_value: u16,
    /// Current position in the 32-step sequence.
    ///
    /// Starts at 15 where `TRIANGLE_SEQUENCE[15] = 0`, so the
    /// DAC's power-on output is silence.  When counters expire the
    /// sequencer freezes and the DAC holds its last value — no
    /// explicit "activated" flag needed.
    sequence_pos: u8,
}

impl Triangle {
    /// Creates a triangle channel in its initial state.
    pub(in crate::nes) fn new() -> Self {
        Self {
            length: LengthCounter::new(),
            linear: LinearCounter::new(),
            timer_period: 0,
            timer_value: 0,
            sequence_pos: 15, // TRIANGLE_SEQUENCE[15] = 0 → silence
        }
    }

    /// Advances the timer by one CPU cycle.
    pub(in crate::nes) fn tick(&mut self) {
        if !self.length.active() || !self.linear.active() {
            return;
        }
        if self.timer_value == 0 {
            self.timer_value = self.timer_period;
            self.sequence_pos = (self.sequence_pos + 1) % 32;
        } else {
            self.timer_value -= 1;
        }
    }

    /// Returns the current output sample (0–15).
    ///
    /// The counters gate the sequencer (in `tick`), not the output.
    /// When frozen, the DAC holds its last value.  Periods below 2
    /// produce ultrasonics; the analog filter averages them to ~7.5.
    pub(in crate::nes) fn output(&self) -> u8 {
        if self.timer_period < 2 && (self.length.active() && self.linear.active()) {
            return 7;
        }
        let pos = usize::from(self.sequence_pos);
        TRIANGLE_SEQUENCE.get(pos).copied().unwrap_or(0)
    }

    /// Writes a register (offset 0–3 within $4008–$400B).
    pub(in crate::nes) fn write_reg(&mut self, reg: u8, val: u8) {
        match reg {
            0 => {
                self.linear.control = val & 0x80 != 0;
                self.length.halted = val & 0x80 != 0;
                self.linear.reload_value = val & 0x7F;
            }
            2 => self.timer_period = (self.timer_period & 0x0700) | u16::from(val),
            3 => {
                self.timer_period = (self.timer_period & 0x00FF) | (u16::from(val & 0x07) << 8);
                self.length.load(val >> 3);
                self.linear.reload_flag = true;
            }
            _ => {}
        }
    }
}
