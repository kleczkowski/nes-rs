//! Individual instruction handler functions.
//!
//! Each handler has the signature:
//! ```text
//! fn(cpu, bus, operand, base_cycles) -> StepResult
//! ```
//! Handlers return [`StepOk::Advance`] for normal instructions or
//! [`StepOk::Jump`] when they set PC directly (branches, jumps).

#![allow(clippy::unnecessary_wraps)]

use super::Cpu;
use super::addr::Operand;
use super::flags;
use super::step::{StepErr, StepOk, StepResult};
use crate::nes::bus::Bus;
use crate::nes::ppu::Ppu;

/// Handler function pointer type.
pub(super) type OpHandler = fn(&mut Cpu, &mut Bus, &mut Ppu, Operand, u8) -> StepResult;

// ── Helpers ──────────────────────────────────────────────────────

/// Returns +1 if `page_cross` is true, 0 otherwise.
fn page_penalty(op: Operand) -> u8 {
    u8::from(op.page_cross)
}

fn advance(op: Operand, cycles: u8) -> StepResult {
    Ok(StepOk::Advance {
        size: op.size,
        cycles,
    })
}

// ── Arithmetic ───────────────────────────────────────────────────

/// ADC — Add with carry.
pub(super) fn adc(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    let val = bus.read(op.addr, ppu);
    let carry = u16::from(cpu.get_flag(flags::CARRY));
    let sum = u16::from(cpu.a) + u16::from(val) + carry;
    let result = sum as u8;
    cpu.set_flag(flags::CARRY, sum > 0xFF);
    cpu.set_flag(
        flags::OVERFLOW,
        (cpu.a ^ result) & (val ^ result) & 0x80 != 0,
    );
    cpu.a = result;
    cpu.update_nz(result);
    advance(op, cy + page_penalty(op))
}

/// SBC — Subtract with borrow (A - M - (1-C)).
pub(super) fn sbc(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    let val = bus.read(op.addr, ppu);
    let carry = u16::from(cpu.get_flag(flags::CARRY));
    let diff = u16::from(cpu.a)
        .wrapping_sub(u16::from(val))
        .wrapping_sub(1 - carry);
    let result = diff as u8;
    cpu.set_flag(flags::CARRY, diff < 0x100);
    cpu.set_flag(
        flags::OVERFLOW,
        (cpu.a ^ result) & (!val ^ result) & 0x80 != 0,
    );
    cpu.a = result;
    cpu.update_nz(result);
    advance(op, cy + page_penalty(op))
}

// ── Logic ────────────────────────────────────────────────────────

/// AND — Logical AND with accumulator.
pub(super) fn and(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    cpu.a &= bus.read(op.addr, ppu);
    cpu.update_nz(cpu.a);
    advance(op, cy + page_penalty(op))
}

/// ORA — Logical OR with accumulator.
pub(super) fn ora(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    cpu.a |= bus.read(op.addr, ppu);
    cpu.update_nz(cpu.a);
    advance(op, cy + page_penalty(op))
}

/// EOR — Logical XOR with accumulator.
pub(super) fn eor(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    cpu.a ^= bus.read(op.addr, ppu);
    cpu.update_nz(cpu.a);
    advance(op, cy + page_penalty(op))
}

// ── Shifts & Rotates ─────────────────────────────────────────────

/// ASL — Arithmetic shift left (accumulator or memory).
pub(super) fn asl_acc(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.set_flag(flags::CARRY, cpu.a & 0x80 != 0);
    cpu.a <<= 1;
    cpu.update_nz(cpu.a);
    advance(op, cy)
}

/// ASL — Arithmetic shift left (memory).
pub(super) fn asl_mem(
    cpu: &mut Cpu,
    bus: &mut Bus,
    ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let mut val = bus.read(op.addr, ppu);
    cpu.set_flag(flags::CARRY, val & 0x80 != 0);
    val <<= 1;
    bus.write(op.addr, val, ppu);
    cpu.update_nz(val);
    advance(op, cy)
}

/// LSR — Logical shift right (accumulator).
pub(super) fn lsr_acc(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.set_flag(flags::CARRY, cpu.a & 0x01 != 0);
    cpu.a >>= 1;
    cpu.update_nz(cpu.a);
    advance(op, cy)
}

/// LSR — Logical shift right (memory).
pub(super) fn lsr_mem(
    cpu: &mut Cpu,
    bus: &mut Bus,
    ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let mut val = bus.read(op.addr, ppu);
    cpu.set_flag(flags::CARRY, val & 0x01 != 0);
    val >>= 1;
    bus.write(op.addr, val, ppu);
    cpu.update_nz(val);
    advance(op, cy)
}

/// ROL — Rotate left (accumulator).
pub(super) fn rol_acc(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let old_carry = u8::from(cpu.get_flag(flags::CARRY));
    cpu.set_flag(flags::CARRY, cpu.a & 0x80 != 0);
    cpu.a = (cpu.a << 1) | old_carry;
    cpu.update_nz(cpu.a);
    advance(op, cy)
}

/// ROL — Rotate left (memory).
pub(super) fn rol_mem(
    cpu: &mut Cpu,
    bus: &mut Bus,
    ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let old_carry = u8::from(cpu.get_flag(flags::CARRY));
    let mut val = bus.read(op.addr, ppu);
    cpu.set_flag(flags::CARRY, val & 0x80 != 0);
    val = (val << 1) | old_carry;
    bus.write(op.addr, val, ppu);
    cpu.update_nz(val);
    advance(op, cy)
}

/// ROR — Rotate right (accumulator).
pub(super) fn ror_acc(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let old_carry = u8::from(cpu.get_flag(flags::CARRY));
    cpu.set_flag(flags::CARRY, cpu.a & 0x01 != 0);
    cpu.a = (cpu.a >> 1) | (old_carry << 7);
    cpu.update_nz(cpu.a);
    advance(op, cy)
}

/// ROR — Rotate right (memory).
pub(super) fn ror_mem(
    cpu: &mut Cpu,
    bus: &mut Bus,
    ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let old_carry = u8::from(cpu.get_flag(flags::CARRY));
    let mut val = bus.read(op.addr, ppu);
    cpu.set_flag(flags::CARRY, val & 0x01 != 0);
    val = (val >> 1) | (old_carry << 7);
    bus.write(op.addr, val, ppu);
    cpu.update_nz(val);
    advance(op, cy)
}

// ── Compare ──────────────────────────────────────────────────────

/// Generic compare: sets C, Z, N based on (reg - val).
fn compare(cpu: &mut Cpu, reg: u8, val: u8) {
    let diff = reg.wrapping_sub(val);
    cpu.set_flag(flags::CARRY, reg >= val);
    cpu.update_nz(diff);
}

/// CMP — Compare accumulator.
pub(super) fn cmp(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    compare(cpu, cpu.a, bus.read(op.addr, ppu));
    advance(op, cy + page_penalty(op))
}

/// CPX — Compare X register.
pub(super) fn cpx(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    compare(cpu, cpu.x, bus.read(op.addr, ppu));
    advance(op, cy)
}

/// CPY — Compare Y register.
pub(super) fn cpy(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    compare(cpu, cpu.y, bus.read(op.addr, ppu));
    advance(op, cy)
}

// ── Increment / Decrement ────────────────────────────────────────

/// INC — Increment memory.
pub(super) fn inc(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    let val = bus.read(op.addr, ppu).wrapping_add(1);
    bus.write(op.addr, val, ppu);
    cpu.update_nz(val);
    advance(op, cy)
}

/// DEC — Decrement memory.
pub(super) fn dec(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    let val = bus.read(op.addr, ppu).wrapping_sub(1);
    bus.write(op.addr, val, ppu);
    cpu.update_nz(val);
    advance(op, cy)
}

/// INX — Increment X.
pub(super) fn inx(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.x = cpu.x.wrapping_add(1);
    cpu.update_nz(cpu.x);
    advance(op, cy)
}

/// INY — Increment Y.
pub(super) fn iny(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.y = cpu.y.wrapping_add(1);
    cpu.update_nz(cpu.y);
    advance(op, cy)
}

/// DEX — Decrement X.
pub(super) fn dex(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.x = cpu.x.wrapping_sub(1);
    cpu.update_nz(cpu.x);
    advance(op, cy)
}

/// DEY — Decrement Y.
pub(super) fn dey(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.y = cpu.y.wrapping_sub(1);
    cpu.update_nz(cpu.y);
    advance(op, cy)
}

// ── Load ─────────────────────────────────────────────────────────

/// LDA — Load accumulator.
pub(super) fn lda(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    cpu.a = bus.read(op.addr, ppu);
    cpu.update_nz(cpu.a);
    advance(op, cy + page_penalty(op))
}

/// LDX — Load X register.
pub(super) fn ldx(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    cpu.x = bus.read(op.addr, ppu);
    cpu.update_nz(cpu.x);
    advance(op, cy + page_penalty(op))
}

/// LDY — Load Y register.
pub(super) fn ldy(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    cpu.y = bus.read(op.addr, ppu);
    cpu.update_nz(cpu.y);
    advance(op, cy + page_penalty(op))
}

// ── Store ────────────────────────────────────────────────────────

/// STA — Store accumulator.
pub(super) fn sta(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    bus.write(op.addr, cpu.a, ppu);
    advance(op, cy)
}

/// STX — Store X register.
pub(super) fn stx(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    bus.write(op.addr, cpu.x, ppu);
    advance(op, cy)
}

/// STY — Store Y register.
pub(super) fn sty(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    bus.write(op.addr, cpu.y, ppu);
    advance(op, cy)
}

// ── Transfer ─────────────────────────────────────────────────────

/// TAX — Transfer A to X.
pub(super) fn tax(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.x = cpu.a;
    cpu.update_nz(cpu.x);
    advance(op, cy)
}

/// TAY — Transfer A to Y.
pub(super) fn tay(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.y = cpu.a;
    cpu.update_nz(cpu.y);
    advance(op, cy)
}

/// TXA — Transfer X to A.
pub(super) fn txa(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.a = cpu.x;
    cpu.update_nz(cpu.a);
    advance(op, cy)
}

/// TYA — Transfer Y to A.
pub(super) fn tya(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.a = cpu.y;
    cpu.update_nz(cpu.a);
    advance(op, cy)
}

/// TSX — Transfer SP to X.
pub(super) fn tsx(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.x = cpu.sp;
    cpu.update_nz(cpu.x);
    advance(op, cy)
}

/// TXS — Transfer X to SP (does NOT set flags).
pub(super) fn txs(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.sp = cpu.x;
    advance(op, cy)
}

// ── Stack ────────────────────────────────────────────────────────

/// PHA — Push accumulator.
pub(super) fn pha(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    cpu.push(bus, ppu, cpu.a);
    advance(op, cy)
}

/// PHP — Push processor status (with B and U set).
pub(super) fn php(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    cpu.push(bus, ppu, cpu.status | flags::BREAK | flags::UNUSED);
    advance(op, cy)
}

/// PLA — Pull accumulator.
pub(super) fn pla(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    // pop mutates sp, which requires &mut Bus for the read address
    // computation, but the actual read is from the stack page.
    cpu.a = cpu.pop(bus, ppu);
    cpu.update_nz(cpu.a);
    advance(op, cy)
}

/// PLP — Pull processor status (B and U ignored on pull).
pub(super) fn plp(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    cpu.status = cpu.pop(bus, ppu) & !(flags::BREAK) | flags::UNUSED;
    advance(op, cy)
}

// ── Branch ───────────────────────────────────────────────────────

/// Generic branch: if `condition` is true, jump to `op.addr`.
fn branch(cpu: &mut Cpu, op: Operand, cy: u8, condition: bool) -> StepResult {
    if condition {
        cpu.pc = op.addr;
        Ok(StepOk::Jump {
            cycles: cy + 1 + page_penalty(op),
        })
    } else {
        advance(op, cy)
    }
}

/// BCC — Branch if carry clear.
pub(super) fn bcc(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let cond = !cpu.get_flag(flags::CARRY);
    branch(cpu, op, cy, cond)
}

/// BCS — Branch if carry set.
pub(super) fn bcs(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let cond = cpu.get_flag(flags::CARRY);
    branch(cpu, op, cy, cond)
}

/// BEQ — Branch if zero set.
pub(super) fn beq(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let cond = cpu.get_flag(flags::ZERO);
    branch(cpu, op, cy, cond)
}

/// BNE — Branch if zero clear.
pub(super) fn bne(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let cond = !cpu.get_flag(flags::ZERO);
    branch(cpu, op, cy, cond)
}

/// BMI — Branch if negative set.
pub(super) fn bmi(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let cond = cpu.get_flag(flags::NEGATIVE);
    branch(cpu, op, cy, cond)
}

/// BPL — Branch if negative clear.
pub(super) fn bpl(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let cond = !cpu.get_flag(flags::NEGATIVE);
    branch(cpu, op, cy, cond)
}

/// BVC — Branch if overflow clear.
pub(super) fn bvc(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let cond = !cpu.get_flag(flags::OVERFLOW);
    branch(cpu, op, cy, cond)
}

/// BVS — Branch if overflow set.
pub(super) fn bvs(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    let cond = cpu.get_flag(flags::OVERFLOW);
    branch(cpu, op, cy, cond)
}

// ── Jump / Subroutine ────────────────────────────────────────────

/// JMP — Jump to address.
pub(super) fn jmp(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.pc = op.addr;
    Ok(StepOk::Jump { cycles: cy })
}

/// JSR — Jump to subroutine (push return address - 1).
pub(super) fn jsr(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    let ret = cpu.pc.wrapping_add(u16::from(op.size)).wrapping_sub(1);
    cpu.push_u16(bus, ppu, ret);
    cpu.pc = op.addr;
    Ok(StepOk::Jump { cycles: cy })
}

/// RTS — Return from subroutine (pull PC + 1).
pub(super) fn rts(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    let _ = op;
    let addr = cpu.pop_u16(bus, ppu).wrapping_add(1);
    cpu.pc = addr;
    Ok(StepOk::Jump { cycles: cy })
}

/// RTI — Return from interrupt (pull status, then PC).
pub(super) fn rti(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    let _ = op;
    cpu.status = cpu.pop(bus, ppu) & !(flags::BREAK) | flags::UNUSED;
    cpu.pc = cpu.pop_u16(bus, ppu);
    Ok(StepOk::Jump { cycles: cy })
}

// ── Flag instructions ────────────────────────────────────────────

/// CLC — Clear carry flag.
pub(super) fn clc(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.set_flag(flags::CARRY, false);
    advance(op, cy)
}

/// SEC — Set carry flag.
pub(super) fn sec(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.set_flag(flags::CARRY, true);
    advance(op, cy)
}

/// CLI — Clear interrupt disable.
pub(super) fn cli(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.set_flag(flags::IRQ_DISABLE, false);
    advance(op, cy)
}

/// SEI — Set interrupt disable.
pub(super) fn sei(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.set_flag(flags::IRQ_DISABLE, true);
    advance(op, cy)
}

/// CLV — Clear overflow flag.
pub(super) fn clv(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.set_flag(flags::OVERFLOW, false);
    advance(op, cy)
}

/// CLD — Clear decimal flag.
pub(super) fn cld(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.set_flag(flags::DECIMAL, false);
    advance(op, cy)
}

/// SED — Set decimal flag.
pub(super) fn sed(
    cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    cpu.set_flag(flags::DECIMAL, true);
    advance(op, cy)
}

// ── Miscellaneous ────────────────────────────────────────────────

/// BIT — Test bits in memory against accumulator.
pub(super) fn bit(cpu: &mut Cpu, bus: &mut Bus, ppu: &mut Ppu, op: Operand, cy: u8) -> StepResult {
    let val = bus.read(op.addr, ppu);
    cpu.set_flag(flags::ZERO, cpu.a & val == 0);
    cpu.set_flag(flags::OVERFLOW, val & 0x40 != 0);
    cpu.set_flag(flags::NEGATIVE, val & 0x80 != 0);
    advance(op, cy)
}

/// NOP — No operation.
pub(super) fn nop(
    _cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    op: Operand,
    cy: u8,
) -> StepResult {
    advance(op, cy)
}

/// BRK — Force break.
pub(super) fn brk(
    _cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    _op: Operand,
    _cy: u8,
) -> StepResult {
    Err(StepErr::Break)
}

/// Illegal opcode handler (never called through the table directly;
/// `step()` catches illegal opcodes before dispatching).
pub(super) fn illegal(
    _cpu: &mut Cpu,
    _bus: &mut Bus,
    _ppu: &mut Ppu,
    _op: Operand,
    _cy: u8,
) -> StepResult {
    Err(StepErr::IllegalOpcode(0))
}
