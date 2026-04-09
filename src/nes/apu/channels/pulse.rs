//! Pulse (square wave) channel — used by Pulse 1 ($4000) and Pulse 2 ($4004).

use crate::nes::apu::units::envelope::Envelope;
use crate::nes::apu::units::length::LengthCounter;
use crate::nes::apu::units::sweep::Sweep;

/// Duty cycle waveform sequences (8 steps each, 4 duty modes).
///
/// The ordering matters: writing $4003/$4007 resets `sequence_pos`
/// to 0.  Position 1 must be HIGH for duties 0–2 so the first
/// audible edge arrives after just one timer cycle, not six.
#[rustfmt::skip]
const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0], // 12.5%
    [0, 1, 1, 0, 0, 0, 0, 0], // 25%
    [0, 1, 1, 1, 1, 0, 0, 0], // 50%
    [1, 0, 0, 1, 1, 1, 1, 1], // 75% (inverted 25%)
];

/// Pulse channel state.
#[derive(Debug, Clone)]
pub(in crate::nes) struct Pulse {
    /// Envelope generator.
    pub(in crate::nes) envelope: Envelope,
    /// Length counter.
    pub(in crate::nes) length: LengthCounter,
    /// Sweep unit.
    pub(in crate::nes) sweep: Sweep,
    /// Timer period (11-bit, from registers).
    pub(in crate::nes) timer_period: u16,
    /// Current timer countdown.
    timer_value: u16,
    /// Duty cycle mode (0–3).
    duty: u8,
    /// Current position in the 8-step duty sequence.
    sequence_pos: u8,
}

impl Pulse {
    /// Creates a pulse channel.
    ///
    /// `ones_complement` is `true` for Pulse 1, `false` for Pulse 2.
    pub(in crate::nes) fn new(ones_complement: bool) -> Self {
        Self {
            envelope: Envelope::new(),
            length: LengthCounter::new(),
            sweep: Sweep::new(ones_complement),
            timer_period: 0,
            timer_value: 0,
            duty: 0,
            sequence_pos: 0,
        }
    }

    /// Advances the timer by one APU cycle (every 2 CPU cycles).
    pub(in crate::nes) fn tick(&mut self) {
        if self.timer_value == 0 {
            self.timer_value = self.timer_period;
            self.sequence_pos = (self.sequence_pos + 1) % 8;
        } else {
            self.timer_value -= 1;
        }
    }

    /// Returns the current output sample (0–15).
    pub(in crate::nes) fn output(&self) -> u8 {
        let duty_row = usize::from(self.duty & 0x03);
        let step = usize::from(self.sequence_pos);
        let duty_on = DUTY_TABLE
            .get(duty_row)
            .and_then(|row| row.get(step))
            .copied()
            .unwrap_or(0);
        if duty_on == 0 || !self.length.active() || self.sweep.muting(self.timer_period) {
            return 0;
        }
        self.envelope.output()
    }

    /// Writes a register (offset 0–3 within the pulse register block).
    pub(in crate::nes) fn write_reg(&mut self, reg: u8, val: u8) {
        match reg {
            0 => {
                self.duty = (val >> 6) & 0x03;
                self.length.halted = val & 0x20 != 0;
                self.envelope.write(val);
            }
            1 => self.sweep.write(val),
            2 => self.timer_period = (self.timer_period & 0x0700) | u16::from(val),
            3 => {
                self.timer_period = (self.timer_period & 0x00FF) | (u16::from(val & 0x07) << 8);
                self.length.load(val >> 3);
                self.sequence_pos = 0;
                self.envelope.start = true;
            }
            _ => {}
        }
    }
}
