//! MOS 6502 CPU (NES variant 2A03 — no decimal mode).
//!
//! The CPU executes instructions from the memory bus, updating its
//! internal registers and status flags. It does not own memory;
//! all reads and writes go through the [`super::bus::Bus`].

#![allow(dead_code)]

mod addr;
mod handlers;
mod opcodes;
mod step;

#[allow(unused_imports)]
pub(crate) use step::{StepErr, StepOk, StepResult};

use crate::nes::bus::Bus;
use crate::nes::ppu::Ppu;

/// Status flag bits in the P register.
pub(super) mod flags {
    /// Carry flag.
    pub(in crate::nes) const CARRY: u8 = 1 << 0;
    /// Zero flag.
    pub(in crate::nes) const ZERO: u8 = 1 << 1;
    /// IRQ disable flag.
    pub(in crate::nes) const IRQ_DISABLE: u8 = 1 << 2;
    /// Decimal mode (not used on NES but the flag exists).
    pub(in crate::nes) const DECIMAL: u8 = 1 << 3;
    /// Break flag (set on BRK, PHP).
    pub(in crate::nes) const BREAK: u8 = 1 << 4;
    /// Unused flag (always set).
    pub(in crate::nes) const UNUSED: u8 = 1 << 5;
    /// Overflow flag.
    pub(in crate::nes) const OVERFLOW: u8 = 1 << 6;
    /// Negative flag.
    pub(in crate::nes) const NEGATIVE: u8 = 1 << 7;
}

/// NMI vector address in CPU memory.
const NMI_VECTOR: u16 = 0xFFFA;

/// Reset vector address in CPU memory.
const RESET_VECTOR: u16 = 0xFFFC;

/// IRQ/BRK vector address in CPU memory.
const IRQ_VECTOR: u16 = 0xFFFE;

/// NES CPU registers and cycle counter.
pub(crate) struct Cpu {
    /// Accumulator.
    pub(super) a: u8,
    /// X index register.
    pub(super) x: u8,
    /// Y index register.
    pub(super) y: u8,
    /// Stack pointer (offset into page $01).
    pub(super) sp: u8,
    /// Program counter.
    pub(super) pc: u16,
    /// Processor status flags.
    pub(super) status: u8,
    /// Elapsed cycle count (for PPU/APU synchronization).
    pub(super) cycles: u64,
    /// NMI has been requested (processed before next instruction).
    nmi_pending: bool,
    /// IRQ has been requested (processed if `IRQ_DISABLE` is clear).
    irq_pending: bool,
}

impl Cpu {
    /// Creates a CPU in its power-on state.
    ///
    /// The program counter is set to 0 here; a real reset sequence
    /// reads the reset vector from the bus at $FFFC–$FFFD.
    pub(crate) fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            pc: 0,
            status: flags::UNUSED | flags::IRQ_DISABLE,
            cycles: 0,
            nmi_pending: false,
            irq_pending: false,
        }
    }

    /// Signals an NMI to the CPU. It will be handled before the
    /// next instruction executes.
    pub(crate) fn request_nmi(&mut self) {
        self.nmi_pending = true;
    }

    /// Signals an IRQ to the CPU. It will be handled before the
    /// next instruction if the IRQ disable flag is clear.
    pub(crate) fn request_irq(&mut self) {
        self.irq_pending = true;
    }

    /// Performs a reset sequence: reads the reset vector from
    /// $FFFC–$FFFD and sets PC, matching power-on behavior.
    pub(crate) fn reset(&mut self, bus: &mut Bus, ppu: &mut Ppu) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.status = flags::UNUSED | flags::IRQ_DISABLE;
        self.nmi_pending = false;
        self.irq_pending = false;
        let lo = u16::from(bus.read(RESET_VECTOR, ppu));
        let hi = u16::from(bus.read(RESET_VECTOR + 1, ppu));
        self.pc = hi << 8 | lo;
    }

    /// Executes a single instruction and returns the result.
    ///
    /// If an NMI is pending, it is handled first (pushes PC and
    /// status, loads PC from $FFFA–$FFFB, consumes 7 cycles).
    pub(crate) fn step(&mut self, bus: &mut Bus, ppu: &mut Ppu) -> StepResult {
        if self.nmi_pending {
            self.handle_nmi(bus, ppu);
            return Ok(StepOk::Jump { cycles: 7 });
        }

        if self.irq_pending && !self.get_flag(flags::IRQ_DISABLE) {
            self.handle_irq(bus, ppu);
            return Ok(StepOk::Jump { cycles: 7 });
        }
        self.irq_pending = false;

        let opcode = bus.read(self.pc, ppu);
        let entry = opcodes::lookup(opcode)?;
        let operand = addr::resolve(entry.mode, self, bus, ppu);
        let ok = (entry.handler)(self, bus, ppu, operand, entry.base_cycles)?;
        self.apply(ok);
        Ok(ok)
    }

    /// Applies a successful step result to the CPU state.
    fn apply(&mut self, ok: StepOk) {
        match ok {
            StepOk::Advance { size, cycles } => {
                self.pc = self.pc.wrapping_add(u16::from(size));
                self.cycles = self.cycles.wrapping_add(u64::from(cycles));
            }
            StepOk::Jump { cycles } => {
                self.cycles = self.cycles.wrapping_add(u64::from(cycles));
            }
        }
    }

    // ── Interrupt handling ─────────────────────────────────────────

    /// Handles a pending NMI: push PC and status, load NMI vector.
    fn handle_nmi(&mut self, bus: &mut Bus, ppu: &mut Ppu) {
        self.nmi_pending = false;
        self.push_u16(bus, ppu, self.pc);
        // Push status with BREAK clear and UNUSED set.
        self.push(bus, ppu, (self.status & !flags::BREAK) | flags::UNUSED);
        self.set_flag(flags::IRQ_DISABLE, true);
        let lo = u16::from(bus.read(NMI_VECTOR, ppu));
        let hi = u16::from(bus.read(NMI_VECTOR + 1, ppu));
        self.pc = hi << 8 | lo;
        self.cycles = self.cycles.wrapping_add(7);
    }

    /// Handles a pending IRQ: push PC and status, load IRQ vector.
    fn handle_irq(&mut self, bus: &mut Bus, ppu: &mut Ppu) {
        self.irq_pending = false;
        self.push_u16(bus, ppu, self.pc);
        self.push(bus, ppu, (self.status & !flags::BREAK) | flags::UNUSED);
        self.set_flag(flags::IRQ_DISABLE, true);
        let lo = u16::from(bus.read(IRQ_VECTOR, ppu));
        let hi = u16::from(bus.read(IRQ_VECTOR + 1, ppu));
        self.pc = hi << 8 | lo;
        self.cycles = self.cycles.wrapping_add(7);
    }

    // ── Flag helpers ──────────────────────────────────────────────

    /// Returns `true` if the given status flag is set.
    pub(super) fn get_flag(&self, flag: u8) -> bool {
        self.status & flag != 0
    }

    /// Sets or clears a status flag.
    pub(super) fn set_flag(&mut self, flag: u8, val: bool) {
        if val {
            self.status |= flag;
        } else {
            self.status &= !flag;
        }
    }

    /// Updates the Negative and Zero flags from a value.
    pub(super) fn update_nz(&mut self, val: u8) {
        self.set_flag(flags::ZERO, val == 0);
        self.set_flag(flags::NEGATIVE, val & 0x80 != 0);
    }

    // ── Stack helpers (page $01) ──────────────────────────────────

    /// Pushes a byte onto the stack.
    pub(super) fn push(&mut self, bus: &mut Bus, ppu: &mut Ppu, val: u8) {
        let addr = 0x0100 | u16::from(self.sp);
        bus.write(addr, val, ppu);
        self.sp = self.sp.wrapping_sub(1);
    }

    /// Pops a byte from the stack.
    pub(super) fn pop(&mut self, bus: &mut Bus, ppu: &mut Ppu) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        let addr = 0x0100 | u16::from(self.sp);
        bus.read(addr, ppu)
    }

    /// Pushes a 16-bit value onto the stack (high byte first).
    pub(super) fn push_u16(&mut self, bus: &mut Bus, ppu: &mut Ppu, val: u16) {
        let (hi, lo) = ((val >> 8) as u8, val as u8);
        self.push(bus, ppu, hi);
        self.push(bus, ppu, lo);
    }

    /// Pops a 16-bit value from the stack (low byte first).
    pub(super) fn pop_u16(&mut self, bus: &mut Bus, ppu: &mut Ppu) -> u16 {
        let lo = u16::from(self.pop(bus, ppu));
        let hi = u16::from(self.pop(bus, ppu));
        hi << 8 | lo
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::nes::bus::Bus;
    use crate::nes::ppu::Ppu;

    fn load_program(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, origin: u16, program: &[u8]) {
        cpu.pc = origin;
        for (i, &byte) in program.iter().enumerate() {
            bus.write(origin.wrapping_add(i as u16), byte, ppu);
        }
    }

    fn run_until_halt(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu) -> u64 {
        let start = cpu.cycles;
        while cpu.step(bus, ppu).is_ok() {}
        cpu.cycles - start
    }

    #[test]
    fn lda_immediate_sta_zeropage() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();
        load_program(
            &mut cpu,
            &mut bus,
            &mut ppu,
            0x0200,
            &[0xA9, 0x42, 0x85, 0x10, 0x00],
        );
        let _ = run_until_halt(&mut cpu, &mut bus, &mut ppu);

        assert_eq!(cpu.a, 0x42);
        assert_eq!(bus.read(0x0010, &mut ppu), 0x42);
    }

    #[test]
    fn adc_with_carry() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();
        load_program(
            &mut cpu,
            &mut bus,
            &mut ppu,
            0x0200,
            &[0x38, 0xA9, 0x01, 0x69, 0x01, 0x00],
        );
        let _ = run_until_halt(&mut cpu, &mut bus, &mut ppu);
        assert_eq!(cpu.a, 3);
    }

    #[test]
    fn adc_overflow_and_carry_flags() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();
        load_program(
            &mut cpu,
            &mut bus,
            &mut ppu,
            0x0200,
            &[0x18, 0xA9, 0xFF, 0x69, 0x01, 0x00],
        );
        let _ = run_until_halt(&mut cpu, &mut bus, &mut ppu);
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.get_flag(flags::CARRY));
        assert!(cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn branch_bne_loop() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();
        load_program(
            &mut cpu,
            &mut bus,
            &mut ppu,
            0x0200,
            &[0xA2, 0x03, 0xCA, 0xD0, 0xFD, 0x00],
        );
        let _ = run_until_halt(&mut cpu, &mut bus, &mut ppu);
        assert_eq!(cpu.x, 0x00);
        assert!(cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn jsr_rts_round_trip() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();
        load_program(
            &mut cpu,
            &mut bus,
            &mut ppu,
            0x0200,
            &[0x20, 0x10, 0x02, 0xA9, 0xAA, 0x00],
        );
        load_program(&mut cpu, &mut bus, &mut ppu, 0x0210, &[0xA9, 0x55, 0x60]);
        cpu.pc = 0x0200;
        let _ = run_until_halt(&mut cpu, &mut bus, &mut ppu);
        assert_eq!(cpu.a, 0xAA);
    }

    #[test]
    fn push_pop_stack() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();
        load_program(
            &mut cpu,
            &mut bus,
            &mut ppu,
            0x0200,
            &[0xA9, 0x42, 0x48, 0xA9, 0x00, 0x68, 0x00],
        );
        let _ = run_until_halt(&mut cpu, &mut bus, &mut ppu);
        assert_eq!(cpu.a, 0x42);
    }

    #[test]
    fn jmp_absolute() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();
        load_program(&mut cpu, &mut bus, &mut ppu, 0x0200, &[0x4C, 0x10, 0x02]);
        load_program(&mut cpu, &mut bus, &mut ppu, 0x0210, &[0xA9, 0x77, 0x00]);
        cpu.pc = 0x0200;
        let _ = run_until_halt(&mut cpu, &mut bus, &mut ppu);
        assert_eq!(cpu.a, 0x77);
    }

    #[test]
    fn step_returns_correct_cycles() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();
        load_program(
            &mut cpu,
            &mut bus,
            &mut ppu,
            0x0200,
            &[0xA9, 0x42, 0xEA, 0x00],
        );

        let ok1 = cpu.step(&mut bus, &mut ppu);
        assert!(matches!(ok1, Ok(StepOk::Advance { cycles: 2, size: 2 })));
        let ok2 = cpu.step(&mut bus, &mut ppu);
        assert!(matches!(ok2, Ok(StepOk::Advance { cycles: 2, size: 1 })));
        let err = cpu.step(&mut bus, &mut ppu);
        assert!(matches!(err, Err(StepErr::Break)));
    }

    #[test]
    fn illegal_opcode_returns_error() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();
        let mut ppu = Ppu::new();
        load_program(&mut cpu, &mut bus, &mut ppu, 0x0200, &[0x02]);
        let result = cpu.step(&mut bus, &mut ppu);
        assert!(matches!(result, Err(StepErr::IllegalOpcode(0x02))));
    }
}
