//! `AxROM` (mapper 7) — 32 KB PRG bank switching with single-screen mirroring.
//!
//! - $8000–$FFFF: switchable 32 KB PRG bank
//! - CHR: 8 KB CHR-RAM
//! - Mirroring: single-screen, selected by bit 4 of bank register
//!
//! Used by: Battletoads, Marble Madness, Wizards & Warriors.

use super::Mapper;
use crate::nes::cartridge::{Cartridge, Mirroring};

const PRG_BANK_SIZE: usize = 32_768;

/// `AxROM` mapper — 32 KB PRG switching, single-screen mirroring.
pub(super) struct Axrom {
    prg_rom: Vec<u8>,
    chr_ram: Vec<u8>,
    /// Currently selected 32 KB PRG bank (bits 0–2 of write).
    bank_select: u8,
    /// Total number of 32 KB PRG banks.
    bank_count: u8,
    /// Single-screen nametable select (bit 4 of write).
    nametable: u8,
}

impl Axrom {
    pub(super) fn new(cart: Cartridge) -> Self {
        let bank_count = (cart.prg_rom().len() / PRG_BANK_SIZE).max(1) as u8;
        Self {
            prg_rom: cart.prg_rom().to_vec(),
            chr_ram: vec![0; 8192],
            bank_select: 0,
            bank_count,
            nametable: 0,
        }
    }
}

impl Mapper for Axrom {
    fn cpu_read(&self, addr: u16) -> u8 {
        if addr < 0x8000 {
            return 0;
        }
        let base = usize::from(self.bank_select) * PRG_BANK_SIZE;
        let offset = base + usize::from(addr - 0x8000);
        self.prg_rom.get(offset).copied().unwrap_or(0)
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if addr >= 0x8000 {
            self.bank_select = (val & 0x07) % self.bank_count;
            self.nametable = (val >> 4) & 1;
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        self.chr_ram.get(usize::from(addr)).copied().unwrap_or(0)
    }

    fn ppu_write(&mut self, addr: u16, val: u8) {
        if let Some(cell) = self.chr_ram.get_mut(usize::from(addr)) {
            *cell = val;
        }
    }

    fn mirroring(&self) -> Mirroring {
        if self.nametable == 0 {
            Mirroring::SingleLow
        } else {
            Mirroring::SingleHigh
        }
    }
}
