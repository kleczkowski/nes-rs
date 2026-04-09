//! Per-cycle background and sprite rendering.
//!
//! Called from [`Ppu::tick()`] for visible scanlines (0–239) and the
//! pre-render scanline (261). Handles background tile fetching, shift
//! register updates, sprite evaluation, and pixel output.

use super::Ppu;
use super::palette;
use crate::nes::framebuffer::Framebuffer;
use crate::nes::mapper::Mapper;

// ── Public entry points ──────────────────────────────────────────

/// Executes one cycle of a visible scanline (0–239).
pub(super) fn render_cycle(ppu: &mut Ppu, mapper: &dyn Mapper, fb: &mut Framebuffer) {
    // Promote the sprite-0-on-this-scanline flag BEFORE rendering
    // starts, so hit detection at cycles 1–255 sees the right value.
    if ppu.cycle == 0 {
        ppu.sprite_zero_current = ppu.sprite_zero_next;
    }

    if ppu.rendering_enabled() {
        // Cycles 1–256: fetch BG tiles and output pixels.
        if ppu.cycle >= 1 && ppu.cycle <= 256 {
            output_pixel(ppu, fb);
            fetch_bg_step(ppu, mapper);
        }

        // Cycle 256: increment vertical scroll.
        if ppu.cycle == 256 {
            increment_y(ppu);
        }

        // Cycle 257: copy horizontal bits from t to v, evaluate
        // sprites.  OAM Y is off-by-one (games write desired_Y − 1),
        // so using the current scanline naturally finds sprites for
        // the next scanline's rendering.
        if ppu.cycle == 257 {
            copy_horizontal(ppu);
            evaluate_sprites(ppu, mapper, ppu.scanline);
        }

        // Cycles 321–336: prefetch first two tiles of next scanline.
        if ppu.cycle >= 321 && ppu.cycle <= 336 {
            fetch_bg_step(ppu, mapper);
        }
    }
}

/// Executes one cycle of the pre-render scanline (261).
pub(super) fn pre_render_cycle(ppu: &mut Ppu, mapper: &dyn Mapper) {
    // Cycle 1: clear VBlank, sprite 0 hit, and overflow flags.
    if ppu.cycle == 1 {
        ppu.status &= !0xE0;
        ppu.nmi_occurred = false;
    }

    if ppu.rendering_enabled() {
        if ppu.cycle >= 1 && ppu.cycle <= 256 {
            fetch_bg_step(ppu, mapper);
        }

        if ppu.cycle == 256 {
            increment_y(ppu);
        }

        if ppu.cycle == 257 {
            copy_horizontal(ppu);
            evaluate_sprites(ppu, mapper, ppu.scanline);
        }

        // Cycles 280–304: repeatedly copy vertical bits from t to v.
        if ppu.cycle >= 280 && ppu.cycle <= 304 {
            copy_vertical(ppu);
        }

        if ppu.cycle >= 321 && ppu.cycle <= 336 {
            fetch_bg_step(ppu, mapper);
        }

        // Odd frame cycle skip (NTSC only).
        if ppu.odd_frame_skip && ppu.cycle == 339 && ppu.odd_frame {
            ppu.cycle = 340;
        }
    }
}

// ── Background fetching ──────────────────────────────────────────

/// One step of the 8-cycle background tile fetch pipeline.
fn fetch_bg_step(ppu: &mut Ppu, mapper: &dyn Mapper) {
    shift_bg_registers(ppu);

    match ppu.cycle % 8 {
        // Nametable byte.
        1 => {
            load_shift_registers(ppu);
            let nt_addr = 0x2000 | (ppu.v & 0x0FFF);
            ppu.nt_byte = ppu.ppu_read(nt_addr, mapper);
        }
        // Attribute byte.
        3 => {
            let v = ppu.v;
            let at_addr = 0x23C0 | (v & 0x0C00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x07);
            let at = ppu.ppu_read(at_addr, mapper);
            let shift = ((v >> 4) & 0x04) | (v & 0x02);
            ppu.at_byte = (at >> shift) & 0x03;
        }
        // Pattern table low byte.
        5 => {
            let fine_y = (ppu.v >> 12) & 0x07;
            let tile = u16::from(ppu.nt_byte);
            let addr = ppu.bg_pattern_addr() + tile * 16 + fine_y;
            ppu.pt_low = ppu.ppu_read(addr, mapper);
        }
        // Pattern table high byte.
        7 => {
            let fine_y = (ppu.v >> 12) & 0x07;
            let tile = u16::from(ppu.nt_byte);
            let addr = ppu.bg_pattern_addr() + tile * 16 + fine_y + 8;
            ppu.pt_high = ppu.ppu_read(addr, mapper);
        }
        0 => increment_x(ppu),
        _ => {}
    }
}

/// Shifts the background and attribute shift registers by one.
fn shift_bg_registers(ppu: &mut Ppu) {
    ppu.bg_shift_lo <<= 1;
    ppu.bg_shift_hi <<= 1;
    ppu.at_shift_lo = (ppu.at_shift_lo << 1) | u8::from(ppu.at_latch_lo);
    ppu.at_shift_hi = (ppu.at_shift_hi << 1) | u8::from(ppu.at_latch_hi);
}

/// Loads new tile data into the low 8 bits of the shift registers.
fn load_shift_registers(ppu: &mut Ppu) {
    ppu.bg_shift_lo = (ppu.bg_shift_lo & 0xFF00) | u16::from(ppu.pt_low);
    ppu.bg_shift_hi = (ppu.bg_shift_hi & 0xFF00) | u16::from(ppu.pt_high);
    ppu.at_latch_lo = ppu.at_byte & 0x01 != 0;
    ppu.at_latch_hi = ppu.at_byte & 0x02 != 0;
}

// ── Pixel output ─────────────────────────────────────────────────

/// Composes one pixel from BG + sprite data and writes to framebuffer.
fn output_pixel(ppu: &mut Ppu, fb: &mut Framebuffer) {
    let x = ppu.cycle - 1;
    let y = ppu.scanline;

    let bg_pixel = bg_pixel(ppu, x);
    let (sp_pixel, sp_attr, sp_is_zero) = sprite_pixel(ppu, x);

    let pal_index = compose_pixel(ppu, bg_pixel, sp_pixel, sp_attr, sp_is_zero);
    // Look up the 6-bit system color from palette RAM.
    let system_color = ppu
        .palette
        .get(usize::from(pal_index) & 0x1F)
        .copied()
        .unwrap_or(0);
    let color = palette::lookup(system_color);
    fb.set_pixel(usize::from(x), usize::from(y), color);
}

/// Extracts the current background pixel (2-bit color + 2-bit palette).
fn bg_pixel(ppu: &Ppu, x: u16) -> u8 {
    if !ppu.show_bg() || (x < 8 && !ppu.show_bg_left()) {
        return 0;
    }
    let shift = 15 - u16::from(ppu.fine_x);
    let lo = ((ppu.bg_shift_lo >> shift) & 1) as u8;
    let hi = ((ppu.bg_shift_hi >> shift) & 1) as u8;
    let pixel = (hi << 1) | lo;
    if pixel == 0 {
        return 0;
    }
    let at_shift = 7 - ppu.fine_x;
    let at_lo = (ppu.at_shift_lo >> at_shift) & 1;
    let at_hi = (ppu.at_shift_hi >> at_shift) & 1;
    let palette_id = (at_hi << 1) | at_lo;
    (palette_id << 2) | pixel
}

/// Finds the highest-priority sprite pixel at the given X coordinate.
fn sprite_pixel(ppu: &Ppu, x: u16) -> (u8, u8, bool) {
    if !ppu.show_sprites() || (x < 8 && !ppu.show_sprites_left()) {
        return (0, 0, false);
    }
    for i in 0..usize::from(ppu.sprite_count) {
        let Some(sp) = ppu.sprite_patterns.get(i) else {
            break;
        };
        let offset = x.wrapping_sub(u16::from(sp.x));
        if offset >= 8 {
            continue;
        }
        let shift = 7 - offset as u8;
        let lo = (sp.pattern_lo >> shift) & 1;
        let hi = (sp.pattern_hi >> shift) & 1;
        let pixel = (hi << 1) | lo;
        if pixel == 0 {
            continue;
        }
        let palette_id = (sp.attr & 0x03) + 4;
        let color = (palette_id << 2) | pixel;
        let is_zero = i == 0 && ppu.sprite_zero_current;
        return (color, sp.attr, is_zero);
    }
    (0, 0, false)
}

/// Composes final pixel from BG and sprite, applying priority.
fn compose_pixel(ppu: &mut Ppu, bg: u8, sp: u8, sp_attr: u8, sp_is_zero: bool) -> u8 {
    let bg_opaque = bg & 0x03 != 0;
    let sp_opaque = sp & 0x03 != 0;

    // Sprite zero hit detection.
    if sp_is_zero && bg_opaque && sp_opaque && ppu.cycle != 256 {
        ppu.status |= 0x40;
    }

    match (bg_opaque, sp_opaque) {
        (false, false) => 0, // both transparent → backdrop
        (false, true) => sp,
        (true, false) => bg,
        (true, true) => {
            // Sprite behind BG (priority bit set) → show BG.
            if sp_attr & 0x20 != 0 { bg } else { sp }
        }
    }
}

// ── Sprite evaluation (simplified) ───────────────────────────────

/// Evaluates which sprites are visible on `target_scanline`.
///
/// Called at cycle 257 with the *next* scanline so that patterns
/// are ready when that scanline starts rendering.
fn evaluate_sprites(ppu: &mut Ppu, mapper: &dyn Mapper, target_scanline: u16) {
    let height = u16::from(ppu.sprite_height());
    let y = target_scanline;

    ppu.sprite_count = 0;
    ppu.sprite_zero_next = false;
    ppu.secondary_oam = [0xFF; 256];

    for i in 0u8..64 {
        let base = usize::from(i) * 4;
        let Some(&sprite_y) = ppu.oam.get(base) else {
            continue;
        };
        let row = y.wrapping_sub(u16::from(sprite_y));
        if row >= height {
            continue;
        }
        if ppu.sprite_count >= 8 {
            ppu.status |= 0x20; // sprite overflow
            if ppu.sprite_limit {
                break;
            }
        }
        if i == 0 {
            ppu.sprite_zero_next = true;
        }
        let idx = usize::from(ppu.sprite_count) * 4;
        for off in 0..4 {
            if let (Some(dst), Some(&src)) = (
                ppu.secondary_oam.get_mut(idx + off),
                ppu.oam.get(base + off),
            ) {
                *dst = src;
            }
        }
        ppu.sprite_count += 1;
    }

    load_sprite_patterns(ppu, mapper, y, height);
}

/// Fetches pattern data for each sprite in secondary OAM.
fn load_sprite_patterns(ppu: &mut Ppu, mapper: &dyn Mapper, scanline: u16, height: u16) {
    for i in 0..usize::from(ppu.sprite_count) {
        let base = i * 4;
        let sprite_y = ppu.secondary_oam.get(base).copied().unwrap_or(0xFF);
        let tile_idx = ppu.secondary_oam.get(base + 1).copied().unwrap_or(0);
        let attr = ppu.secondary_oam.get(base + 2).copied().unwrap_or(0);
        let sprite_x = ppu.secondary_oam.get(base + 3).copied().unwrap_or(0);

        let mut row = scanline.wrapping_sub(u16::from(sprite_y));
        let flip_v = attr & 0x80 != 0;
        if flip_v {
            row = height - 1 - row;
        }

        let (pattern_addr, row_in_tile) = if height == 16 {
            let bank = u16::from(tile_idx & 0x01) * 0x1000;
            let tile = u16::from(tile_idx & 0xFE);
            let (t, r) = if row < 8 {
                (tile, row)
            } else {
                (tile + 1, row - 8)
            };
            (bank + t * 16 + r, r)
        } else {
            (
                ppu.sprite_pattern_addr() + u16::from(tile_idx) * 16 + row,
                row,
            )
        };
        let _ = row_in_tile; // used only for the addr calculation above

        let mut lo = ppu.ppu_read(pattern_addr, mapper);
        let mut hi = ppu.ppu_read(pattern_addr + 8, mapper);

        // Horizontal flip.
        if attr & 0x40 != 0 {
            lo = lo.reverse_bits();
            hi = hi.reverse_bits();
        }

        if let Some(sp) = ppu.sprite_patterns.get_mut(i) {
            *sp = super::SpriteRow {
                pattern_lo: lo,
                pattern_hi: hi,
                attr,
                x: sprite_x,
            };
        }
    }
}

// ── Scroll helpers ───────────────────────────────────────────────

/// Increments the coarse X scroll in v.
fn increment_x(ppu: &mut Ppu) {
    if (ppu.v & 0x001F) == 31 {
        ppu.v &= !0x001F;
        ppu.v ^= 0x0400; // switch horizontal nametable
    } else {
        ppu.v += 1;
    }
}

/// Increments the Y scroll in v (coarse Y + fine Y).
fn increment_y(ppu: &mut Ppu) {
    if (ppu.v & 0x7000) == 0x7000 {
        ppu.v &= !0x7000; // fine Y = 0
        let mut coarse_y = (ppu.v & 0x03E0) >> 5;
        if coarse_y == 29 {
            coarse_y = 0;
            ppu.v ^= 0x0800; // switch vertical nametable
        } else if coarse_y == 31 {
            coarse_y = 0; // no nametable switch
        } else {
            coarse_y += 1;
        }
        ppu.v = (ppu.v & !0x03E0) | (coarse_y << 5);
    } else {
        ppu.v += 0x1000; // fine Y < 7, increment
    }
}

/// Copies horizontal scroll bits from t to v.
fn copy_horizontal(ppu: &mut Ppu) {
    ppu.v = (ppu.v & !0x041F) | (ppu.t & 0x041F);
}

/// Copies vertical scroll bits from t to v.
fn copy_vertical(ppu: &mut Ppu) {
    ppu.v = (ppu.v & !0x7BE0) | (ppu.t & 0x7BE0);
}
