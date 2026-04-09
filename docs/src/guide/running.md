# Running the Emulator

## Basic usage

```sh
nes-rs [OPTIONS] [ROM]
```

The only argument is an optional path to an iNES ROM file (`.nes`). If omitted, the emulator starts with no ROM loaded — press **F3** to open a file browser.

```sh
# Load a ROM directly
nes-rs path/to/game.nes

# Start empty, pick a ROM later
nes-rs
```

## Command-line options

| Option | Description |
|--------|-------------|
| `--region <REGION>` | Force a TV region instead of auto-detecting. Values: `ntsc`, `pal`. |
| `-h`, `--help` | Print help and exit. |
| `-V`, `--version` | Print version and exit. |

### Region override

By default, the emulator reads the TV system from the iNES header (byte 9, bit 0). You can override this:

```sh
# Force PAL timing on a ROM with an incorrect header
nes-rs --region pal game.nes
```

The region affects CPU clock speed, PPU scanline count, APU sequencer timing, and target frame rate. See the [NTSC vs PAL](../nes/regions.md) chapter for details.

## Loading ROMs at runtime

There are two ways to load a ROM after the emulator has started:

1. **File browser** — Press **F3** to open a native OS file dialog.
2. **Drag and drop** — Drag a `.nes` file onto the emulator window.

Both methods reset the emulator and start the new game immediately.

## ROM format

nes-rs supports the **iNES** format (`.nes` files). This is the most common format for NES ROM images. The emulator reads the 16-byte iNES header to determine:

- PRG-ROM and CHR-ROM sizes
- Mapper number (bank-switching hardware)
- Nametable mirroring mode
- TV system (NTSC or PAL)

If the ROM uses an unsupported mapper, the emulator logs an error and does not load it.

## Logging

nes-rs uses the `RUST_LOG` environment variable for log filtering:

```sh
# Show debug-level logs
RUST_LOG=debug nes-rs game.nes

# Show only warnings and errors
RUST_LOG=warn nes-rs game.nes

# Default (info level)
nes-rs game.nes
```
