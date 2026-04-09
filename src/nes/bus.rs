//! CPU memory bus and address decoding.
//!
//! The NES CPU addresses a 64 KB space:
//!
//! | Range           | Size  | Device                         |
//! |-----------------|-------|--------------------------------|
//! | $0000 – $07FF   | 2 KB  | Internal RAM                   |
//! | $0800 – $1FFF   | —     | RAM mirrors                    |
//! | $2000 – $2007   | 8     | PPU registers                  |
//! | $2008 – $3FFF   | —     | PPU register mirrors           |
//! | $4000 – $4013   | 20    | APU registers                  |
//! | $4014           | 1     | OAM DMA                        |
//! | $4015           | 1     | APU status                     |
//! | $4016           | 1     | Controller 1                   |
//! | $4017           | 1     | Controller 2 / APU frame ctr   |
//! | $4020 – $FFFF   | ~49 K | Cartridge (PRG-ROM / mapper)   |

#![allow(dead_code)]

use super::apu::Apu;
use super::cartridge::Cartridge;
use super::controller::{Buttons, Controller};
use super::mapper::{self, Mapper};
use super::ppu::Ppu;

/// Size of the NES internal RAM in bytes.
const RAM_SIZE: usize = 2048;

/// CPU memory bus connecting RAM, PPU, controllers, and mapper.
///
/// The APU lives in [`super::Nes`] and is temporarily parked here
/// during each CPU step so that register reads/writes at
/// $4000–$4017 can be routed.
pub(crate) struct Bus {
    /// 2 KB internal work RAM.
    ram: [u8; RAM_SIZE],
    /// Active mapper (created from a loaded cartridge).
    mapper: Option<Box<dyn Mapper>>,
    /// Temporarily holds the APU during a CPU step for register routing.
    pub(super) apu: Option<Apu>,
    /// Player 1 controller.
    controller1: Controller,
    /// Player 2 controller.
    controller2: Controller,
}

impl Clone for Bus {
    fn clone(&self) -> Self {
        Self {
            ram: self.ram,
            mapper: self.mapper.as_ref().map(|m| m.box_clone()),
            apu: self.apu.clone(),
            controller1: self.controller1,
            controller2: self.controller2,
        }
    }
}

impl Bus {
    /// Creates a bus with zeroed RAM, no cartridge, and default peripherals.
    pub(crate) fn new() -> Self {
        Self {
            ram: [0; RAM_SIZE],
            mapper: None,
            apu: None,
            controller1: Controller::default(),
            controller2: Controller::default(),
        }
    }

    /// Loads a cartridge by creating the appropriate mapper.
    ///
    /// # Errors
    ///
    /// Returns an error if the cartridge's mapper ID is not supported.
    pub(crate) fn load_cartridge(&mut self, cart: Cartridge) -> anyhow::Result<()> {
        self.mapper = Some(mapper::from_cartridge(cart)?);
        Ok(())
    }

    /// Updates the button state for a controller (0 = player 1, 1 = player 2).
    pub(crate) fn set_controller_buttons(&mut self, player: u8, buttons: Buttons) {
        match player {
            0 => self.controller1.set_buttons(buttons),
            _ => self.controller2.set_buttons(buttons),
        }
    }

    /// Returns a mutable reference to the active mapper, if any.
    pub(super) fn mapper_mut(&mut self) -> Option<&mut (dyn Mapper + 'static)> {
        self.mapper.as_deref_mut()
    }

    /// Returns `true` if a cartridge is loaded.
    pub(super) fn has_mapper(&self) -> bool {
        self.mapper.is_some()
    }

    /// Reads a byte without side effects (used for DMC sample fetches).
    pub(super) fn peek(&self, addr: u16) -> u8 {
        self.read_raw(addr)
    }

    /// Reads a byte from the given CPU address.
    #[allow(clippy::match_same_arms)]
    pub(super) fn read(&mut self, addr: u16, ppu: &mut Ppu) -> u8 {
        match addr {
            0x0000..=0x1FFF => {
                let index = usize::from(addr) & 0x07FF;
                self.ram.get(index).copied().unwrap_or(0)
            }
            0x2000..=0x3FFF => {
                let reg = (addr & 0x0007) as u8;
                match &self.mapper {
                    Some(m) => ppu.cpu_read(reg, m.as_ref()),
                    None => ppu.cpu_read(reg, &NullMapper),
                }
            }
            0x4015 => self.apu.as_mut().map_or(0, Apu::read_status),
            0x4016 => self.controller1.read(),
            0x4017 => self.controller2.read(),
            0x4000..=0x4013 => 0, // APU write-only registers
            0x4020..=0xFFFF => self.mapper.as_ref().map_or(0, |m| m.cpu_read(addr)),
            _ => 0,
        }
    }

    /// Writes a byte to the given CPU address.
    #[allow(clippy::match_same_arms)]
    pub(super) fn write(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
        match addr {
            0x0000..=0x1FFF => {
                let index = usize::from(addr) & 0x07FF;
                if let Some(cell) = self.ram.get_mut(index) {
                    *cell = val;
                }
            }
            0x2000..=0x3FFF => {
                let reg = (addr & 0x0007) as u8;
                if let Some(m) = &mut self.mapper {
                    ppu.cpu_write(reg, val, m.as_mut());
                }
            }
            0x4014 => self.oam_dma(val, ppu),
            0x4016 => {
                self.controller1.write(val);
                self.controller2.write(val);
            }
            0x4000..=0x4013 | 0x4015 | 0x4017 => {
                if let Some(apu) = &mut self.apu {
                    apu.write_register(addr, val);
                }
            }
            0x4020..=0xFFFF => {
                if let Some(m) = &mut self.mapper {
                    m.cpu_write(addr, val);
                }
            }
            _ => {}
        }
    }

    /// Performs OAM DMA: copies 256 bytes from CPU page to PPU OAM.
    fn oam_dma(&self, page: u8, ppu: &mut Ppu) {
        let base = u16::from(page) << 8;
        for i in 0u16..256 {
            let byte = self.read_raw(base.wrapping_add(i));
            ppu.oam_dma_write(byte);
        }
    }

    /// Reads a byte without routing to PPU, APU, or controllers (for DMA).
    fn read_raw(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => {
                let index = usize::from(addr) & 0x07FF;
                self.ram.get(index).copied().unwrap_or(0)
            }
            0x4020..=0xFFFF => self.mapper.as_ref().map_or(0, |m| m.cpu_read(addr)),
            _ => 0,
        }
    }
}

/// Null mapper used when no cartridge is loaded.
struct NullMapper;

impl Mapper for NullMapper {
    fn cpu_read(&self, _addr: u16) -> u8 {
        0
    }
    fn cpu_write(&mut self, _addr: u16, _val: u8) {}
    fn ppu_read(&self, _addr: u16) -> u8 {
        0
    }
    fn ppu_write(&mut self, _addr: u16, _val: u8) {}
    fn mirroring(&self) -> super::cartridge::Mirroring {
        super::cartridge::Mirroring::Horizontal
    }
    fn box_clone(&self) -> Box<dyn Mapper> {
        Box::new(Self)
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn ram_read_write() {
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();
        bus.write(0x0000, 0x42, &mut ppu);
        assert_eq!(bus.read(0x0000, &mut ppu), 0x42);
    }

    #[test]
    fn ram_mirrors() {
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();
        bus.write(0x0000, 0xAB, &mut ppu);
        assert_eq!(bus.read(0x0800, &mut ppu), 0xAB);
        assert_eq!(bus.read(0x1000, &mut ppu), 0xAB);
        assert_eq!(bus.read(0x1800, &mut ppu), 0xAB);
    }

    #[test]
    fn cartridge_prg_rom_read() {
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();

        let mut prg = vec![0x00; 16_384];
        prg[0] = 0xEA;
        prg[0x3FFC] = 0x00;
        prg[0x3FFD] = 0x80;

        let cart = Cartridge::from_ines(&make_ines(&prg)).expect("test ROM should be valid");
        bus.load_cartridge(cart)
            .expect("mapper 0 should be supported");

        assert_eq!(bus.read(0x8000, &mut ppu), 0xEA);
        assert_eq!(bus.read(0xC000, &mut ppu), 0xEA);
        assert_eq!(bus.read(0xFFFC, &mut ppu), 0x00);
        assert_eq!(bus.read(0xFFFD, &mut ppu), 0x80);
    }

    #[test]
    fn no_cartridge_reads_zero() {
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();
        assert_eq!(bus.read(0x8000, &mut ppu), 0);
    }

    #[test]
    fn controller_strobe_and_read() {
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();

        bus.set_controller_buttons(0, Buttons::A | Buttons::START);
        bus.write(0x4016, 1, &mut ppu);
        bus.write(0x4016, 0, &mut ppu);

        assert_eq!(bus.read(0x4016, &mut ppu), 1); // A
        assert_eq!(bus.read(0x4016, &mut ppu), 0); // B
        assert_eq!(bus.read(0x4016, &mut ppu), 0); // Select
        assert_eq!(bus.read(0x4016, &mut ppu), 1); // Start
    }

    #[test]
    fn apu_register_write_read() {
        let mut bus = Bus::new();
        bus.apu = Some(Apu::new());
        let mut ppu = Ppu::new();

        // Enable pulse 1.
        bus.write(0x4015, 0x01, &mut ppu);
        let status = bus.read(0x4015, &mut ppu);
        // Pulse 1 length counter is 0 (no length loaded), so bit 0 is 0.
        assert_eq!(status & 0x01, 0);
    }

    fn make_ines(prg: &[u8]) -> Vec<u8> {
        let prg_banks = prg.len() / 16_384;
        let mut data = Vec::new();
        data.extend_from_slice(b"NES\x1a");
        data.push(prg_banks as u8);
        data.push(0);
        data.extend_from_slice(&[0u8; 10]);
        data.extend_from_slice(prg);
        data
    }
}
