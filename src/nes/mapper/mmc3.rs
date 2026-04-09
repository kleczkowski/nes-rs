//! MMC3 / `TxROM` (mapper 4) — the most widely used NES mapper.
//!
//! Features:
//! - Two switchable 8 KB PRG banks + two fixed 8 KB banks
//! - Six CHR bank registers (two 2 KB + four 1 KB)
//! - Scanline counter with IRQ
//! - Dynamic mirroring
//!
//! Used by: Super Mario Bros. 2 & 3, Mega Man 3–6, Kirby's
//! Adventure, and ~590 other games.
//!
//! ## Register map
//!
//! | Address       | Even/Odd | Function                     |
//! |---------------|----------|------------------------------|
//! | $8000         | Even     | Bank select (target + mode)  |
//! | $8001         | Odd      | Bank data                    |
//! | $A000         | Even     | Mirroring                    |
//! | $A001         | Odd      | PRG-RAM protect (ignored)    |
//! | $C000         | Even     | IRQ latch value              |
//! | $C001         | Odd      | IRQ reload flag              |
//! | $E000         | Even     | IRQ disable + acknowledge    |
//! | $E001         | Odd      | IRQ enable                   |

use std::sync::Arc;

use super::Mapper;
use crate::nes::cartridge::{Cartridge, Mirroring};

const PRG_BANK_SIZE: usize = 8_192;
const CHR_BANK_1K: usize = 1_024;

/// MMC3 mapper with scanline IRQ counter.
#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
pub(super) struct Mmc3 {
    prg_rom: Arc<[u8]>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_bank_count: u8,
    chr_bank_count: u16,

    // ── Bank select ─────────────────────────────────────────
    /// Target register for next $8001 write (bits 0–2 of $8000).
    bank_target: u8,
    /// PRG bank mode (bit 6 of $8000).
    prg_mode: bool,
    /// CHR bank mode (bit 7 of $8000).
    chr_mode: bool,
    /// Bank register values R0–R7.
    regs: [u8; 8],

    // ── Mirroring ───────────────────────────────────────────
    mirroring: Mirroring,

    // ── Scanline IRQ ────────────────────────────────────────
    /// Scanline counter latch value (written via $C000).
    irq_latch: u8,
    /// Current scanline counter.
    irq_counter: u8,
    /// Counter should reload on next scanline tick.
    irq_reload: bool,
    /// IRQ output is enabled.
    irq_enabled: bool,
    /// An IRQ is pending delivery to the CPU.
    irq_pending: bool,
}

impl Mmc3 {
    pub(super) fn new(cart: Cartridge) -> Self {
        let (prg_rom, chr_rom, mirroring) = cart.into_parts();
        let chr_is_ram = chr_rom.is_empty();
        let chr = if chr_is_ram { vec![0; 8192] } else { chr_rom };
        let prg_bank_count = (prg_rom.len() / PRG_BANK_SIZE) as u8;
        let chr_bank_count = (chr.len() / CHR_BANK_1K) as u16;

        Self {
            prg_rom,
            chr,
            chr_is_ram,
            prg_bank_count,
            chr_bank_count,
            bank_target: 0,
            prg_mode: false,
            chr_mode: false,
            regs: [0; 8],
            mirroring,
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_pending: false,
        }
    }

    /// Resolves a CPU address ($8000–$FFFF) to a PRG-ROM offset.
    fn resolve_prg(&self, addr: u16) -> usize {
        let bank = match (addr, self.prg_mode) {
            // prg_mode=false: R6 at $8000, R7 at $A000, (-2) at $C000, (-1) at $E000
            // prg_mode=true:  (-2) at $8000, R7 at $A000, R6 at $C000, (-1) at $E000
            (0x8000..=0x9FFF, false) | (0xC000..=0xDFFF, true) => {
                usize::from(self.regs[6] % self.prg_bank_count)
            }
            (0xA000..=0xBFFF, _) => usize::from(self.regs[7] % self.prg_bank_count),
            (0xC000..=0xDFFF, false) | (0x8000..=0x9FFF, true) => {
                usize::from(self.prg_bank_count - 2)
            }
            (0xE000..=0xFFFF, _) => usize::from(self.prg_bank_count - 1),
            _ => return 0,
        };
        bank * PRG_BANK_SIZE + usize::from(addr & 0x1FFF)
    }

    /// Resolves a PPU address ($0000–$1FFF) to a CHR offset.
    fn resolve_chr(&self, addr: u16) -> usize {
        let addr = usize::from(addr);
        // Determine the 1 KB slot index (0–7) within the 8 KB window.
        let slot = addr / CHR_BANK_1K;

        // chr_mode=false: R0 2KB@$0000, R1 2KB@$0800, R2-R5 1KB@$1000-$1C00
        // chr_mode=true:  R2-R5 1KB@$0000-$0C00, R0 2KB@$1000, R1 2KB@$1800
        let bank = if self.chr_mode {
            match slot {
                0 => usize::from(self.regs[2]),
                1 => usize::from(self.regs[3]),
                2 => usize::from(self.regs[4]),
                3 => usize::from(self.regs[5]),
                4 | 5 => (usize::from(self.regs[0]) & 0xFE) + (slot & 1),
                _ => (usize::from(self.regs[1]) & 0xFE) + (slot & 1),
            }
        } else {
            match slot {
                0 | 1 => (usize::from(self.regs[0]) & 0xFE) + (slot & 1),
                2 | 3 => (usize::from(self.regs[1]) & 0xFE) + (slot & 1),
                4 => usize::from(self.regs[2]),
                5 => usize::from(self.regs[3]),
                6 => usize::from(self.regs[4]),
                _ => usize::from(self.regs[5]),
            }
        };

        let bank = bank % usize::from(self.chr_bank_count);
        bank * CHR_BANK_1K + (addr % CHR_BANK_1K)
    }
}

impl Mapper for Mmc3 {
    fn cpu_read(&self, addr: u16) -> u8 {
        if addr < 0x8000 {
            return 0;
        }
        let offset = self.resolve_prg(addr);
        self.prg_rom.get(offset).copied().unwrap_or(0)
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if addr < 0x8000 {
            return;
        }
        let even = addr & 1 == 0;
        match (addr, even) {
            (0x8000..=0x9FFF, true) => {
                self.bank_target = val & 0x07;
                self.prg_mode = val & 0x40 != 0;
                self.chr_mode = val & 0x80 != 0;
            }
            (0x8000..=0x9FFF, false) => {
                let target = usize::from(self.bank_target);
                if let Some(reg) = self.regs.get_mut(target) {
                    *reg = val;
                }
            }
            (0xA000..=0xBFFF, true) => {
                self.mirroring = if val & 1 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                };
            }
            (0xC000..=0xDFFF, true) => {
                self.irq_latch = val;
            }
            (0xC000..=0xDFFF, false) => {
                self.irq_reload = true;
            }
            (0xE000..=0xFFFF, true) => {
                self.irq_enabled = false;
                self.irq_pending = false;
            }
            (0xE000..=0xFFFF, false) => {
                self.irq_enabled = true;
            }
            _ => {}
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        let offset = self.resolve_chr(addr);
        self.chr.get(offset).copied().unwrap_or(0)
    }

    fn ppu_write(&mut self, addr: u16, val: u8) {
        if self.chr_is_ram {
            let offset = self.resolve_chr(addr);
            if let Some(cell) = self.chr.get_mut(offset) {
                *cell = val;
            }
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn notify_scanline(&mut self) {
        if self.irq_counter == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter -= 1;
        }

        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_pending = true;
        }
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn irq_clear(&mut self) {
        self.irq_pending = false;
    }

    fn box_clone(&self) -> Box<dyn Mapper> {
        Box::new(self.clone())
    }
}
