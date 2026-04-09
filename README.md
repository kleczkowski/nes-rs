# nes-rs

A cycle-aware NES (Nintendo Entertainment System) emulator written in Rust.

## Features

- **CPU** — MOS 6502 (2A03 variant) with full instruction set, cycle counting, and NMI/IRQ interrupts
- **PPU** — Scanline-based rendering at 256×240, sprite evaluation, background scrolling, palette RAM
- **APU** — All five channels (2× pulse, triangle, noise, DMC) with frame sequencer, DC-blocking filters, and Bresenham down-sampling to 44.1 kHz
- **10 mappers** — NROM (0), MMC1 (1), UxROM (2), CNROM (3), MMC3 (4), AxROM (7), MMC2 (9), Color Dreams (11), GxROM (66), Camerica (71)
- **NTSC & PAL** — Auto-detected from the iNES header or forced via CLI flag
- **Scaling modes** — Centered, aspect-fit, and stretch
- **Gamepad & keyboard** — Configurable bindings, two-player support
- **Drag-and-drop** — Drop a `.nes` file onto the window to load it
- **In-app config panel** — Press F1 to adjust volume, FPS, V-Sync, sprite limit, scale mode, region, and controls

## Requirements

- Rust 2024 edition (1.85+)
- System libraries for raylib:
  - **Linux:** `libgl-dev libx11-dev libxcursor-dev libxrandr-dev libxinerama-dev libxi-dev libasound2-dev`
  - **macOS / Windows:** No extra system packages needed

## Building

```sh
make build          # debug build
make release        # optimised release build
make test           # run tests
make check          # fmt check + clippy + tests
```

Cross-compile by setting `TARGET`:

```sh
make release TARGET=x86_64-pc-windows-gnu
make package TARGET=x86_64-pc-windows-gnu
```

## Usage

```
nes-rs [OPTIONS] [ROM]

Arguments:
  [ROM]  Path to an iNES ROM file (.nes)

Options:
  --region <REGION>  Force TV region (ntsc, pal)
  -h, --help         Print help
  -V, --version      Print version
```

If no ROM is provided, the emulator starts empty — press **F3** to open the file browser.

### Hotkeys

| Key | Action |
|-----|--------|
| F1 | Toggle config panel |
| F3 | Open file browser |

### Default controls

| NES button | Keyboard |
|------------|----------|
| A | Z |
| B | X |
| Select | Backspace |
| Start | Enter |
| D-Pad | Arrow keys |

## Architecture

```
src/
├── main.rs              CLI entry point
├── nes.rs               Emulator trait and orchestrator
├── nes/
│   ├── cpu/             6502 core, addressing modes, opcode table
│   ├── ppu/             Scanline renderer, registers, palette
│   ├── apu/             Channels, mixer, frame sequencer
│   ├── mapper/          Cartridge mapper implementations
│   ├── bus.rs           CPU address space decoding
│   ├── cartridge.rs     iNES parser (nom)
│   ├── controller.rs    Joypad shift-register emulation
│   └── region.rs        NTSC / PAL timing constants
└── frontend/
    ├── video.rs         Scaling and framebuffer rendering
    ├── audio.rs         Lock-free ring buffer
    ├── input.rs         Keyboard and gamepad mapping
    ├── config.rs        Settings panel (F1)
    └── filebrowser.rs   Native file dialog (rfd)
```

## License

[MIT](LICENSE)
