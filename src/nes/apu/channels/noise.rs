//! Noise channel ($400C–$400F) — LFSR-based pseudo-random noise.

use crate::nes::apu::units::envelope::Envelope;
use crate::nes::apu::units::length::LengthCounter;

/// Timer period lookup table (NTSC).
#[rustfmt::skip]
const PERIOD_TABLE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

/// Noise channel state.
#[derive(Debug, Clone)]
pub(in crate::nes) struct Noise {
    /// Envelope generator.
    pub(in crate::nes) envelope: Envelope,
    /// Length counter.
    pub(in crate::nes) length: LengthCounter,
    /// Timer period (from lookup table).
    timer_period: u16,
    /// Current timer countdown.
    timer_value: u16,
    /// 15-bit linear feedback shift register.
    shift_register: u16,
    /// Mode bit: false = bit 1 feedback, true = bit 6 feedback.
    mode: bool,
}

impl Noise {
    /// Creates a noise channel in its initial state.
    pub(in crate::nes) fn new() -> Self {
        Self {
            envelope: Envelope::new(),
            length: LengthCounter::new(),
            timer_period: 0,
            timer_value: 0,
            shift_register: 1, // initial LFSR state
            mode: false,
        }
    }

    /// Advances the timer by one CPU cycle.
    pub(in crate::nes) fn tick(&mut self) {
        if self.timer_value == 0 {
            self.timer_value = self.timer_period;
            self.clock_shift_register();
        } else {
            self.timer_value -= 1;
        }
    }

    /// Clocks the LFSR.
    fn clock_shift_register(&mut self) {
        let feedback_bit = if self.mode { 6 } else { 1 };
        let bit0 = self.shift_register & 1;
        let other = (self.shift_register >> feedback_bit) & 1;
        let feedback = bit0 ^ other;
        self.shift_register >>= 1;
        self.shift_register |= feedback << 14;
    }

    /// Returns the current output sample (0–15).
    pub(in crate::nes) fn output(&self) -> u8 {
        if self.shift_register & 1 != 0 || !self.length.active() {
            return 0;
        }
        self.envelope.output()
    }

    /// Writes a register (offset 0–3 within $400C–$400F).
    pub(in crate::nes) fn write_reg(&mut self, reg: u8, val: u8) {
        match reg {
            0 => {
                self.length.halted = val & 0x20 != 0;
                self.envelope.write(val);
            }
            2 => {
                self.mode = val & 0x80 != 0;
                let index = usize::from(val & 0x0F);
                self.timer_period = PERIOD_TABLE.get(index).copied().unwrap_or(0);
            }
            3 => {
                self.length.load(val >> 3);
                self.envelope.start = true;
            }
            _ => {}
        }
    }
}
