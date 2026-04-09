//! MMC1 / `SxROM` (mapper 1) — serial shift register bank switching.
//!
//! Writes to $8000–$FFFF feed a 5-bit shift register. After 5 writes
//! the accumulated value is latched into one of four internal registers
//! (selected by bits 14–13 of the write address):
//!
//! | Address       | Register | Controls                          |
//! |---------------|----------|-----------------------------------|
//! | $8000–$9FFF   | Control  | Mirroring, PRG/CHR banking mode   |
//! | $A000–$BFFF   | CHR 0    | CHR bank for $0000–$0FFF          |
//! | $C000–$DFFF   | CHR 1    | CHR bank for $1000–$1FFF          |
//! | $E000–$FFFF   | PRG      | PRG bank select                   |

use std::sync::Arc;

use super::Mapper;
use crate::nes::cartridge::{Cartridge, Mirroring};

const PRG_BANK_SIZE: usize = 16_384;
const CHR_BANK_SIZE: usize = 4_096;

/// MMC1 mapper with serial shift register.
#[derive(Clone)]
pub(super) struct Mmc1 {
    prg_rom: Arc<[u8]>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_bank_count: u8,

    /// 5-bit shift register (bits 0–4 hold data, bit 5 unused).
    shift: u8,
    /// Number of bits written into the shift register (0–4).
    shift_count: u8,

    // ── Latched registers ────────────────────────────────────
    /// Control register (mirroring, PRG/CHR mode).
    control: u8,
    /// CHR bank 0 register.
    chr_bank0: u8,
    /// CHR bank 1 register.
    chr_bank1: u8,
    /// PRG bank register.
    prg_bank: u8,
}

impl Mmc1 {
    pub(super) fn new(cart: Cartridge) -> Self {
        let (prg_rom, chr_rom, _mirroring) = cart.into_parts();
        let chr_is_ram = chr_rom.is_empty();
        let chr = if chr_is_ram { vec![0; 8192] } else { chr_rom };
        let prg_bank_count = (prg_rom.len() / PRG_BANK_SIZE) as u8;
        Self {
            prg_rom,
            chr,
            chr_is_ram,
            prg_bank_count,
            shift: 0,
            shift_count: 0,
            // Power-on default: PRG fixed last bank at $C000.
            control: 0x0C,
            chr_bank0: 0,
            chr_bank1: 0,
            prg_bank: 0,
        }
    }

    /// Resets the shift register to its initial state.
    fn reset_shift(&mut self) {
        self.shift = 0;
        self.shift_count = 0;
    }

    /// Handles a write to the shift register / register latch.
    fn write_register(&mut self, addr: u16, val: u8) {
        // Bit 7 set → reset shift register and set control to
        // "fix last PRG bank" mode.
        if val & 0x80 != 0 {
            self.reset_shift();
            self.control |= 0x0C;
            return;
        }

        // Feed bit 0 of val into the shift register.
        self.shift |= (val & 1) << self.shift_count;
        self.shift_count += 1;

        if self.shift_count < 5 {
            return;
        }

        // 5 bits accumulated — latch into the target register.
        let data = self.shift;
        self.reset_shift();

        match addr {
            0x8000..=0x9FFF => self.control = data,
            0xA000..=0xBFFF => self.chr_bank0 = data,
            0xC000..=0xDFFF => self.chr_bank1 = data,
            0xE000..=0xFFFF => self.prg_bank = data & 0x0F,
            _ => {}
        }
    }

    /// PRG banking mode from the control register (bits 2–3).
    fn prg_mode(&self) -> u8 {
        (self.control >> 2) & 0x03
    }

    /// CHR banking mode from the control register (bit 4).
    fn chr_mode_4k(&self) -> bool {
        self.control & 0x10 != 0
    }

    /// Resolves a CPU address to a PRG-ROM byte offset.
    fn resolve_prg(&self, addr: u16) -> usize {
        let bank = match self.prg_mode() {
            // Mode 0, 1: switch 32 KB at $8000 (bank number ignoring bit 0).
            0 | 1 => {
                let bank32 = usize::from(self.prg_bank & 0x0E);
                let page = usize::from(addr - 0x8000);
                return bank32 * PRG_BANK_SIZE + page;
            }
            // Mode 2: fix first bank at $8000, switch $C000.
            2 => {
                if addr < 0xC000 {
                    0
                } else {
                    usize::from(self.prg_bank)
                }
            }
            // Mode 3: switch $8000, fix last bank at $C000.
            _ => {
                if addr < 0xC000 {
                    usize::from(self.prg_bank)
                } else {
                    usize::from(self.prg_bank_count - 1)
                }
            }
        };
        let base = addr & 0x3FFF;
        bank * PRG_BANK_SIZE + usize::from(base)
    }

    /// Resolves a PPU address to a CHR byte offset.
    fn resolve_chr(&self, addr: u16) -> usize {
        if self.chr_mode_4k() {
            // Two separate 4 KB banks.
            if addr < 0x1000 {
                usize::from(self.chr_bank0) * CHR_BANK_SIZE + usize::from(addr)
            } else {
                usize::from(self.chr_bank1) * CHR_BANK_SIZE + usize::from(addr - 0x1000)
            }
        } else {
            // One 8 KB bank (chr_bank0 with bit 0 cleared).
            let bank = usize::from(self.chr_bank0 & 0x1E);
            bank * CHR_BANK_SIZE + usize::from(addr)
        }
    }
}

impl Mapper for Mmc1 {
    fn cpu_read(&self, addr: u16) -> u8 {
        if addr < 0x8000 {
            return 0;
        }
        let offset = self.resolve_prg(addr);
        self.prg_rom.get(offset).copied().unwrap_or(0)
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if addr >= 0x8000 {
            self.write_register(addr, val);
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
        // Bits 0–1 of control: 0/1 = one-screen (approximated as
        // horizontal), 2 = vertical, 3 = horizontal.
        if self.control & 0x03 == 2 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        }
    }

    fn box_clone(&self) -> Box<dyn Mapper> {
        Box::new(self.clone())
    }
}
