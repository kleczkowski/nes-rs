//! `UxROM` (mapper 2) — switchable 16 KB PRG bank.
//!
//! - $8000–$BFFF: switchable 16 KB PRG bank (selected by writes)
//! - $C000–$FFFF: fixed to the last 16 KB PRG bank
//! - CHR: 8 KB CHR-RAM (no CHR-ROM bank switching)
//!
//! A write to $8000–$FFFF sets the low bits as the bank number.

use super::Mapper;
use crate::nes::cartridge::{Cartridge, Mirroring};

/// Size of one PRG bank.
const PRG_BANK_SIZE: usize = 16_384;

/// `UxROM` mapper — switchable lower PRG bank, fixed upper bank.
pub(super) struct Uxrom {
    prg_rom: Vec<u8>,
    chr_ram: Vec<u8>,
    mirroring: Mirroring,
    /// Currently selected 16 KB bank for $8000–$BFFF.
    bank_select: u8,
    /// Total number of 16 KB PRG banks.
    bank_count: u8,
}

impl Uxrom {
    pub(super) fn new(cart: Cartridge) -> Self {
        let bank_count = (cart.prg_rom().len() / PRG_BANK_SIZE) as u8;
        Self {
            prg_rom: cart.prg_rom().to_vec(),
            chr_ram: vec![0; 8192],
            mirroring: cart.mirroring(),
            bank_select: 0,
            bank_count,
        }
    }
}

impl Mapper for Uxrom {
    fn cpu_read(&self, addr: u16) -> u8 {
        let offset = match addr {
            // Switchable bank.
            0x8000..=0xBFFF => {
                let bank = usize::from(self.bank_select) * PRG_BANK_SIZE;
                bank + usize::from(addr - 0x8000)
            }
            // Fixed to last bank.
            0xC000..=0xFFFF => {
                let last = usize::from(self.bank_count - 1) * PRG_BANK_SIZE;
                last + usize::from(addr - 0xC000)
            }
            _ => return 0,
        };
        self.prg_rom.get(offset).copied().unwrap_or(0)
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if addr >= 0x8000 {
            self.bank_select = val % self.bank_count;
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
        self.mirroring
    }
}
