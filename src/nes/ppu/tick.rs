//! Output signals from a single PPU tick.

/// Signal produced by one PPU cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TickOutput {
    /// Nothing noteworthy happened this cycle.
    Idle,
    /// The PPU entered `VBlank` with NMI enabled — CPU should fire NMI.
    Nmi,
    /// A complete frame has been rendered and the framebuffer is ready.
    FrameReady,
}
