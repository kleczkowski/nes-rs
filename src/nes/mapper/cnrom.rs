//! CNROM (mapper 3) — switchable 8 KB CHR bank.
//!
//! - PRG-ROM: fixed 16/32 KB at $8000–$FFFF (same as NROM)
//! - CHR-ROM: switchable 8 KB bank (selected by writes to $8000–$FFFF)
//!
//! A write to $8000–$FFFF sets the low bits as the CHR bank number.

use super::Mapper;
use crate::nes::cartridge::{Cartridge, Mirroring};

/// Size of one CHR bank.
const CHR_BANK_SIZE: usize = 8192;

/// CNROM mapper — fixed PRG, switchable CHR bank.
pub(super) struct Cnrom {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    mirroring: Mirroring,
    /// Currently selected 8 KB CHR bank.
    chr_bank: u8,
    /// Total number of 8 KB CHR banks.
    chr_bank_count: u8,
}

impl Cnrom {
    pub(super) fn new(cart: Cartridge) -> Self {
        let chr_bank_count = (cart.chr_rom().len() / CHR_BANK_SIZE).max(1) as u8;
        Self {
            prg_rom: cart.prg_rom().to_vec(),
            chr_rom: cart.chr_rom().to_vec(),
            mirroring: cart.mirroring(),
            chr_bank: 0,
            chr_bank_count,
        }
    }
}

impl Mapper for Cnrom {
    fn cpu_read(&self, addr: u16) -> u8 {
        if addr < 0x8000 {
            return 0;
        }
        let offset = usize::from(addr - 0x8000) % self.prg_rom.len();
        self.prg_rom.get(offset).copied().unwrap_or(0)
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if addr >= 0x8000 {
            self.chr_bank = val % self.chr_bank_count;
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        let bank_offset = usize::from(self.chr_bank) * CHR_BANK_SIZE;
        let offset = bank_offset + usize::from(addr);
        self.chr_rom.get(offset).copied().unwrap_or(0)
    }

    fn ppu_write(&mut self, _addr: u16, _val: u8) {
        // CNROM uses CHR-ROM — writes are ignored.
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}
