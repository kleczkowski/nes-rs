//! Output signals from a single APU tick.

/// Signal produced by one APU cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TickOutput {
    /// Nothing noteworthy happened this cycle.
    Idle,
    /// Frame counter IRQ fired — CPU should handle if IRQ is enabled.
    Irq,
}
