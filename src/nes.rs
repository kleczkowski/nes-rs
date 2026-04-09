//! NES emulator core — module root and orchestrator.
//!
//! This module re-exports the shared types used by the frontend
//! and declares the subsystem modules (CPU, PPU, APU, bus, cartridge).

mod apu;
mod bus;
mod cartridge;
mod controller;
mod cpu;
mod framebuffer;
mod mapper;
mod ppu;
pub(crate) mod region;
mod stub;

pub(crate) use controller::Buttons;
pub(crate) use framebuffer::{Framebuffer, HEIGHT as SCREEN_HEIGHT, WIDTH as SCREEN_WIDTH};
pub(crate) use region::Region;

use apu::Apu;
use bus::Bus;
use cartridge::Cartridge;
use cpu::Cpu;
use ppu::{Ppu, TickOutput};

use crate::frontend::audio;

/// Audio output sample rate in Hz.
pub(crate) const SAMPLE_RATE: u32 = 44_100;

/// Trait that any NES emulator implementation must satisfy
/// for the frontend to drive it.
pub(crate) trait Emulator {
    /// Run the emulator for `dt_ms` wall-clock milliseconds.
    fn update(&mut self, dt_ms: f64);

    /// Returns a reference to the current framebuffer.
    fn framebuffer(&self) -> &Framebuffer;

    /// Updates the controller button state for the given player (0 or 1).
    fn set_buttons(&mut self, player: u8, buttons: Buttons);

    /// Enables or disables the 8-sprite-per-scanline limit.
    fn set_sprite_limit(&mut self, enabled: bool);

    /// Returns the active TV region.
    fn region(&self) -> Region;

    /// Loads an iNES ROM, resetting the emulator.
    ///
    /// # Errors
    ///
    /// Returns an error if the ROM is invalid or the mapper is
    /// unsupported.
    fn load_rom(&mut self, data: &[u8]) -> anyhow::Result<()>;
}

/// Real NES emulator — owns CPU, PPU, APU, Bus, and Framebuffer.
pub(crate) struct Nes {
    cpu: Cpu,
    ppu: Ppu,
    bus: Bus,
    apu: Option<Apu>,
    /// Back-buffer: PPU renders here.
    fb: Framebuffer,
    /// Front-buffer: the last complete frame, safe to display.
    fb_front: Framebuffer,
    /// Set when the PPU finishes a frame; cleared after swap.
    frame_ready: bool,
    /// Bresenham accumulator for down-sampling APU output to 44 100 Hz.
    sample_clock: u32,
    /// Two cascaded first-order DC-blocking high-pass filters,
    /// matching the real NES's two AC-coupling stages.
    hp1_in: f32,
    hp1_out: f32,
    hp2_in: f32,
    hp2_out: f32,
    /// Active TV region (determines all timing).
    region: Region,
    /// CLI override — when set, ignores the cartridge header.
    region_override: Option<Region>,
    /// Fractional PPU dot accumulator for non-integer PPU/CPU ratios.
    ppu_frac: u16,
}

/// High-pass coefficient for each stage (~35 Hz at 44 100 Hz).
/// Two cascaded stages give 12 dB/oct rolloff and settle DC
/// shifts ~2× faster than one stage, without distorting the
/// triangle's ramp.
const HP_ALPHA: f32 = 0.995;

impl Nes {
    /// Creates a new NES with no cartridge loaded.
    ///
    /// If `region_override` is `Some`, it takes precedence over
    /// whatever the iNES header says when a ROM is loaded.
    pub(crate) fn new(region_override: Option<Region>) -> Self {
        Self {
            cpu: Cpu::new(),
            ppu: Ppu::new(),
            bus: Bus::new(),
            apu: Some(Apu::new()),
            fb: Framebuffer::new(),
            fb_front: Framebuffer::new(),
            frame_ready: false,
            sample_clock: 0,
            hp1_in: 0.0,
            hp1_out: 0.0,
            hp2_in: 0.0,
            hp2_out: 0.0,
            region: Region::default(),
            region_override,
            ppu_frac: 0,
        }
    }

    /// Applies a region change to all timing-sensitive subsystems.
    fn apply_region(&mut self, region: Region) {
        self.region = region;
        self.ppu.set_region(region);
        if let Some(apu) = &mut self.apu {
            apu.set_region(region);
        }
        self.ppu_frac = 0;
        tracing::info!(%region, "timing configured");
    }

    /// Parks the APU in the Bus for register routing, runs
    /// one CPU instruction, then takes it back.
    fn cpu_step(&mut self) -> cpu::StepResult {
        self.bus.apu = self.apu.take();
        let result = self.cpu.step(&mut self.bus, &mut self.ppu);
        self.apu = self.bus.apu.take();
        result
    }
}

impl Emulator for Nes {
    fn update(&mut self, dt_ms: f64) {
        let cpu_clock_hz = self.region.cpu_clock_hz();
        let cpu_cycles_per_ms = f64::from(cpu_clock_hz) / 1000.0;
        let (ppu_num, ppu_den) = self.region.ppu_ratio();

        let target_cycles = (dt_ms * cpu_cycles_per_ms) as u64;
        let mut cycles_run: u64 = 0;
        let mut sample_batch = Vec::with_capacity(128);

        while cycles_run < target_cycles {
            let step = self.cpu_step();
            let cycles = match step {
                Ok(ok) => ok.cycles(),
                Err(e) => {
                    tracing::warn!(error = ?e, pc = format_args!("${:04X}", self.cpu.pc), "CPU halted");
                    break;
                }
            };

            cycles_run += u64::from(cycles);

            // APU runs 1:1 with CPU.
            if let Some(apu) = &mut self.apu {
                for _ in 0..cycles {
                    if apu.tick() == apu::TickOutput::Irq {
                        self.cpu.request_irq();
                    }

                    // Service DMC sample fetches through the bus.
                    let dmc_addr = apu.dmc_sample_addr();
                    if let Some(addr) = dmc_addr {
                        let byte = self.bus.peek(addr);
                        apu.dmc_fill_sample(byte);
                    }

                    // Bresenham down-sampler: point-sample at SAMPLE_RATE.
                    self.sample_clock += SAMPLE_RATE;
                    if self.sample_clock >= cpu_clock_hz {
                        self.sample_clock -= cpu_clock_hz;

                        let s = apu.out_sample;

                        // Two cascaded DC-blocking high-pass stages.
                        self.hp1_out = HP_ALPHA * (self.hp1_out + s - self.hp1_in);
                        self.hp1_in = s;

                        self.hp2_out = HP_ALPHA * (self.hp2_out + self.hp1_out - self.hp2_in);
                        self.hp2_in = self.hp1_out;

                        let pcm = (self.hp2_out * f32::from(i16::MAX))
                            .clamp(f32::from(i16::MIN), f32::from(i16::MAX))
                            as i16;
                        sample_batch.push(pcm);
                    }
                }
            }

            // PPU runs at ppu_num/ppu_den dots per CPU cycle.
            // Bresenham accumulator distributes fractional ticks evenly.
            for _ in 0..cycles {
                self.ppu_frac += ppu_num;
                while self.ppu_frac >= ppu_den {
                    self.ppu_frac -= ppu_den;
                    let Some(mapper) = self.bus.mapper_mut() else {
                        continue;
                    };
                    match self.ppu.tick(mapper, &mut self.fb) {
                        TickOutput::Nmi => self.cpu.request_nmi(),
                        TickOutput::FrameReady => {
                            std::mem::swap(&mut self.fb, &mut self.fb_front);
                            self.frame_ready = true;
                        }
                        TickOutput::Idle => {}
                    }
                }
            }
        }

        // Flush any remaining samples.
        if !sample_batch.is_empty() {
            audio::queue_samples(&sample_batch);
        }
    }

    fn framebuffer(&self) -> &Framebuffer {
        &self.fb_front
    }

    fn set_buttons(&mut self, player: u8, buttons: Buttons) {
        self.bus.set_controller_buttons(player, buttons);
    }

    fn set_sprite_limit(&mut self, enabled: bool) {
        self.ppu.sprite_limit = enabled;
    }

    fn region(&self) -> Region {
        self.region
    }

    fn load_rom(&mut self, data: &[u8]) -> anyhow::Result<()> {
        tracing::info!(size = data.len(), "loading ROM");
        let cart = Cartridge::from_ines(data)?;
        let detected = cart.region();
        let region = self.region_override.unwrap_or(detected);
        if self.region_override.is_some() && detected != region {
            tracing::info!(
                detected = %detected,
                override_to = %region,
                "region override active",
            );
        }
        self.bus.load_cartridge(cart)?;
        self.apply_region(region);
        self.bus.apu = self.apu.take();
        self.cpu.reset(&mut self.bus, &mut self.ppu);
        self.apu = self.bus.apu.take();
        tracing::info!(
            reset_vector = format_args!("${:04X}", self.cpu.pc),
            "emulator reset complete",
        );
        Ok(())
    }
}
