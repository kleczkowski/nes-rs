//! APU register read/write dispatch ($4000–$4017).

use super::Apu;

impl Apu {
    /// Handles a CPU write to an APU register.
    pub(in crate::nes) fn write_register(&mut self, addr: u16, val: u8) {
        match addr {
            0x4000..=0x4003 => self.pulse1.write_reg((addr & 0x03) as u8, val),
            0x4004..=0x4007 => self.pulse2.write_reg((addr & 0x03) as u8, val),
            0x4008..=0x400B => self.triangle.write_reg((addr & 0x03) as u8, val),
            0x400C..=0x400F => self.noise.write_reg((addr & 0x03) as u8, val),
            0x4010..=0x4013 => self.dmc.write_reg((addr & 0x03) as u8, val),
            0x4015 => self.write_status(val),
            0x4017 => self.sequencer.write_control(val),
            _ => {}
        }
    }

    /// Handles a CPU read from $4015 (APU status).
    pub(in crate::nes) fn read_status(&mut self) -> u8 {
        let mut status = 0u8;
        if self.pulse1.length.active() {
            status |= 0x01;
        }
        if self.pulse2.length.active() {
            status |= 0x02;
        }
        if self.triangle.length.active() {
            status |= 0x04;
        }
        if self.noise.length.active() {
            status |= 0x08;
        }
        if self.dmc.bytes_remaining > 0 {
            status |= 0x10;
        }
        if self.sequencer.irq_pending {
            status |= 0x40;
        }
        if self.dmc.irq_pending {
            status |= 0x80;
        }
        // Reading $4015 clears the frame IRQ flag.
        self.sequencer.irq_pending = false;
        status
    }

    /// Handles a write to $4015 (channel enable/disable).
    fn write_status(&mut self, val: u8) {
        self.pulse1.length.set_enabled(val & 0x01 != 0);
        self.pulse2.length.set_enabled(val & 0x02 != 0);
        self.triangle.length.set_enabled(val & 0x04 != 0);
        self.noise.length.set_enabled(val & 0x08 != 0);
        self.dmc.set_enabled(val & 0x10 != 0);
    }
}
