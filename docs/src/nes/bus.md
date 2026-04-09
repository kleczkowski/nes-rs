# Memory Map & Bus

The NES CPU addresses a 64 KB memory space. There is no memory management unit — all address decoding is done by discrete logic on the motherboard and cartridge. The CPU bus routes each address to the appropriate device.

## CPU memory map

| Address range | Size | Device | Access |
|---------------|------|--------|--------|
| `$0000`–`$07FF` | 2 KB | Internal RAM | Read/Write |
| `$0800`–`$1FFF` | — | RAM mirrors (repeats every 2 KB) | Read/Write |
| `$2000`–`$2007` | 8 bytes | PPU registers | Mixed |
| `$2008`–`$3FFF` | — | PPU register mirrors (repeats every 8 bytes) | Mixed |
| `$4000`–`$4013` | 20 bytes | APU registers | Write |
| `$4014` | 1 byte | OAM DMA | Write |
| `$4015` | 1 byte | APU status | Read/Write |
| `$4016` | 1 byte | Controller 1 | Read/Write |
| `$4017` | 1 byte | Controller 2 / APU frame counter | Read/Write |
| `$4018`–`$401F` | 8 bytes | Normally unused | — |
| `$4020`–`$FFFF` | ~48 KB | Cartridge space (mapper-controlled) | Read/Write |

### Mirroring

The NES uses address mirroring extensively to save decode logic:

- **RAM**: The 2 KB at `$0000`–`$07FF` is mirrored three times across `$0800`–`$1FFF`. A read from `$0800` returns the same data as `$0000`. The bus achieves this by masking the address with `& 0x07FF`.
- **PPU registers**: The 8 registers at `$2000`–`$2007` are mirrored across `$2008`–`$3FFF`. The bus masks with `& 0x0007` to get the register offset.

### Zero page and stack

The first 256 bytes (`$0000`–`$00FF`) are called **zero page**. The 6502 has dedicated addressing modes for zero page that are faster and use fewer bytes than their absolute counterparts. Games store frequently-accessed variables here.

The stack occupies page 1 (`$0100`–`$01FF`). The 8-bit stack pointer is an offset within this page.

## Bus implementation

In nes-rs, the `Bus` struct owns the RAM, mapper, and controller state. The APU is temporarily "parked" in the bus during each CPU step so that register reads/writes at `$4000`–`$4017` can be routed:

```rust
fn cpu_step(&mut self) -> cpu::StepResult {
    self.bus.apu = self.apu.take();
    let result = self.cpu.step(&mut self.bus, &mut self.ppu);
    self.apu = self.bus.apu.take();
    result
}
```

This avoids Rust's borrow checker conflicts — the CPU step needs mutable access to the bus, and the bus needs to route APU register accesses, but the `Nes` struct owns both.

### Read path

```rust
match addr {
    0x0000..=0x1FFF => self.ram[addr & 0x07FF],       // RAM + mirrors
    0x2000..=0x3FFF => ppu.cpu_read(addr & 0x07, ..),  // PPU regs + mirrors
    0x4015           => apu.read_status(),              // APU status
    0x4016           => controller1.read(),              // Joypad 1
    0x4017           => controller2.read(),              // Joypad 2
    0x4000..=0x4013  => 0,                              // APU write-only
    0x4020..=0xFFFF  => mapper.cpu_read(addr),          // Cartridge
    _                => 0,                              // Open bus
}
```

### Write path

```rust
match addr {
    0x0000..=0x1FFF => self.ram[addr & 0x07FF] = val,
    0x2000..=0x3FFF => ppu.cpu_write(addr & 0x07, val, ..),
    0x4014           => self.oam_dma(val, ppu),
    0x4016           => { controller1.write(val); controller2.write(val); }
    0x4000..=0x4013
    | 0x4015
    | 0x4017         => apu.write_register(addr, val),
    0x4020..=0xFFFF  => mapper.cpu_write(addr, val),
    _                => {},
}
```

## OAM DMA

Writing a byte to `$4014` triggers a **DMA transfer**: 256 bytes are copied from CPU page `(value << 8)` to PPU OAM. This takes 513–514 CPU cycles on real hardware (nes-rs performs it instantly during the write, which is a simplification that works in practice).

For example, writing `$02` to `$4014` copies `$0200`–`$02FF` to OAM. Games typically reserve a page of RAM for a "shadow OAM" buffer and DMA it to the PPU during VBlank.

## PPU address space

The PPU has a separate 16 KB address space (detailed in the [PPU chapter](ppu.md)):

| Range | Device |
|-------|--------|
| `$0000`–`$1FFF` | Pattern tables (via mapper CHR-ROM/RAM) |
| `$2000`–`$2FFF` | Nametables (internal VRAM, mirrored) |
| `$3F00`–`$3FFF` | Palette RAM |

The CPU does not directly access PPU memory — all PPU reads/writes go through the PPUADDR (`$2006`) and PPUDATA (`$2007`) registers.

## Interrupt vectors

The top 6 bytes of the CPU address space hold the interrupt vectors, which are typically located in the cartridge's PRG-ROM:

| Address | Vector |
|---------|--------|
| `$FFFA`–`$FFFB` | NMI handler |
| `$FFFC`–`$FFFD` | Reset handler |
| `$FFFE`–`$FFFF` | IRQ/BRK handler |
