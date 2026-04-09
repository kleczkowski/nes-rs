//! NES emulator written in Rust.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod frontend;
mod nes;

use std::path::PathBuf;

use clap::Parser;
use nes::{Emulator, Nes, Region};
use tracing_subscriber::EnvFilter;

/// NES emulator written in Rust.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Path to an iNES ROM file (.nes).
    ///
    /// If omitted, starts with no ROM loaded.
    /// Press F3 to open the file browser.
    rom: Option<PathBuf>,

    /// Force a TV region instead of auto-detecting from the ROM header.
    #[arg(long, value_enum)]
    region: Option<Region>,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    let mut emu = Nes::new(args.region);

    if let Some(ref path) = args.rom {
        tracing::info!(path = %path.display(), "loading ROM from CLI argument");
        let data = std::fs::read(path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
        emu.load_rom(&data)?;
    } else {
        tracing::info!("starting with no ROM loaded — press F3 to open file browser");
    }

    frontend::run(&mut emu)
}
