//! Audio Processing Unit (2A03).
//!
//! The APU generates audio through five channels:
//! two pulse-wave, one triangle-wave, one noise, and one
//! delta modulation channel (DMC). It is memory-mapped to
//! CPU addresses $4000–$4017.

#![allow(dead_code)]

mod channels;
mod mixer;
mod regs;
mod sequencer;
mod tick;
mod units;

pub(crate) use tick::TickOutput;

use super::region::Region;
use channels::dmc::Dmc;
use channels::noise::Noise;
use channels::pulse::Pulse;
use channels::triangle::Triangle;
use mixer::Mixer;
use sequencer::FrameSequencer;

/// APU state — owns all 5 channels, frame sequencer, and mixer.
///
/// The APU mixes every CPU cycle and stores the latest output.
/// Down-sampling to 44 100 Hz is handled by the caller (Nes) using
/// a Bresenham counter, matching the architecture of other
/// emulators like nez.
#[derive(Clone)]
pub(crate) struct Apu {
    /// Pulse channel 1.
    pulse1: Pulse,
    /// Pulse channel 2.
    pulse2: Pulse,
    /// Triangle channel.
    triangle: Triangle,
    /// Noise channel.
    noise: Noise,
    /// Delta modulation channel.
    dmc: Dmc,
    /// Frame sequencer (clocks envelopes, lengths, sweeps).
    sequencer: FrameSequencer,
    /// Mixer with precomputed lookup tables.
    mixer: Mixer,
    /// Latest mixed output (0.0–1.0), updated every CPU cycle.
    pub(crate) out_sample: f32,
    /// Whether the APU is on an even CPU cycle (pulse timers tick every 2).
    even_cycle: bool,
}

impl Apu {
    /// Creates an APU in its power-on state.
    pub(crate) fn new() -> Self {
        Self {
            pulse1: Pulse::new(true),  // ones' complement
            pulse2: Pulse::new(false), // twos' complement
            triangle: Triangle::new(),
            noise: Noise::new(),
            dmc: Dmc::new(),
            sequencer: FrameSequencer::new(),
            mixer: Mixer::new(),
            out_sample: 0.0,
            even_cycle: false,
        }
    }

    /// Reconfigures the APU for a different TV region.
    pub(crate) fn set_region(&mut self, region: Region) {
        self.sequencer
            .set_step_cycles(region.sequencer_step_cycles());
    }

    /// Advances the APU by one CPU cycle.
    ///
    /// Returns [`TickOutput::Irq`] if the frame counter or DMC
    /// triggered an interrupt.
    pub(crate) fn tick(&mut self) -> TickOutput {
        // Frame sequencer runs every CPU cycle.
        let clocks = self.sequencer.tick();
        if clocks.quarter {
            self.clock_quarter_frame();
        }
        if clocks.half {
            self.clock_half_frame();
        }

        // Triangle ticks every CPU cycle; pulse/noise every 2.
        self.triangle.tick();
        if self.even_cycle {
            self.pulse1.tick();
            self.pulse2.tick();
            self.noise.tick();
        }
        self.dmc.tick();
        self.even_cycle = !self.even_cycle;

        // Mix every cycle — the caller reads `out_sample` when the
        // Bresenham down-sampler fires.
        self.out_sample = self.mixer.mix(
            self.pulse1.output(),
            self.pulse2.output(),
            self.triangle.output(),
            self.noise.output(),
            self.dmc.output(),
        );

        // Signal IRQ if frame counter or DMC flagged one.
        if clocks.irq || self.sequencer.irq_pending || self.dmc.irq_pending {
            TickOutput::Irq
        } else {
            TickOutput::Idle
        }
    }

    /// Returns the DMC sample read address, if a byte is needed.
    pub(crate) fn dmc_sample_addr(&self) -> Option<u16> {
        self.dmc.sample_addr()
    }

    /// Fills the DMC sample buffer with a byte read from memory.
    pub(crate) fn dmc_fill_sample(&mut self, byte: u8) {
        self.dmc.fill_sample(byte);
    }

    /// Clocks envelope generators and the triangle linear counter.
    fn clock_quarter_frame(&mut self) {
        self.pulse1.envelope.clock();
        self.pulse2.envelope.clock();
        self.triangle.linear.clock();
        self.noise.envelope.clock();
    }

    /// Clocks length counters and sweep units.
    fn clock_half_frame(&mut self) {
        self.pulse1.length.clock();
        self.pulse2.length.clock();
        self.triangle.length.clock();
        self.noise.length.clock();
        self.pulse1.sweep.clock(&mut self.pulse1.timer_period);
        self.pulse2.sweep.clock(&mut self.pulse2.timer_period);
    }
}
