//! Color Dreams (mapper 11) — 32 KB PRG + 8 KB CHR bank switching.
//!
//! - $8000–$FFFF: switchable 32 KB PRG bank
//! - CHR-ROM: switchable 8 KB bank
//! - Mirroring: static (from cartridge header)
//!
//! Write to $8000–$FFFF: bits 0–1 = PRG bank, bits 4–7 = CHR bank.
//!
//! Used by unlicensed games: Crystal Mines, Bible Adventures, etc.

use std::sync::Arc;

use super::Mapper;
use crate::nes::cartridge::{Cartridge, Mirroring};

const PRG_BANK_SIZE: usize = 32_768;
const CHR_BANK_SIZE: usize = 8_192;

/// Color Dreams mapper — simple 32 KB PRG + 8 KB CHR switching.
#[derive(Clone)]
pub(super) struct ColorDreams {
    prg_rom: Arc<[u8]>,
    chr_rom: Vec<u8>,
    mirroring: Mirroring,
    prg_bank: u8,
    chr_bank: u8,
    prg_bank_count: u8,
    chr_bank_count: u8,
}

impl ColorDreams {
    pub(super) fn new(cart: Cartridge) -> Self {
        let (prg_rom, chr_rom, mirroring) = cart.into_parts();
        let prg_bank_count = (prg_rom.len() / PRG_BANK_SIZE).max(1) as u8;
        let chr_bank_count = (chr_rom.len() / CHR_BANK_SIZE).max(1) as u8;
        Self {
            prg_rom,
            chr_rom,
            mirroring,
            prg_bank: 0,
            chr_bank: 0,
            prg_bank_count,
            chr_bank_count,
        }
    }
}

impl Mapper for ColorDreams {
    fn cpu_read(&self, addr: u16) -> u8 {
        if addr < 0x8000 {
            return 0;
        }
        let base = usize::from(self.prg_bank) * PRG_BANK_SIZE;
        let offset = base + usize::from(addr - 0x8000);
        self.prg_rom.get(offset).copied().unwrap_or(0)
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if addr >= 0x8000 {
            self.prg_bank = (val & 0x03) % self.prg_bank_count;
            self.chr_bank = (val >> 4) % self.chr_bank_count;
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        let base = usize::from(self.chr_bank) * CHR_BANK_SIZE;
        let offset = base + usize::from(addr);
        self.chr_rom.get(offset).copied().unwrap_or(0)
    }

    fn ppu_write(&mut self, _addr: u16, _val: u8) {}

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn box_clone(&self) -> Box<dyn Mapper> {
        Box::new(self.clone())
    }
}
