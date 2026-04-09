//! Camerica / Codemasters (mapper 71) — 16 KB switchable PRG bank.
//!
//! - $8000–$BFFF: switchable 16 KB PRG bank
//! - $C000–$FFFF: fixed to last 16 KB bank
//! - CHR: 8 KB CHR-RAM
//! - Mirroring: static (from cartridge header)
//!
//! Write to $C000–$FFFF: bits 0–3 = PRG bank select.
//!
//! Used by: Micro Machines, Fire Hawk, Bee 52.

use std::sync::Arc;

use super::Mapper;
use crate::nes::cartridge::{Cartridge, Mirroring};

const PRG_BANK_SIZE: usize = 16_384;

/// Camerica mapper — `UxROM` variant for Codemasters games.
#[derive(Clone)]
pub(super) struct Camerica {
    prg_rom: Arc<[u8]>,
    chr_ram: Vec<u8>,
    mirroring: Mirroring,
    bank_select: u8,
    bank_count: u8,
}

impl Camerica {
    pub(super) fn new(cart: Cartridge) -> Self {
        let (prg_rom, _, mirroring) = cart.into_parts();
        let bank_count = (prg_rom.len() / PRG_BANK_SIZE).max(1) as u8;
        Self {
            prg_rom,
            chr_ram: vec![0; 8192],
            mirroring,
            bank_select: 0,
            bank_count,
        }
    }
}

impl Mapper for Camerica {
    fn cpu_read(&self, addr: u16) -> u8 {
        let offset = match addr {
            0x8000..=0xBFFF => {
                usize::from(self.bank_select) * PRG_BANK_SIZE + usize::from(addr - 0x8000)
            }
            0xC000..=0xFFFF => {
                usize::from(self.bank_count - 1) * PRG_BANK_SIZE + usize::from(addr - 0xC000)
            }
            _ => return 0,
        };
        self.prg_rom.get(offset).copied().unwrap_or(0)
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if addr >= 0xC000 {
            self.bank_select = (val & 0x0F) % self.bank_count;
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

    fn box_clone(&self) -> Box<dyn Mapper> {
        Box::new(self.clone())
    }
}
