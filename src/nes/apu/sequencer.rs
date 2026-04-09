//! Frame sequencer — clocks envelope, length counter, and sweep units.
//!
//! The frame sequencer runs at ~240 Hz (every ~7457 CPU cycles) in
//! two modes:
//! - **4-step** (mode 0): quarter/half frame clocks + optional IRQ
//! - **5-step** (mode 1): quarter/half frame clocks, no IRQ

/// CPU cycles per frame sequencer step (NTSC, approximate).
const STEP_CYCLES: u16 = 7457;

/// Frame sequencer mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::nes) enum Mode {
    /// 4-step sequence with optional IRQ.
    FourStep,
    /// 5-step sequence, no IRQ.
    FiveStep,
}

/// Signals produced by one frame sequencer tick.
#[derive(Debug, Default, Clone, Copy)]
pub(in crate::nes) struct FrameClocks {
    /// Clock envelope generators and triangle linear counter.
    pub(in crate::nes) quarter: bool,
    /// Clock length counters and sweep units.
    pub(in crate::nes) half: bool,
    /// Fire frame counter IRQ (mode 0 only).
    pub(in crate::nes) irq: bool,
}

/// Frame sequencer state.
#[derive(Debug, Clone)]
pub(in crate::nes) struct FrameSequencer {
    /// Current mode (4-step or 5-step).
    mode: Mode,
    /// Current step in the sequence (0–3 or 0–4).
    step: u8,
    /// CPU cycle counter within the current step.
    cycle: u16,
    /// Whether frame IRQ is inhibited ($4017 bit 6).
    pub(in crate::nes) irq_inhibit: bool,
    /// Pending IRQ flag.
    pub(in crate::nes) irq_pending: bool,
}

impl FrameSequencer {
    /// Creates a frame sequencer in its initial state.
    pub(in crate::nes) fn new() -> Self {
        Self {
            mode: Mode::FourStep,
            step: 0,
            cycle: 0,
            irq_inhibit: false,
            irq_pending: false,
        }
    }

    /// Advances by one CPU cycle and returns which clocks to fire.
    pub(in crate::nes) fn tick(&mut self) -> FrameClocks {
        self.cycle += 1;
        if self.cycle < STEP_CYCLES {
            return FrameClocks::default();
        }
        self.cycle = 0;
        let clocks = self.step_clocks();
        self.advance_step();
        clocks
    }

    /// Returns the clocks for the current step.
    fn step_clocks(&mut self) -> FrameClocks {
        match self.mode {
            Mode::FourStep => match self.step {
                0 | 2 => FrameClocks {
                    quarter: true,
                    ..Default::default()
                },
                1 => FrameClocks {
                    quarter: true,
                    half: true,
                    ..Default::default()
                },
                3 => {
                    if !self.irq_inhibit {
                        self.irq_pending = true;
                    }
                    FrameClocks {
                        quarter: true,
                        half: true,
                        irq: !self.irq_inhibit,
                    }
                }
                _ => FrameClocks::default(),
            },
            Mode::FiveStep => match self.step {
                0 | 2 => FrameClocks {
                    quarter: true,
                    ..Default::default()
                },
                1 | 3 => FrameClocks {
                    quarter: true,
                    half: true,
                    ..Default::default()
                },
                _ => FrameClocks::default(), // step 4 is idle
            },
        }
    }

    /// Advances to the next step, wrapping based on mode.
    fn advance_step(&mut self) {
        let max = match self.mode {
            Mode::FourStep => 3,
            Mode::FiveStep => 4,
        };
        self.step = if self.step >= max { 0 } else { self.step + 1 };
    }

    /// Handles a write to $4017 (frame counter control).
    pub(in crate::nes) fn write_control(&mut self, val: u8) {
        self.mode = if val & 0x80 != 0 {
            Mode::FiveStep
        } else {
            Mode::FourStep
        };
        self.irq_inhibit = val & 0x40 != 0;
        if self.irq_inhibit {
            self.irq_pending = false;
        }
        self.step = 0;
        self.cycle = 0;
    }
}
