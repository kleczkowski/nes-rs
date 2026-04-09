//! Delta Modulation Channel ($4010–$4013) — 1-bit delta-encoded sample playback.

/// DMC rate table (NTSC CPU cycles per output level change).
#[rustfmt::skip]
const RATE_TABLE: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];

/// Delta modulation channel state.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub(in crate::nes) struct Dmc {
    /// Channel enabled.
    pub(in crate::nes) enabled: bool,
    /// IRQ enabled for end-of-sample.
    pub(in crate::nes) irq_enabled: bool,
    /// IRQ pending (set when sample ends with IRQ enabled).
    pub(in crate::nes) irq_pending: bool,
    /// Loop the sample.
    looping: bool,
    /// Timer period (from rate table).
    timer_period: u16,
    /// Current timer countdown.
    timer_value: u16,
    /// Current output level (0–127, 7-bit DAC).
    output_level: u8,
    /// Sample start address ($C000 + addr * 64).
    sample_addr: u16,
    /// Sample byte length (length * 16 + 1).
    sample_length: u16,
    /// Current read address.
    pub(in crate::nes) current_addr: u16,
    /// Bytes remaining in the current sample.
    pub(in crate::nes) bytes_remaining: u16,
    /// Buffered sample byte (None if empty — needs refill).
    sample_buffer: Option<u8>,
    /// Output shift register.
    shift_register: u8,
    /// Bits remaining in the current shift register (0–8).
    bits_remaining: u8,
    /// Silence flag (shift register produces no output changes).
    silence: bool,
}

impl Dmc {
    /// Creates a DMC channel in its initial state.
    pub(in crate::nes) fn new() -> Self {
        Self {
            enabled: false,
            irq_enabled: false,
            irq_pending: false,
            looping: false,
            timer_period: 428,
            timer_value: 0,
            output_level: 0,
            sample_addr: 0xC000,
            sample_length: 1,
            current_addr: 0xC000,
            bytes_remaining: 0,
            sample_buffer: None,
            shift_register: 0,
            bits_remaining: 0,
            silence: true,
        }
    }

    /// Advances the timer by one CPU cycle.
    pub(in crate::nes) fn tick(&mut self) {
        if self.timer_value == 0 {
            self.timer_value = self.timer_period;
            self.clock_output();
        } else {
            self.timer_value -= 1;
        }
    }

    /// Clocks the output unit — shifts one bit and adjusts output level.
    fn clock_output(&mut self) {
        if self.bits_remaining == 0 {
            self.bits_remaining = 8;
            if let Some(byte) = self.sample_buffer.take() {
                self.silence = false;
                self.shift_register = byte;
            } else {
                self.silence = true;
            }
        }
        if !self.silence {
            if self.shift_register & 1 != 0 {
                if self.output_level <= 125 {
                    self.output_level += 2;
                }
            } else if self.output_level >= 2 {
                self.output_level -= 2;
            }
            self.shift_register >>= 1;
        }
        self.bits_remaining = self.bits_remaining.saturating_sub(1);
    }

    /// Returns the current output level (0–127).
    pub(in crate::nes) fn output(&self) -> u8 {
        self.output_level
    }

    /// Returns `true` if the sample buffer is empty and bytes remain.
    pub(in crate::nes) fn needs_sample(&self) -> bool {
        self.sample_buffer.is_none() && self.bytes_remaining > 0
    }

    /// Returns the address to read the next sample byte from, if needed.
    pub(in crate::nes) fn sample_addr(&self) -> Option<u16> {
        if self.needs_sample() {
            Some(self.current_addr)
        } else {
            None
        }
    }

    /// Fills the sample buffer with a byte read from memory.
    pub(in crate::nes) fn fill_sample(&mut self, byte: u8) {
        self.sample_buffer = Some(byte);
        self.current_addr = self.current_addr.wrapping_add(1) | 0x8000;
        self.bytes_remaining = self.bytes_remaining.saturating_sub(1);
        if self.bytes_remaining == 0 {
            if self.looping {
                self.current_addr = self.sample_addr;
                self.bytes_remaining = self.sample_length;
            } else if self.irq_enabled {
                self.irq_pending = true;
            }
        }
    }

    /// Writes a register (offset 0–3 within $4010–$4013).
    pub(in crate::nes) fn write_reg(&mut self, reg: u8, val: u8) {
        match reg {
            0 => {
                self.irq_enabled = val & 0x80 != 0;
                self.looping = val & 0x40 != 0;
                let index = usize::from(val & 0x0F);
                self.timer_period = RATE_TABLE.get(index).copied().unwrap_or(428);
                if !self.irq_enabled {
                    self.irq_pending = false;
                }
            }
            1 => self.output_level = val & 0x7F,
            2 => self.sample_addr = 0xC000 | (u16::from(val) << 6),
            3 => self.sample_length = (u16::from(val) << 4) | 1,
            _ => {}
        }
    }

    /// Enables or disables the DMC. Enabling starts playback if bytes remain.
    pub(in crate::nes) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.irq_pending = false;
        if !enabled {
            self.bytes_remaining = 0;
        } else if self.bytes_remaining == 0 {
            self.current_addr = self.sample_addr;
            self.bytes_remaining = self.sample_length;
        }
    }
}
