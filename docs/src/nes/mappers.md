# Cartridges & Mappers

## iNES format

NES ROM images use the **iNES format**, which starts with a 16-byte header followed by the ROM data:

| Offset | Size | Content |
|--------|------|---------|
| 0–3 | 4 bytes | Magic: `NES\x1A` |
| 4 | 1 byte | PRG-ROM size in 16 KB units |
| 5 | 1 byte | CHR-ROM size in 8 KB units (0 = CHR-RAM) |
| 6 | 1 byte | Flags 6 — mapper low nibble, mirroring, trainer |
| 7 | 1 byte | Flags 7 — mapper high nibble |
| 8 | 1 byte | PRG-RAM size (rarely used) |
| 9 | 1 byte | Flags 9 — TV system (bit 0: 0 = NTSC, 1 = PAL) |
| 10–15 | 6 bytes | Padding |

The mapper ID is assembled from the high nibble of flags 6 (low 4 bits) and the high nibble of flags 7 (high 4 bits): `mapper = (flags7 & 0xF0) | (flags6 >> 4)`.

nes-rs parses the iNES header using **nom** parser combinators in `src/nes/cartridge.rs`. A 512-byte trainer section (between header and PRG-ROM) is supported but ignored.

## What is a mapper?

The NES CPU can only address 64 KB, and the PPU can only address 16 KB. Many games contain more ROM than fits in these address spaces. **Mappers** are bank-switching circuits on the cartridge that dynamically swap pages of ROM into the CPU and PPU address windows.

Different games use different mapper circuits. Each mapper has a unique numbering scheme (the "mapper ID") from the iNES format. nes-rs implements the mapper as a trait:

```rust
pub(crate) trait Mapper {
    fn cpu_read(&self, addr: u16) -> u8;
    fn cpu_write(&mut self, addr: u16, val: u8);
    fn ppu_read(&self, addr: u16) -> u8;
    fn ppu_write(&mut self, addr: u16, val: u8);
    fn mirroring(&self) -> Mirroring;
    fn notify_scanline(&mut self) {}  // For scanline counters
    fn irq_pending(&self) -> bool { false }
    fn irq_clear(&mut self) {}
}
```

## Supported mappers

### Mapper 0 — NROM

The simplest mapper. No bank switching at all.

- **PRG**: 16 KB or 32 KB, mapped directly to `$8000`–`$FFFF`. With 16 KB, the bank is mirrored at `$C000`.
- **CHR**: 8 KB of CHR-ROM (or CHR-RAM) mapped to `$0000`–`$1FFF`.
- **Games**: Donkey Kong, Super Mario Bros., Excitebike.

### Mapper 1 — MMC1 (SxROM)

A common mapper with a serial shift register interface. The CPU writes one bit at a time (5 writes to build a register value).

- **PRG**: 256 KB, switchable in 16 KB or 32 KB modes.
- **CHR**: 128 KB, switchable in 4 KB or 8 KB modes.
- **Mirroring**: Dynamically configurable (horizontal, vertical, single-screen).
- **Games**: The Legend of Zelda, Metroid, Mega Man 2.

### Mapper 2 — UxROM

Simple PRG bank switching with fixed CHR.

- **PRG**: Switchable 16 KB bank at `$8000`, fixed last bank at `$C000`.
- **CHR**: 8 KB CHR-RAM.
- **Games**: Castlevania, Contra, Duck Tales.

### Mapper 3 — CNROM

Simple CHR bank switching with fixed PRG.

- **PRG**: Fixed 16 KB or 32 KB.
- **CHR**: Switchable 8 KB CHR-ROM bank.
- **Games**: Arkanoid, Gradius, Paperboy.

### Mapper 4 — MMC3 (TxROM)

The most complex and widely-used mapper. Features fine-grained bank switching and a scanline counter with IRQ.

- **PRG**: Up to 512 KB, switchable in 8 KB banks.
- **CHR**: Up to 256 KB, switchable in 1 KB and 2 KB banks.
- **Mirroring**: Dynamically configurable.
- **Scanline counter**: Counts PPU scanlines by monitoring address line A12. Fires an IRQ after a programmed number of scanlines, enabling raster effects (split-screen scrolling, status bars).
- **Games**: Super Mario Bros. 2 and 3, Kirby's Adventure, Mega Man 3–6.

### Mapper 7 — AxROM

32 KB PRG bank switching with single-screen mirroring.

- **PRG**: Switchable 32 KB bank.
- **CHR**: 8 KB CHR-RAM.
- **Mirroring**: Single-screen, dynamically selectable (low or high page).
- **Games**: Battletoads, Marble Madness.

### Mapper 9 — MMC2

Unusual mapper with automatic CHR bank switching triggered by specific tile fetches.

- **PRG**: 8 KB switchable bank at `$8000`, three fixed 8 KB banks.
- **CHR**: Two pairs of 4 KB banks that swap based on which tile the PPU reads.
- **Games**: Mike Tyson's Punch-Out!!

### Mapper 11 — Color Dreams

Simple unlicensed mapper.

- **PRG**: Switchable 32 KB bank.
- **CHR**: Switchable 8 KB bank.
- **Games**: Various unlicensed titles (Bible Adventures, Crystal Mines).

### Mapper 66 — GxROM

Combined PRG and CHR bank switching via a single register.

- **PRG**: Switchable 32 KB bank (2 bits).
- **CHR**: Switchable 8 KB bank (2 bits).
- **Games**: Super Mario Bros. / Duck Hunt combo cart.

### Mapper 71 — Camerica

Used by Camerica/Codemasters games.

- **PRG**: Switchable 16 KB bank at `$8000`, fixed last bank at `$C000`.
- **CHR**: Fixed 8 KB CHR-RAM.
- **Games**: Fire Hawk, Micro Machines.

## Nametable mirroring

The cartridge controls how the PPU's four logical nametables map to the 2 KB of physical VRAM:

| Mode | Logical → Physical | Use case |
|------|-------------------|----------|
| Horizontal | 0→A, 1→A, 2→B, 3→B | Vertical scrolling |
| Vertical | 0→A, 1→B, 2→A, 3→B | Horizontal scrolling |
| Single-screen (low) | All → A | Special effects |
| Single-screen (high) | All → B | Special effects |
| Four-screen | 0→A, 1→B, 2→C, 3→D | Rare (requires 4 KB VRAM) |

The mirroring mode is either hardwired by the cartridge PCB or dynamically switchable by the mapper.

## CHR-ROM vs CHR-RAM

Games with CHR-ROM store all tile graphics in read-only memory on the cartridge. The mapper switches banks to display different tiles.

Games with CHR-RAM (indicated by 0 CHR banks in the iNES header) have 8 KB of writable character memory. The game copies tile data from PRG-ROM into CHR-RAM at runtime. This is common with mapper 2 (UxROM) games.
