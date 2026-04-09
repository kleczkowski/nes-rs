# Introduction

**nes-rs** is a cycle-aware Nintendo Entertainment System (NES) emulator written in Rust. It emulates the core hardware of the NES — CPU, PPU, and APU — with enough accuracy to run a wide catalog of commercial games from the 1980s and early 1990s.

## What this book covers

This documentation serves two purposes:

1. **User guide** — how to build, install, and use the emulator.
2. **Technical reference** — how the NES hardware works and how nes-rs models each subsystem.

Whether you want to play games or understand the engineering behind a classic console, this book has you covered.

## Features at a glance

- **CPU** — Full MOS 6502 instruction set (2A03 variant, no decimal mode), with cycle counting and NMI/IRQ interrupts.
- **PPU** — Scanline-based renderer at 256x240 pixels, with sprite evaluation, background scrolling, and palette RAM.
- **APU** — All five audio channels (2x pulse, triangle, noise, DMC) with frame sequencer, nonlinear mixer, and DC-blocking filters.
- **10 mappers** — NROM (0), MMC1 (1), UxROM (2), CNROM (3), MMC3 (4), AxROM (7), MMC2 (9), Color Dreams (11), GxROM (66), Camerica (71).
- **NTSC and PAL** — Auto-detected from the iNES header or forced via CLI flag.
- **Raylib frontend** — Hardware-accelerated rendering, lock-free audio, three scaling modes, gamepad support, drag-and-drop ROM loading, and an in-app settings panel.
- **Rewind** — Hold R to rewind up to 10 seconds of gameplay and resume from any point, VHS-style.
- **Quality-of-life** — Pause (P), soft reset (F5), fullscreen (F11), fast forward (hold Tab, 4×), mute (M).

## Source code

The source code is available at [github.com/kleczkowski/nes-rs](https://github.com/kleczkowski/nes-rs) under the MIT license.
