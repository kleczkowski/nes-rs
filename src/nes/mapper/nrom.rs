//! NROM (mapper 0) — no bank switching.
//!
//! The simplest NES mapper. PRG-ROM is fixed at $8000–$FFFF:
//! - 32 KB ROM fills the entire window
//! - 16 KB ROM mirrors into both halves
//!
//! CHR-ROM (or CHR-RAM) is fixed at PPU $0000–$1FFF.

use std::sync::Arc;

use super::Mapper;
use crate::nes::cartridge::{Cartridge, Mirroring};

/// PRG-ROM start address in CPU space.
const PRG_START: u16 = 0x8000;

/// NROM mapper — fixed PRG and CHR banks.
#[derive(Clone)]
pub(super) struct Nrom {
    prg_rom: Arc<[u8]>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    mirroring: Mirroring,
}

impl Nrom {
    pub(super) fn new(cart: Cartridge) -> Self {
        let (prg_rom, chr_rom, mirroring) = cart.into_parts();
        let chr_is_ram = chr_rom.is_empty();
        let chr = if chr_is_ram {
            vec![0; 8192]
        } else {
            chr_rom
        };
        Self {
            prg_rom,
            chr,
            chr_is_ram,
            mirroring,
        }
    }
}

impl Mapper for Nrom {
    fn cpu_read(&self, addr: u16) -> u8 {
        if addr < PRG_START {
            return 0;
        }
        let offset = usize::from(addr - PRG_START) % self.prg_rom.len();
        self.prg_rom.get(offset).copied().unwrap_or(0)
    }

    fn cpu_write(&mut self, _addr: u16, _val: u8) {
        // NROM has no writable registers.
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        let offset = usize::from(addr) % self.chr.len();
        self.chr.get(offset).copied().unwrap_or(0)
    }

    fn ppu_write(&mut self, addr: u16, val: u8) {
        if self.chr_is_ram {
            let offset = usize::from(addr) % self.chr.len();
            if let Some(cell) = self.chr.get_mut(offset) {
                *cell = val;
            }
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn box_clone(&self) -> Box<dyn Mapper> {
        Box::new(self.clone())
    }
}
