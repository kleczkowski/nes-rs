//! Mapper trait and implementations for NES cartridge bank switching.
//!
//! Each mapper defines how the cartridge's PRG-ROM and CHR-ROM are
//! mapped into the CPU and PPU address spaces. The [`Mapper`] trait
//! abstracts over different bank-switching hardware.

#![allow(dead_code, clippy::needless_pass_by_value)]

mod cnrom;
mod mmc1;
mod nrom;
mod uxrom;

use super::cartridge::{Cartridge, Mirroring};

/// Abstraction over NES cartridge mapper hardware.
///
/// The CPU bus delegates cartridge-space reads and writes to the
/// mapper, which translates addresses into offsets within the
/// cartridge's PRG-ROM and CHR-ROM.
pub(crate) trait Mapper {
    /// Reads a byte from the CPU address space ($4020–$FFFF).
    fn cpu_read(&self, addr: u16) -> u8;

    /// Writes a byte to the CPU address space ($4020–$FFFF).
    ///
    /// Many mappers use writes to the ROM region to control
    /// bank-switching registers.
    fn cpu_write(&mut self, addr: u16, val: u8);

    /// Reads a byte from the PPU address space ($0000–$1FFF).
    fn ppu_read(&self, addr: u16) -> u8;

    /// Writes a byte to the PPU address space ($0000–$1FFF).
    ///
    /// Only applicable when CHR-RAM is used instead of CHR-ROM.
    fn ppu_write(&mut self, addr: u16, val: u8);

    /// Returns the current nametable mirroring mode.
    ///
    /// Some mappers (e.g., MMC1) can change mirroring dynamically.
    fn mirroring(&self) -> Mirroring;
}

/// Creates a boxed mapper for the given cartridge.
///
/// # Errors
///
/// Returns an error if the cartridge's mapper ID is not supported.
pub(crate) fn from_cartridge(cart: Cartridge) -> anyhow::Result<Box<dyn Mapper>> {
    let id = cart.mapper_id();
    let mapper: Box<dyn Mapper> = match id {
        0 => Box::new(nrom::Nrom::new(cart)),
        1 => Box::new(mmc1::Mmc1::new(cart)),
        2 => Box::new(uxrom::Uxrom::new(cart)),
        3 => Box::new(cnrom::Cnrom::new(cart)),
        _ => {
            tracing::error!(mapper_id = id, "unsupported mapper");
            anyhow::bail!("unsupported mapper: {id}");
        }
    };
    tracing::info!(mapper_id = id, "mapper initialized");
    Ok(mapper)
}
