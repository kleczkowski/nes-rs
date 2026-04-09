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
mod stub;

pub(crate) use controller::Buttons;
pub(crate) use framebuffer::{Framebuffer, HEIGHT as SCREEN_HEIGHT, WIDTH as SCREEN_WIDTH};

use apu::Apu;
use bus::Bus;
use cartridge::Cartridge;
use cpu::Cpu;
use ppu::{Ppu, TickOutput};

use crate::frontend::audio;

/// NTSC CPU clock rate in Hz.
pub(crate) const CPU_CLOCK_HZ: u32 = 1_789_773;

/// Audio output sample rate in Hz.
pub(crate) const SAMPLE_RATE: u32 = 44_100;

/// CPU cycles per millisecond (NTSC).
const CPU_CYCLES_PER_MS: f64 = CPU_CLOCK_HZ as f64 / 1000.0;

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
}

/// High-pass coefficient for each stage (~35 Hz at 44 100 Hz).
/// Two cascaded stages give 12 dB/oct rolloff and settle DC
/// shifts ~2× faster than one stage, without distorting the
/// triangle's ramp.
const HP_ALPHA: f32 = 0.995;

impl Nes {
    /// Creates a new NES with no cartridge loaded.
    pub(crate) fn new() -> Self {
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
        }
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
        let target_cycles = (dt_ms * CPU_CYCLES_PER_MS) as u64;
        let mut cycles_run: u64 = 0;
        let mut sample_batch = Vec::with_capacity(128);

        while cycles_run < target_cycles {
            let step = self.cpu_step();
            let cycles = match step {
                Ok(ok) => ok.cycles(),
                Err(_) => break,
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
                    if self.sample_clock >= CPU_CLOCK_HZ {
                        self.sample_clock -= CPU_CLOCK_HZ;

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

            // PPU runs 3 dots per CPU cycle.
            for _ in 0..u16::from(cycles) * 3 {
                let Some(mapper) = self.bus.mapper_mut() else {
                    continue;
                };
                match self.ppu.tick(mapper, &mut self.fb) {
                    TickOutput::Nmi => self.cpu.request_nmi(),
                    TickOutput::FrameReady => {
                        // Swap back→front so the display always
                        // sees a fully rendered frame.
                        std::mem::swap(&mut self.fb, &mut self.fb_front);
                        self.frame_ready = true;
                    }
                    TickOutput::Idle => {}
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

    fn load_rom(&mut self, data: &[u8]) -> anyhow::Result<()> {
        let cart = Cartridge::from_ines(data)?;
        self.bus.load_cartridge(cart)?;
        self.bus.apu = self.apu.take();
        self.cpu.reset(&mut self.bus, &mut self.ppu);
        self.apu = self.bus.apu.take();
        Ok(())
    }
}
