//! Stub emulator for frontend testing.
//!
//! Renders an animated SMPTE color bar test pattern without
//! requiring a real CPU/PPU implementation.

#![allow(dead_code)]

use super::{Emulator, Snapshot};
use super::controller::Buttons;
use super::framebuffer::{Framebuffer, HEIGHT, WIDTH};
use super::region::Region;

/// Stub emulator that renders an animated test pattern.
pub(crate) struct StubEmulator {
    fb: Framebuffer,
    frame_count: u32,
}

impl StubEmulator {
    /// Creates a new stub emulator.
    pub(crate) fn new() -> Self {
        Self {
            fb: Framebuffer::new(),
            frame_count: 0,
        }
    }
}

impl Default for StubEmulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Emulator for StubEmulator {
    fn update(&mut self, _dt_ms: f64) {
        let shift = self.frame_count as usize;
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                // Scrolling color bars: each 32-pixel column gets
                // a different hue, and the pattern shifts each frame.
                let col = (x.wrapping_add(shift)) / 32;
                let rgb: [u8; 3] = match col % 8 {
                    0 => [255, 255, 255], // white
                    1 => [255, 255, 0],   // yellow
                    2 => [0, 255, 255],   // cyan
                    3 => [0, 255, 0],     // green
                    4 => [255, 0, 255],   // magenta
                    5 => [255, 0, 0],     // red
                    6 => [0, 0, 255],     // blue
                    _ => [0, 0, 0],       // black
                };
                // Dim the bottom third to simulate a brightness gradient.
                let dim: u16 = if y > HEIGHT * 2 / 3 {
                    128
                } else if y > HEIGHT / 3 {
                    192
                } else {
                    255
                };
                self.fb.set_pixel(
                    x,
                    y,
                    [
                        (u16::from(rgb[0]) * dim / 255) as u8,
                        (u16::from(rgb[1]) * dim / 255) as u8,
                        (u16::from(rgb[2]) * dim / 255) as u8,
                    ],
                );
            }
        }
        self.frame_count = self.frame_count.wrapping_add(1);
    }

    fn framebuffer(&self) -> &Framebuffer {
        &self.fb
    }

    fn set_buttons(&mut self, _player: u8, _buttons: Buttons) {}

    fn set_sprite_limit(&mut self, _enabled: bool) {}

    fn region(&self) -> Region {
        Region::default()
    }

    fn set_region_override(&mut self, _region: Option<Region>) {}

    fn load_rom(&mut self, _data: &[u8]) -> anyhow::Result<()> {
        Ok(())
    }

    fn reset(&mut self) {}

    fn snapshot(&self) -> Option<Snapshot> {
        None
    }

    fn restore(&mut self, _snapshot: &Snapshot) {}
}
