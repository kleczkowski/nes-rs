# NTSC vs PAL

The NES was sold in two main variants: **NTSC** (Americas, Japan) and **PAL** (Europe, Australia). The differences stem from the TV standards used in each region, which dictate the master clock frequency and thus cascade into every timing-sensitive subsystem.

## Timing comparison

| Parameter | NTSC | PAL |
|-----------|------|-----|
| Master clock | 21.477272 MHz | 26.601712 MHz |
| CPU clock | 1,789,773 Hz | 1,662,607 Hz |
| PPU dots per CPU cycle | 3 | 3.2 (16/5) |
| Scanlines per frame | 262 | 312 |
| Pre-render scanline | 261 | 311 |
| Visible scanlines | 240 | 240 |
| VBlank scanlines | 20 | 70 |
| Frame rate | ~60.0988 Hz | ~50.0070 Hz |
| Odd frame skip | Yes | No |
| APU sequencer step | 7,457 cycles | 8,313 cycles |

## How regions affect emulation

### CPU clock

PAL runs about 7% slower than NTSC. Games designed for one region will run at the wrong speed on the other.

### PPU-to-CPU ratio

This is the most technically interesting difference. On NTSC, the PPU runs exactly 3 dots per CPU cycle — a clean integer ratio. On PAL, it is 3.2 dots per CPU cycle, or **16/5**.

nes-rs handles this with a **Bresenham accumulator**. Each CPU cycle, the fractional PPU counter advances by the numerator (16 for PAL), and a PPU tick fires whenever the counter exceeds the denominator (5 for PAL):

```rust
for _ in 0..cycles {
    self.ppu_frac += ppu_num;   // +3 (NTSC) or +16 (PAL)
    while self.ppu_frac >= ppu_den {
        self.ppu_frac -= ppu_den; // -1 (NTSC) or -5 (PAL)
        self.ppu.tick(mapper, &mut self.fb);
    }
}
```

This distributes the fractional ticks evenly across CPU cycles without floating-point math.

### VBlank duration

PAL has 70 VBlank scanlines versus NTSC's 20. This gives PAL games significantly more CPU time during VBlank to update VRAM, transfer sprite data, and perform other PPU-sensitive operations.

### Odd frame skip

On NTSC, the pre-render scanline is shortened by one dot on odd frames (339 cycles instead of 340). This keeps the PPU's pixel clock aligned with the CPU over time. PAL does not do this because its non-integer PPU ratio already handles alignment differently.

### APU sequencer

The frame sequencer step period is longer on PAL (8,313 cycles vs 7,457), which means envelopes decay more slowly and length counters run slower. Some music will sound slightly different between regions as a result.

## Region detection

nes-rs detects the region from the iNES header (byte 9, bit 0):
- Bit 0 = 0 → NTSC
- Bit 0 = 1 → PAL

This can be overridden at the command line (`--region pal`) or at runtime through the configuration panel. When the region changes, nes-rs reconfigures:
- PPU pre-render scanline and odd-frame skip behavior
- APU sequencer step period
- Target FPS (60 → 50 or vice versa)
- PPU Bresenham accumulator ratio

## Practical impact

Most NES ROMs are NTSC. PAL-specific ROMs are less common and were often hastily ported — many exhibit slightly off timing, slower music, or squished graphics due to the different aspect ratio. nes-rs defaults to NTSC timing when no ROM is loaded.
