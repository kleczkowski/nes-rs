//! NES emulator written in Rust.

mod frontend;
mod nes;

use std::path::PathBuf;

use clap::Parser;
use nes::{Emulator, Nes};
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
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    let mut emu = Nes::new();

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
