//! Picture Processing Unit (2C02).
//!
//! The PPU renders the NES display at 256×240 pixels. It has its own
//! 16 KB address space for pattern tables, nametables, and palettes.
//! The CPU communicates with it through memory-mapped registers at
//! $2000–$2007.

#![allow(dead_code)]

mod addr;
mod palette;
pub(super) mod regs;
mod render;
mod tick;

pub(crate) use tick::TickOutput;

use super::framebuffer::Framebuffer;
use super::mapper::Mapper;

/// Cached sprite data for one scanline position.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SpriteRow {
    /// Low byte of pattern data.
    pub(super) pattern_lo: u8,
    /// High byte of pattern data.
    pub(super) pattern_hi: u8,
    /// Sprite attribute byte (palette, priority, flip).
    pub(super) attr: u8,
    /// X position on screen.
    pub(super) x: u8,
}

/// PPU state: registers, VRAM, OAM, and rendering internals.
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Ppu {
    // ── Memory ───────────────────────────────────────────────
    /// PPU-internal VRAM (2 KB nametable RAM).
    pub(super) vram: [u8; 2048],
    /// Object Attribute Memory (256 bytes, 64 sprites × 4 bytes).
    pub(super) oam: [u8; 256],
    /// Palette RAM (32 bytes).
    pub(super) palette: [u8; 32],

    // ── Timing ───────────────────────────────────────────────
    /// Current scanline (0–261).
    pub(super) scanline: u16,
    /// Current cycle within scanline (0–340).
    pub(super) cycle: u16,
    /// Odd frame toggle (pre-render skips one cycle on odd frames).
    pub(super) odd_frame: bool,

    // ── CPU-facing registers ─────────────────────────────────
    /// $2000 PPUCTRL.
    pub(super) ctrl: u8,
    /// $2001 PPUMASK.
    pub(super) mask: u8,
    /// $2002 PPUSTATUS.
    pub(super) status: u8,
    /// OAM address register ($2003).
    pub(super) oam_addr: u8,

    // ── Internal latches ─────────────────────────────────────
    /// Current VRAM address (15 bits).
    pub(super) v: u16,
    /// Temporary VRAM address (15 bits).
    pub(super) t: u16,
    /// Fine X scroll (3 bits).
    pub(super) fine_x: u8,
    /// Write toggle (false = first write, true = second).
    pub(super) w: bool,
    /// PPUDATA read buffer.
    pub(super) read_buffer: u8,

    // ── Background shift registers ───────────────────────────
    /// Pending nametable byte.
    pub(super) nt_byte: u8,
    /// Pending attribute byte.
    pub(super) at_byte: u8,
    /// Pending pattern table low byte.
    pub(super) pt_low: u8,
    /// Pending pattern table high byte.
    pub(super) pt_high: u8,
    /// Background pattern shift register (low plane).
    pub(super) bg_shift_lo: u16,
    /// Background pattern shift register (high plane).
    pub(super) bg_shift_hi: u16,
    /// Background attribute shift register (low bit).
    pub(super) at_shift_lo: u8,
    /// Background attribute shift register (high bit).
    pub(super) at_shift_hi: u8,
    /// Attribute latch for next tile (low bit).
    pub(super) at_latch_lo: bool,
    /// Attribute latch for next tile (high bit).
    pub(super) at_latch_hi: bool,

    // ── Sprite evaluation ────────────────────────────────────
    /// Secondary OAM (up to 8 sprites for next scanline).
    pub(super) secondary_oam: [u8; 256],
    /// Number of sprites found for next scanline.
    pub(super) sprite_count: u8,
    /// Sprite zero will be on the next scanline.
    pub(super) sprite_zero_next: bool,
    /// Sprite zero is on the current scanline.
    pub(super) sprite_zero_current: bool,
    /// Sprite pattern data for the current scanline.
    pub(super) sprite_patterns: [SpriteRow; 64],
    /// When false, removes the 8-sprite-per-scanline hardware limit.
    pub(crate) sprite_limit: bool,

    // ── NMI tracking ─────────────────────────────────────────
    /// `VBlank` NMI has fired this frame.
    pub(super) nmi_occurred: bool,
}

impl Ppu {
    /// Creates a PPU in its power-on state.
    pub(crate) fn new() -> Self {
        Self {
            vram: [0; 2048],
            oam: [0; 256],
            palette: [0; 32],
            scanline: 0,
            cycle: 0,
            odd_frame: false,
            ctrl: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            v: 0,
            t: 0,
            fine_x: 0,
            w: false,
            read_buffer: 0,
            nt_byte: 0,
            at_byte: 0,
            pt_low: 0,
            pt_high: 0,
            bg_shift_lo: 0,
            bg_shift_hi: 0,
            at_shift_lo: 0,
            at_shift_hi: 0,
            at_latch_lo: false,
            at_latch_hi: false,
            secondary_oam: [0; 256],
            sprite_count: 0,
            sprite_zero_next: false,
            sprite_zero_current: false,
            sprite_patterns: [SpriteRow::default(); 64],
            sprite_limit: true,
            nmi_occurred: false,
        }
    }

    /// Advances the PPU by one cycle.
    ///
    /// Returns a signal indicating whether NMI should fire or a
    /// frame is complete. The caller runs this 3 times per CPU cycle.
    pub(crate) fn tick(&mut self, mapper: &mut dyn Mapper, fb: &mut Framebuffer) -> TickOutput {
        let mut output = TickOutput::Idle;

        match self.scanline {
            // Visible scanlines — render pixels.
            0..=239 => render::render_cycle(self, mapper, fb),
            // VBlank start.
            241 => {
                if self.cycle == 1 {
                    self.status |= 0x80;
                    self.nmi_occurred = true;
                    if self.nmi_enabled() {
                        output = TickOutput::Nmi;
                    }
                }
            }
            // Pre-render scanline — prepare for next frame.
            261 => render::pre_render_cycle(self, mapper),
            // Post-render (240) and VBlank (242–260) — idle.
            _ => {}
        }

        self.advance_cycle(&mut output);
        output
    }

    /// Advances cycle/scanline counters, handling wraparound.
    fn advance_cycle(&mut self, output: &mut TickOutput) {
        self.cycle += 1;
        if self.cycle > 340 {
            self.cycle = 0;
            self.scanline += 1;
            if self.scanline > 261 {
                self.scanline = 0;
                self.odd_frame = !self.odd_frame;
                *output = TickOutput::FrameReady;
            }
        }
    }

    // ── PPUCTRL bit helpers ──────────────────────────────────

    /// Whether NMI is enabled (PPUCTRL bit 7).
    pub(super) fn nmi_enabled(&self) -> bool {
        self.ctrl & 0x80 != 0
    }

    /// Sprite height: 8 or 16 pixels (PPUCTRL bit 5).
    pub(super) fn sprite_height(&self) -> u8 {
        if self.ctrl & 0x20 != 0 { 16 } else { 8 }
    }

    /// Base address of the background pattern table (PPUCTRL bit 4).
    pub(super) fn bg_pattern_addr(&self) -> u16 {
        if self.ctrl & 0x10 != 0 { 0x1000 } else { 0 }
    }

    /// Base address of the sprite pattern table (PPUCTRL bit 3).
    pub(super) fn sprite_pattern_addr(&self) -> u16 {
        if self.ctrl & 0x08 != 0 { 0x1000 } else { 0 }
    }

    /// VRAM address increment per $2007 access (1 or 32).
    pub(super) fn vram_increment(&self) -> u16 {
        if self.ctrl & 0x04 != 0 { 32 } else { 1 }
    }

    // ── PPUMASK bit helpers ──────────────────────────────────

    /// Whether any rendering is enabled (background or sprites).
    pub(super) fn rendering_enabled(&self) -> bool {
        self.mask & 0x18 != 0
    }

    /// Whether background rendering is enabled (PPUMASK bit 3).
    pub(super) fn show_bg(&self) -> bool {
        self.mask & 0x08 != 0
    }

    /// Whether sprite rendering is enabled (PPUMASK bit 4).
    pub(super) fn show_sprites(&self) -> bool {
        self.mask & 0x10 != 0
    }

    /// Whether to show background in leftmost 8 pixels (PPUMASK bit 1).
    pub(super) fn show_bg_left(&self) -> bool {
        self.mask & 0x02 != 0
    }

    /// Whether to show sprites in leftmost 8 pixels (PPUMASK bit 2).
    pub(super) fn show_sprites_left(&self) -> bool {
        self.mask & 0x04 != 0
    }

    /// Writes one byte during OAM DMA.
    pub(crate) fn oam_dma_write(&mut self, val: u8) {
        if let Some(cell) = self.oam.get_mut(usize::from(self.oam_addr)) {
            *cell = val;
        }
        self.oam_addr = self.oam_addr.wrapping_add(1);
    }
}
