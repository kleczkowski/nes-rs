//! 6502 addressing mode resolution.
//!
//! Each addressing mode reads 0–2 operand bytes after the opcode
//! and computes an effective address. The resolved [`Operand`]
//! carries the address, instruction size, and page-cross flag.

use super::Cpu;
use crate::nes::bus::Bus;
use crate::nes::ppu::Ppu;

/// 6502 addressing modes.
#[derive(Debug, Clone, Copy)]
pub(super) enum AddrMode {
    /// No operand (e.g., CLC, RTS).
    Implicit,
    /// Operand is the accumulator register.
    Accumulator,
    /// 8-bit literal at PC+1.
    Immediate,
    /// 8-bit zero-page address at PC+1.
    ZeroPage,
    /// Zero-page address + X (wraps within page zero).
    ZeroPageX,
    /// Zero-page address + Y (wraps within page zero).
    ZeroPageY,
    /// 16-bit absolute address at PC+1..PC+2.
    Absolute,
    /// Absolute + X (may cross page).
    AbsoluteX,
    /// Absolute + Y (may cross page).
    AbsoluteY,
    /// 16-bit pointer at PC+1..PC+2, used only by JMP.
    /// Replicates the page-boundary bug: if low byte is $FF,
    /// the high byte wraps within the same page.
    Indirect,
    /// (Indirect,X): zero-page pointer + X → 16-bit address.
    IndirectX,
    /// (Indirect),Y: zero-page pointer → 16-bit base + Y.
    IndirectY,
    /// Signed 8-bit offset from PC+2 (branches only).
    Relative,
}

/// Resolved operand from an addressing mode.
#[derive(Debug, Clone, Copy)]
pub(super) struct Operand {
    /// Effective address (meaningless for Implicit/Accumulator).
    pub(super) addr: u16,
    /// Total instruction size in bytes (1–3), including the opcode.
    pub(super) size: u8,
    /// Whether computing the address crossed a 256-byte page boundary.
    pub(super) page_cross: bool,
}

/// Resolves an addressing mode to an [`Operand`].
#[allow(clippy::match_same_arms)]
pub(super) fn resolve(mode: AddrMode, cpu: &Cpu, bus: &mut Bus, ppu: &mut Ppu) -> Operand {
    let pc = cpu.pc;
    match mode {
        AddrMode::Implicit => Operand {
            addr: 0,
            size: 1,
            page_cross: false,
        },
        AddrMode::Accumulator => Operand {
            addr: 0,
            size: 1,
            page_cross: false,
        },
        AddrMode::Immediate => Operand {
            addr: pc.wrapping_add(1),
            size: 2,
            page_cross: false,
        },
        AddrMode::ZeroPage => Operand {
            addr: u16::from(bus.read(pc.wrapping_add(1), ppu)),
            size: 2,
            page_cross: false,
        },
        AddrMode::ZeroPageX => Operand {
            addr: u16::from(bus.read(pc.wrapping_add(1), ppu).wrapping_add(cpu.x)),
            size: 2,
            page_cross: false,
        },
        AddrMode::ZeroPageY => Operand {
            addr: u16::from(bus.read(pc.wrapping_add(1), ppu).wrapping_add(cpu.y)),
            size: 2,
            page_cross: false,
        },
        AddrMode::Absolute => Operand {
            addr: read_u16(bus, ppu, pc.wrapping_add(1)),
            size: 3,
            page_cross: false,
        },
        AddrMode::AbsoluteX => {
            let base = read_u16(bus, ppu, pc.wrapping_add(1));
            let addr = base.wrapping_add(u16::from(cpu.x));
            Operand {
                addr,
                size: 3,
                page_cross: crosses_page(base, addr),
            }
        }
        AddrMode::AbsoluteY => {
            let base = read_u16(bus, ppu, pc.wrapping_add(1));
            let addr = base.wrapping_add(u16::from(cpu.y));
            Operand {
                addr,
                size: 3,
                page_cross: crosses_page(base, addr),
            }
        }
        AddrMode::Indirect => {
            let ptr = read_u16(bus, ppu, pc.wrapping_add(1));
            let addr = read_u16_wrapped(bus, ppu, ptr);
            Operand {
                addr,
                size: 3,
                page_cross: false,
            }
        }
        AddrMode::IndirectX => {
            let base = bus.read(pc.wrapping_add(1), ppu).wrapping_add(cpu.x);
            let addr = read_u16_zp(bus, ppu, base);
            Operand {
                addr,
                size: 2,
                page_cross: false,
            }
        }
        AddrMode::IndirectY => {
            let base_ptr = bus.read(pc.wrapping_add(1), ppu);
            let base = read_u16_zp(bus, ppu, base_ptr);
            let addr = base.wrapping_add(u16::from(cpu.y));
            Operand {
                addr,
                size: 2,
                page_cross: crosses_page(base, addr),
            }
        }
        AddrMode::Relative => {
            let offset = bus.read(pc.wrapping_add(1), ppu) as i8;
            let next_pc = pc.wrapping_add(2);
            let addr = next_pc.wrapping_add(offset as u16);
            Operand {
                addr,
                size: 2,
                page_cross: crosses_page(next_pc, addr),
            }
        }
    }
}

/// Little-endian 16-bit read from two consecutive addresses.
fn read_u16(bus: &mut Bus, ppu: &mut Ppu, addr: u16) -> u16 {
    let lo = u16::from(bus.read(addr, ppu));
    let hi = u16::from(bus.read(addr.wrapping_add(1), ppu));
    hi << 8 | lo
}

/// 16-bit read from zero page with wrapping (high byte wraps to $00).
fn read_u16_zp(bus: &mut Bus, ppu: &mut Ppu, addr: u8) -> u16 {
    let lo = u16::from(bus.read(u16::from(addr), ppu));
    let hi = u16::from(bus.read(u16::from(addr.wrapping_add(1)), ppu));
    hi << 8 | lo
}

/// 16-bit read that replicates the 6502 JMP indirect page-boundary bug.
fn read_u16_wrapped(bus: &mut Bus, ppu: &mut Ppu, ptr: u16) -> u16 {
    let lo = u16::from(bus.read(ptr, ppu));
    let hi_addr = (ptr & 0xFF00) | u16::from((ptr as u8).wrapping_add(1));
    let hi = u16::from(bus.read(hi_addr, ppu));
    hi << 8 | lo
}

/// Returns `true` if two addresses are in different 256-byte pages.
fn crosses_page(a: u16, b: u16) -> bool {
    (a & 0xFF00) != (b & 0xFF00)
}
