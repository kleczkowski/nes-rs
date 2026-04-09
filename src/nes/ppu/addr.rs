//! PPU address space — internal VRAM, palette, and mapper routing.
//!
//! The PPU has a 16 KB address space:
//!
//! | Range         | Device                              |
//! |---------------|-------------------------------------|
//! | $0000–$1FFF   | Pattern tables (via mapper CHR)     |
//! | $2000–$2FFF   | Nametables (internal VRAM, mirrored)|
//! | $3000–$3EFF   | Nametable mirrors                   |
//! | $3F00–$3FFF   | Palette RAM                         |

use super::Ppu;
use crate::nes::cartridge::Mirroring;
use crate::nes::mapper::Mapper;

impl Ppu {
    /// Reads a byte from the PPU address space.
    pub(super) fn ppu_read(&self, addr: u16, mapper: &dyn Mapper) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => mapper.ppu_read(addr),
            0x2000..=0x3EFF => {
                let index = mirror_nametable(addr, mapper.mirroring());
                self.vram.get(index).copied().unwrap_or(0)
            }
            0x3F00..=0x3FFF => {
                let index = mirror_palette(addr);
                self.palette.get(index).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }

    /// Writes a byte to the PPU address space.
    pub(super) fn ppu_write(&mut self, addr: u16, val: u8, mapper: &mut dyn Mapper) {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => mapper.ppu_write(addr, val),
            0x2000..=0x3EFF => {
                let index = mirror_nametable(addr, mapper.mirroring());
                if let Some(cell) = self.vram.get_mut(index) {
                    *cell = val;
                }
            }
            0x3F00..=0x3FFF => {
                let index = mirror_palette(addr);
                if let Some(cell) = self.palette.get_mut(index) {
                    *cell = val;
                }
            }
            _ => {}
        }
    }
}

/// Maps a nametable address ($2000–$2FFF) to a VRAM index (0–2047).
///
/// The NES has 2 KB of VRAM for two physical nametable pages.
/// The mirroring mode determines how four logical pages map to
/// the two physical ones.
fn mirror_nametable(addr: u16, mirroring: Mirroring) -> usize {
    // Strip to nametable-relative offset within $2000–$2FFF.
    let addr = (addr - 0x2000) & 0x0FFF;
    // Which of the 4 logical nametables (0–3)?
    let table = addr / 0x0400;
    let offset = addr % 0x0400;

    let physical_table = match mirroring {
        // Horizontal: 0→0, 1→0, 2→1, 3→1
        Mirroring::Horizontal => table / 2,
        // Vertical: 0→0, 1→1, 2→0, 3→1
        Mirroring::Vertical => table & 1,
        // Four-screen: 0→0, 1→1, 2→2, 3→3 (needs 4 KB)
        Mirroring::FourScreen => table,
    };

    usize::from(physical_table * 0x0400 + offset)
}

/// Maps a palette address ($3F00–$3FFF) to a palette RAM index (0–31).
///
/// Sprite palette entry 0 mirrors the background color:
/// $3F10→$3F00, $3F14→$3F04, $3F18→$3F08, $3F1C→$3F0C.
fn mirror_palette(addr: u16) -> usize {
    let mut index = usize::from(addr) & 0x1F;
    // Sprite backdrop mirrors background backdrop.
    if index >= 0x10 && index % 4 == 0 {
        index -= 0x10;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nametable_horizontal_mirroring() {
        let m = Mirroring::Horizontal;
        // Tables 0 and 1 → physical 0.
        assert_eq!(mirror_nametable(0x2000, m), 0x0000);
        assert_eq!(mirror_nametable(0x2400, m), 0x0000);
        // Tables 2 and 3 → physical 1.
        assert_eq!(mirror_nametable(0x2800, m), 0x0400);
        assert_eq!(mirror_nametable(0x2C00, m), 0x0400);
    }

    #[test]
    fn nametable_vertical_mirroring() {
        let m = Mirroring::Vertical;
        assert_eq!(mirror_nametable(0x2000, m), 0x0000);
        assert_eq!(mirror_nametable(0x2400, m), 0x0400);
        assert_eq!(mirror_nametable(0x2800, m), 0x0000);
        assert_eq!(mirror_nametable(0x2C00, m), 0x0400);
    }

    #[test]
    fn palette_mirror_sprite_backdrop() {
        assert_eq!(mirror_palette(0x3F10), 0x00);
        assert_eq!(mirror_palette(0x3F14), 0x04);
        assert_eq!(mirror_palette(0x3F18), 0x08);
        assert_eq!(mirror_palette(0x3F1C), 0x0C);
        // Non-backdrop sprite entries are not mirrored.
        assert_eq!(mirror_palette(0x3F11), 0x11);
        assert_eq!(mirror_palette(0x3F15), 0x15);
    }

    #[test]
    fn palette_wraps_at_32() {
        assert_eq!(mirror_palette(0x3F20), 0x00);
        assert_eq!(mirror_palette(0x3F3F), 0x1F);
    }
}
