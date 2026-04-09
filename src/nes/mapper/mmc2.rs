//! MMC2 / `PxROM` (mapper 9) — latch-switched CHR banks.
//!
//! - $8000–$9FFF: switchable 8 KB PRG bank
//! - $A000–$FFFF: fixed to the last three 8 KB PRG banks
//! - CHR: two 4 KB halves, each with two selectable banks
//!   that switch automatically based on PPU tile fetches
//! - Mirroring: dynamic (register at $F000)
//!
//! The latch mechanism: reading tile $FD from a pattern table
//! half selects one CHR bank; reading tile $FE selects another.
//! This is used exclusively by Mike Tyson's Punch-Out!!

use std::cell::Cell;

use super::Mapper;
use crate::nes::cartridge::{Cartridge, Mirroring};

const PRG_BANK_SIZE: usize = 8_192;
const CHR_BANK_SIZE: usize = 4_096;

/// MMC2 mapper with latch-based CHR switching.
pub(super) struct Mmc2 {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    prg_bank_count: u8,

    /// 8 KB PRG bank at $8000–$9FFF.
    prg_bank: u8,

    /// CHR bank selected when latch 0 sees $FD.
    chr_bank_0_fd: u8,
    /// CHR bank selected when latch 0 sees $FE.
    chr_bank_0_fe: u8,
    /// CHR bank selected when latch 1 sees $FD.
    chr_bank_1_fd: u8,
    /// CHR bank selected when latch 1 sees $FE.
    chr_bank_1_fe: u8,

    /// Current latch for $0000–$0FFF (false = FD, true = FE).
    ///
    /// `Cell` because the latch updates as a side-effect of PPU reads,
    /// and `ppu_read` takes `&self`.
    latch_0: Cell<bool>,
    /// Current latch for $1000–$1FFF.
    latch_1: Cell<bool>,

    /// Mirroring mode (bit 0 of $F000 write).
    mirroring: Mirroring,
}

impl Mmc2 {
    pub(super) fn new(cart: Cartridge) -> Self {
        let prg_bank_count = (cart.prg_rom().len() / PRG_BANK_SIZE) as u8;
        Self {
            prg_rom: cart.prg_rom().to_vec(),
            chr_rom: cart.chr_rom().to_vec(),
            prg_bank_count,
            prg_bank: 0,
            chr_bank_0_fd: 0,
            chr_bank_0_fe: 0,
            chr_bank_1_fd: 0,
            chr_bank_1_fe: 0,
            latch_0: Cell::new(true),
            latch_1: Cell::new(true),
            mirroring: cart.mirroring(),
        }
    }

    /// Returns the active 4 KB CHR bank for the low half ($0000–$0FFF).
    fn chr_bank_0(&self) -> u8 {
        if self.latch_0.get() {
            self.chr_bank_0_fe
        } else {
            self.chr_bank_0_fd
        }
    }

    /// Returns the active 4 KB CHR bank for the high half ($1000–$1FFF).
    fn chr_bank_1(&self) -> u8 {
        if self.latch_1.get() {
            self.chr_bank_1_fe
        } else {
            self.chr_bank_1_fd
        }
    }
}

impl Mapper for Mmc2 {
    fn cpu_read(&self, addr: u16) -> u8 {
        let offset = match addr {
            0x8000..=0x9FFF => {
                usize::from(self.prg_bank) * PRG_BANK_SIZE + usize::from(addr - 0x8000)
            }
            // Last three 8 KB banks are fixed.
            0xA000..=0xFFFF => {
                let fixed_start = usize::from(self.prg_bank_count - 3) * PRG_BANK_SIZE;
                fixed_start + usize::from(addr - 0xA000)
            }
            _ => return 0,
        };
        self.prg_rom.get(offset).copied().unwrap_or(0)
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        match addr {
            0xA000..=0xAFFF => {
                self.prg_bank = (val & 0x0F) % self.prg_bank_count;
            }
            0xB000..=0xBFFF => self.chr_bank_0_fd = val & 0x1F,
            0xC000..=0xCFFF => self.chr_bank_0_fe = val & 0x1F,
            0xD000..=0xDFFF => self.chr_bank_1_fd = val & 0x1F,
            0xE000..=0xEFFF => self.chr_bank_1_fe = val & 0x1F,
            0xF000..=0xFFFF => {
                self.mirroring = if val & 1 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                };
            }
            _ => {}
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        let offset = if addr < 0x1000 {
            usize::from(self.chr_bank_0()) * CHR_BANK_SIZE + usize::from(addr)
        } else {
            usize::from(self.chr_bank_1()) * CHR_BANK_SIZE + usize::from(addr - 0x1000)
        };
        let val = self.chr_rom.get(offset).copied().unwrap_or(0);

        // Latch updates after the byte is fetched.
        match addr {
            0x0FD8..=0x0FDF => self.latch_0.set(false),
            0x0FE8..=0x0FEF => self.latch_0.set(true),
            0x1FD8..=0x1FDF => self.latch_1.set(false),
            0x1FE8..=0x1FEF => self.latch_1.set(true),
            _ => {}
        }

        val
    }

    fn ppu_write(&mut self, _addr: u16, _val: u8) {
        // CHR-ROM is read-only.
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}
