# PPU — Picture Processing Unit

The PPU (Ricoh 2C02) is responsible for generating the NES's 256x240 pixel video output. It has its own 16 KB address space, separate from the CPU, and renders two layers: a scrollable background and up to 64 independently positioned sprites.

## PPU memory

### Internal memory

| Memory | Size | Purpose |
|--------|------|---------|
| VRAM (nametable RAM) | 2 KB | Stores two nametable pages (tile maps) |
| OAM (Object Attribute Memory) | 256 bytes | Stores 64 sprite entries (4 bytes each) |
| Palette RAM | 32 bytes | Stores color indices for background and sprites |

### PPU address space (16 KB)

| Range | Size | Device |
|-------|------|--------|
| `$0000`–`$0FFF` | 4 KB | Pattern table 0 (CHR-ROM/RAM via mapper) |
| `$1000`–`$1FFF` | 4 KB | Pattern table 1 (CHR-ROM/RAM via mapper) |
| `$2000`–`$23FF` | 1 KB | Nametable 0 |
| `$2400`–`$27FF` | 1 KB | Nametable 1 |
| `$2800`–`$2BFF` | 1 KB | Nametable 2 (mirror) |
| `$2C00`–`$2FFF` | 1 KB | Nametable 3 (mirror) |
| `$3000`–`$3EFF` | — | Mirror of `$2000`–`$2EFF` |
| `$3F00`–`$3F1F` | 32 bytes | Palette RAM |
| `$3F20`–`$3FFF` | — | Palette mirrors |

Since the NES only has 2 KB of VRAM, the four logical nametables are mapped to two physical pages using a **mirroring mode** controlled by the cartridge.

## CPU-facing registers

The CPU communicates with the PPU through 8 memory-mapped registers at `$2000`–`$2007`, mirrored across `$2008`–`$3FFF`:

### $2000 — PPUCTRL (write-only)

```text
  7  6  5  4  3  2  1  0
  V  .  H  B  S  I  N  N
```

| Bit | Name | Description |
|-----|------|-------------|
| 0–1 | Nametable select | Base nametable address (`$2000`, `$2400`, `$2800`, `$2C00`) |
| 2 | VRAM increment | 0 = add 1 (across), 1 = add 32 (down) |
| 3 | Sprite pattern table | 0 = `$0000`, 1 = `$1000` (8x8 sprites only) |
| 4 | Background pattern table | 0 = `$0000`, 1 = `$1000` |
| 5 | Sprite height | 0 = 8x8, 1 = 8x16 |
| 7 | NMI enable | Generate NMI at start of VBlank |

### $2001 — PPUMASK (write-only)

```text
  7  6  5  4  3  2  1  0
  B  G  R  s  b  M  m  G
```

| Bit | Description |
|-----|-------------|
| 1 | Show background in leftmost 8 pixels |
| 2 | Show sprites in leftmost 8 pixels |
| 3 | Show background |
| 4 | Show sprites |

### $2002 — PPUSTATUS (read-only)

```text
  7  6  5  .  .  .  .  .
  V  S  O
```

| Bit | Description |
|-----|-------------|
| 5 | Sprite overflow — more than 8 sprites on a scanline |
| 6 | Sprite 0 hit — sprite 0's opaque pixel overlaps an opaque BG pixel |
| 7 | VBlank — set at scanline 241, cleared on read or at pre-render |

Reading `$2002` clears the VBlank flag and resets the write toggle used by `$2005` and `$2006`.

### $2005 — PPUSCROLL (write x2)

Sets the scroll position. Uses a double-write mechanism:
- **First write** — X scroll (coarse X + fine X)
- **Second write** — Y scroll (coarse Y + fine Y)

### $2006 — PPUADDR (write x2)

Sets the VRAM address for `$2007` reads/writes:
- **First write** — High byte (bits 8–13)
- **Second write** — Low byte (bits 0–7), then copies to active address

### $2007 — PPUDATA (read/write)

Reads or writes VRAM at the current address. After each access, the address increments by 1 or 32 (controlled by PPUCTRL bit 2). Non-palette reads are buffered — the first read returns stale data from an internal buffer.

## Background rendering

The background is composed of **nametables** — 32x30 grids of 8x8 tile indices. Each nametable byte references a tile in the pattern table (CHR-ROM), and an associated **attribute table** selects which of 4 palettes is used for each 16x16 pixel region.

### The 8-cycle fetch pipeline

For each 8-pixel tile, the PPU performs 4 fetches over 8 cycles:

| Cycle | Fetch |
|-------|-------|
| 1 | Nametable byte — which tile to draw |
| 3 | Attribute byte — which palette to use |
| 5 | Pattern table low byte — pixel data (bit plane 0) |
| 7 | Pattern table high byte — pixel data (bit plane 1) |
| 0 | Increment horizontal scroll |

The two pattern bytes form a 2-bit-per-pixel row. Combined with the 2-bit palette selection from the attribute byte, each pixel has a 4-bit index into palette RAM.

### Shift registers

The PPU uses 16-bit shift registers to hold two tiles of pattern data at once, allowing it to output one pixel per cycle while fetching the next tile in the background. The fine X scroll (0–7) selects which bit of the shift register is output.

### Scrolling

The PPU's internal `v` register (15 bits) encodes both the VRAM address and scroll position:

```text
  14  13 12   11 10   9  8  7  6  5   4  3  2  1  0
  ─── fine Y ───  NN  ──── coarse Y ────  ── coarse X ──
```

- **Fine Y** (bits 12–14): pixel offset within the current tile row (0–7)
- **NN** (bits 10–11): nametable select
- **Coarse Y** (bits 5–9): tile row (0–29)
- **Coarse X** (bits 0–4): tile column (0–31)

The separate `t` register holds the "target" scroll and is copied to `v` at specific points during rendering.

## Sprite rendering

### OAM (Object Attribute Memory)

Each of the 64 sprites has a 4-byte entry in OAM:

| Byte | Content |
|------|---------|
| 0 | Y position (top edge, minus 1) |
| 1 | Tile index |
| 2 | Attributes: palette (bits 0–1), priority (bit 5), horizontal flip (bit 6), vertical flip (bit 7) |
| 3 | X position (left edge) |

### Sprite evaluation

At cycle 257 of each visible scanline, the PPU evaluates which sprites intersect the **current** scanline. Due to the Y-minus-1 convention in OAM, this naturally selects sprites for the next scanline's rendering:

1. Scan all 64 OAM entries.
2. For each sprite where `scanline - sprite_y < sprite_height`, copy it to secondary OAM.
3. If more than 8 sprites are found, set the sprite overflow flag in PPUSTATUS.
4. Track whether sprite 0 is in the set (for sprite 0 hit detection).

### Sprite 0 hit

When sprite 0's non-transparent pixel overlaps a non-transparent background pixel during rendering, the PPU sets the sprite 0 hit flag in PPUSTATUS (bit 6). Games use this as a raster timing trick — for example, to split the screen into a scrolling playfield and a fixed status bar.

### 8-sprite limit

The real NES hardware can only render 8 sprites per scanline. Additional sprites are dropped, causing the characteristic "flickering" that games use to work around the limit (by rotating which sprites are visible each frame). nes-rs enforces this limit by default but allows disabling it via the configuration panel.

## Timing

The PPU operates on a cycle grid of **341 cycles x 262 scanlines** (NTSC) or **341 cycles x 312 scanlines** (PAL):

| Scanlines | Purpose |
|-----------|---------|
| 0–239 | Visible — render pixels and fetch tile data |
| 240 | Post-render — idle |
| 241 | VBlank start — set VBlank flag, fire NMI |
| 242–260 | VBlank — CPU can safely access VRAM |
| 261 (NTSC) / 311 (PAL) | Pre-render — clear flags, prepare for next frame |

### Odd frame skip

On NTSC, the pre-render scanline is one cycle shorter on odd frames (339 cycles instead of 340). This subtle timing quirk keeps the PPU and CPU synchronized and prevents the picture from jittering. PAL does not have this behavior.

## Color palette

The NES has a fixed palette of **64 colors** (6-bit indices). nes-rs uses the "Wavebeam" palette, which is widely considered the most visually accurate NTSC approximation. The palette is hardcoded — the PPU's palette RAM stores 6-bit indices into this fixed table, not RGB values directly.

Background and sprite palettes each have 4 sub-palettes of 4 colors. Color 0 of each sub-palette is transparent and shares the universal backdrop color at `$3F00`.

## Nametable mirroring

The 2 KB of VRAM provides two physical nametable pages. The mirroring mode maps four logical nametables to these two pages:

| Mode | Mapping | Common use |
|------|---------|-----------|
| **Horizontal** | 0=A, 1=A, 2=B, 3=B | Vertical scrolling games |
| **Vertical** | 0=A, 1=B, 2=A, 3=B | Horizontal scrolling games |
| **Single-screen** | All four map to one page | Special mapper modes |
| **Four-screen** | 4 KB cartridge VRAM | Rare; simultaneous 4-way scroll |

The mirroring mode is set by the cartridge hardware. Some mappers (MMC1, AxROM) can change it dynamically.
