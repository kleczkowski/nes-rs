//! TV system region (NTSC / PAL) and associated timing constants.
//!
//! The NES was released in two main variants with different master
//! clocks, which cascade into every timing-sensitive subsystem:
//! CPU frequency, PPU-to-CPU ratio, frame length, and APU sequencer
//! period.

use std::fmt;

/// TV system / region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub(crate) enum Region {
    /// NTSC (Americas, Japan) — 60 Hz, 262 scanlines.
    #[default]
    Ntsc,
    /// PAL (Europe, Australia) — 50 Hz, 312 scanlines.
    Pal,
}

impl Region {
    /// CPU clock frequency in Hz.
    pub(crate) fn cpu_clock_hz(self) -> u32 {
        match self {
            Self::Ntsc => 1_789_773,
            Self::Pal => 1_662_607,
        }
    }

    /// PPU-to-CPU clock ratio as (numerator, denominator).
    ///
    /// NTSC: 3 dots per CPU cycle (3/1).
    /// PAL:  3.2 dots per CPU cycle (16/5).
    pub(crate) fn ppu_ratio(self) -> (u16, u16) {
        match self {
            Self::Ntsc => (3, 1),
            Self::Pal => (16, 5),
        }
    }

    /// Pre-render scanline number (last scanline of the frame).
    pub(crate) fn pre_render_line(self) -> u16 {
        match self {
            Self::Ntsc => 261,
            Self::Pal => 311,
        }
    }

    /// APU frame sequencer step period in CPU cycles.
    pub(crate) fn sequencer_step_cycles(self) -> u16 {
        match self {
            Self::Ntsc => 7457,
            Self::Pal => 8313,
        }
    }

    /// Whether the PPU skips one dot on odd frames.
    ///
    /// NTSC does this; PAL does not.
    pub(crate) fn has_odd_frame_skip(self) -> bool {
        match self {
            Self::Ntsc => true,
            Self::Pal => false,
        }
    }

    /// Native display refresh rate.
    pub(crate) fn fps(self) -> i32 {
        match self {
            Self::Ntsc => 60,
            Self::Pal => 50,
        }
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ntsc => f.write_str("NTSC"),
            Self::Pal => f.write_str("PAL"),
        }
    }
}
