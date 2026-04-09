# Installation & Building

## Prerequisites

- **Rust** — Edition 2024 (Rust 1.85 or later). Install via [rustup](https://rustup.rs/).
- **System libraries** — Required by raylib for graphics and audio:

| Platform | Packages |
|----------|----------|
| Debian / Ubuntu | `libgl-dev libx11-dev libxcursor-dev libxrandr-dev libxinerama-dev libxi-dev libasound2-dev` |
| Fedora | `mesa-libGL-devel libX11-devel libXcursor-devel libXrandr-devel libXinerama-devel libXi-devel alsa-lib-devel` |
| macOS | None (Xcode command-line tools provide everything) |
| Windows | None (MSVC toolchain includes everything) |

On Debian/Ubuntu, install all dependencies at once:

```sh
sudo apt install libgl-dev libx11-dev libxcursor-dev \
  libxrandr-dev libxinerama-dev libxi-dev libasound2-dev
```

## Building

The project includes a Makefile with standard targets:

```sh
# Debug build (fast compilation, slow execution)
make build

# Optimized release build
make release

# Run the test suite
make test

# Full CI check: format + clippy + tests
make check
```

The release binary is placed at `target/release/nes-rs`.

## Cross-compilation

You can cross-compile for other platforms by setting the `TARGET` variable. This requires the appropriate Rust target and linker toolchain to be installed.

```sh
# Build for Windows from Linux
make release TARGET=x86_64-pc-windows-gnu

# Create a distributable archive
make package TARGET=x86_64-pc-windows-gnu
```

The `package` target strips the binary and creates a `.tar.gz` (Linux/macOS) or `.zip` (Windows) archive in the `dist/` directory.

## Verifying the build

After building, run the test suite to confirm everything is working:

```sh
make test
```

This runs unit tests for the CPU instruction set, memory bus, PPU address decoding, and iNES cartridge parsing.
