//! Nonlinear audio mixer — combines 5 APU channels into a single sample.
//!
//! Uses precomputed lookup tables from the `NESDev` wiki formulas:
//! - `pulse_out = 95.88 / (8128.0 / (p1 + p2) + 100.0)`
//! - `tnd_out = 159.79 / (1 / (t/8227 + n/12241 + d/22638) + 100.0)`

/// Precomputed mixer lookup tables.
pub(in crate::nes) struct Mixer {
    /// Pulse output table (index = pulse1 + pulse2, 0–30).
    pulse_table: [f32; 31],
    /// TND output table (index = 3*triangle + 2*noise + dmc, 0–202).
    tnd_table: [f32; 203],
}

impl Mixer {
    /// Builds the mixer lookup tables.
    pub(in crate::nes) fn new() -> Self {
        let mut pulse_table = [0.0_f32; 31];
        for (n, entry) in pulse_table.iter_mut().enumerate().skip(1) {
            *entry = 95.52 / (8128.0 / n as f32 + 100.0);
        }

        let mut tnd_table = [0.0_f32; 203];
        for (n, entry) in tnd_table.iter_mut().enumerate().skip(1) {
            *entry = 163.67 / (24329.0 / n as f32 + 100.0);
        }

        Self {
            pulse_table,
            tnd_table,
        }
    }

    /// Mixes 5 channel outputs into a single audio sample (0.0 to ~1.0).
    pub(in crate::nes) fn mix(
        &self,
        pulse1: u8,
        pulse2: u8,
        triangle: u8,
        noise: u8,
        dmc: u8,
    ) -> f32 {
        let pulse_index = usize::from(pulse1) + usize::from(pulse2);
        let tnd_index = 3 * usize::from(triangle) + 2 * usize::from(noise) + usize::from(dmc);

        let pulse_out = self.pulse_table.get(pulse_index).copied().unwrap_or(0.0);
        let tnd_out = self.tnd_table.get(tnd_index).copied().unwrap_or(0.0);

        pulse_out + tnd_out
    }
}
