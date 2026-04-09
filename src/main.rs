//! NES emulator written in Rust.

mod frontend;
mod nes;

use std::path::PathBuf;

use clap::Parser;
use nes::{Emulator, Nes};

/// NES emulator written in Rust.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Path to an iNES ROM file (.nes).
    ///
    /// If omitted, starts with no ROM loaded.
    /// Press F3 to open the file browser.
    rom: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut emu = Nes::new();

    if let Some(path) = args.rom {
        let data = std::fs::read(&path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
        emu.load_rom(&data)?;
    }

    frontend::run(&mut emu)
}
