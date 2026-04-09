//! CPU-facing PPU register I/O ($2000–$2007).
//!
//! The CPU accesses the PPU through 8 memory-mapped registers.
//! Several registers use a shared write toggle (`w`) for
//! double-write sequences (PPUSCROLL, PPUADDR).

use super::Ppu;
use crate::nes::mapper::Mapper;

impl Ppu {
    /// Handles a CPU read from a PPU register.
    ///
    /// `reg` is the register offset (0–7, i.e., address & 0x07).
    pub(crate) fn cpu_read(&mut self, reg: u8, mapper: &dyn Mapper) -> u8 {
        match reg {
            // $2002 — PPUSTATUS
            2 => {
                let val = (self.status & 0xE0) | (self.read_buffer & 0x1F);
                self.status &= !0x80; // clear vblank
                self.w = false; // reset write toggle
                val
            }
            // $2004 — OAMDATA
            4 => self
                .oam
                .get(usize::from(self.oam_addr))
                .copied()
                .unwrap_or(0),
            // $2007 — PPUDATA
            7 => self.read_ppudata(mapper),
            // $2000, $2001, $2003, $2005, $2006 are write-only.
            _ => 0,
        }
    }

    /// Handles a CPU write to a PPU register.
    ///
    /// `reg` is the register offset (0–7, i.e., address & 0x07).
    pub(crate) fn cpu_write(&mut self, reg: u8, val: u8, mapper: &mut dyn Mapper) {
        match reg {
            // $2000 — PPUCTRL
            0 => {
                self.ctrl = val;
                // Bits 0–1 → nametable select in t (bits 10–11).
                self.t = (self.t & 0xF3FF) | (u16::from(val & 0x03) << 10);
            }
            // $2001 — PPUMASK
            1 => self.mask = val,
            // $2003 — OAMADDR
            3 => self.oam_addr = val,
            // $2004 — OAMDATA
            4 => {
                if let Some(cell) = self.oam.get_mut(usize::from(self.oam_addr)) {
                    *cell = val;
                }
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            // $2005 — PPUSCROLL (double write)
            5 => self.write_scroll(val),
            // $2006 — PPUADDR (double write)
            6 => self.write_addr(val),
            // $2007 — PPUDATA
            7 => self.write_ppudata(val, mapper),
            _ => {}
        }
    }

    /// Handles PPUDATA ($2007) reads with buffering.
    ///
    /// Non-palette reads return the buffer and refill it from VRAM.
    /// Palette reads return the value directly but still update the
    /// buffer with the "behind" nametable data.
    fn read_ppudata(&mut self, mapper: &dyn Mapper) -> u8 {
        let addr = self.v;
        let raw = self.ppu_read(addr, mapper);
        let result = if (addr & 0x3FFF) >= 0x3F00 {
            // Palette reads are unbuffered; buffer gets nametable data.
            self.read_buffer = self.ppu_read(addr - 0x1000, mapper);
            raw
        } else {
            let buffered = self.read_buffer;
            self.read_buffer = raw;
            buffered
        };
        self.v = self.v.wrapping_add(self.vram_increment());
        result
    }

    /// Handles PPUDATA ($2007) writes.
    fn write_ppudata(&mut self, val: u8, mapper: &mut dyn Mapper) {
        let addr = self.v;
        self.ppu_write(addr, val, mapper);
        self.v = self.v.wrapping_add(self.vram_increment());
    }

    /// Handles PPUSCROLL ($2005) double-write.
    fn write_scroll(&mut self, val: u8) {
        if self.w {
            // Second write: coarse Y and fine Y.
            self.t = (self.t & 0x8C1F)
                | (u16::from(val & 0x07) << 12)  // fine Y
                | (u16::from(val >> 3) << 5); // coarse Y
        } else {
            // First write: coarse X (5 bits) and fine X (3 bits).
            self.t = (self.t & 0xFFE0) | u16::from(val >> 3);
            self.fine_x = val & 0x07;
        }
        self.w = !self.w;
    }

    /// Handles PPUADDR ($2006) double-write.
    fn write_addr(&mut self, val: u8) {
        if self.w {
            // Second write: low byte, then copy t → v.
            self.t = (self.t & 0xFF00) | u16::from(val);
            self.v = self.t;
        } else {
            // First write: high byte (bits 8–13 of t, bit 14 cleared).
            self.t = (self.t & 0x00FF) | (u16::from(val & 0x3F) << 8);
        }
        self.w = !self.w;
    }
}
