# CPU — The 6502 Processor

The NES CPU is a Ricoh 2A03, a custom variant of the MOS Technology 6502. It is an 8-bit processor with a 16-bit address bus, giving it access to 64 KB of memory.

## Registers

The 6502 has a small register set — just six registers:

| Register | Size | Description |
|----------|------|-------------|
| **A** | 8-bit | Accumulator — main register for arithmetic and logic |
| **X** | 8-bit | Index register X — used for addressing and loop counters |
| **Y** | 8-bit | Index register Y — used for addressing and loop counters |
| **SP** | 8-bit | Stack pointer — offset into the stack page (`$0100`–`$01FF`) |
| **PC** | 16-bit | Program counter — address of the next instruction |
| **P** | 8-bit | Processor status — flag bits |

### Status flags (P register)

```text
  7  6  5  4  3  2  1  0
  N  V  -  B  D  I  Z  C
```

| Bit | Flag | Description |
|-----|------|-------------|
| 0 | **C** (Carry) | Set when arithmetic produces a carry/borrow |
| 1 | **Z** (Zero) | Set when the result is zero |
| 2 | **I** (IRQ Disable) | When set, maskable interrupts are ignored |
| 3 | **D** (Decimal) | Exists but has no effect on the 2A03 |
| 4 | **B** (Break) | Set in the pushed status byte by BRK/PHP |
| 5 | **—** (Unused) | Always reads as 1 |
| 6 | **V** (Overflow) | Set when signed arithmetic overflows |
| 7 | **N** (Negative) | Set when bit 7 of the result is 1 |

## Addressing modes

The 6502 has 13 addressing modes that determine how an instruction finds its operand:

| Mode | Syntax | Description |
|------|--------|-------------|
| Implied | `CLC` | No operand; operates on a register |
| Accumulator | `ASL A` | Operates on the accumulator |
| Immediate | `LDA #$42` | Operand is the next byte |
| Zero Page | `LDA $10` | Operand is at an 8-bit address (page zero) |
| Zero Page,X | `LDA $10,X` | Zero page address + X register |
| Zero Page,Y | `LDX $10,Y` | Zero page address + Y register |
| Absolute | `LDA $1234` | Full 16-bit address |
| Absolute,X | `LDA $1234,X` | 16-bit address + X (+1 cycle if page crossed) |
| Absolute,Y | `LDA $1234,Y` | 16-bit address + Y (+1 cycle if page crossed) |
| Indirect | `JMP ($1234)` | Address stored at the given location (JMP only) |
| (Indirect,X) | `LDA ($10,X)` | Indexed indirect — pointer in zero page |
| (Indirect),Y | `LDA ($10),Y` | Indirect indexed — add Y to dereferenced pointer |
| Relative | `BNE $FD` | Signed 8-bit offset from PC (branches only) |

## Instruction execution

Each instruction follows this sequence:

1. **Fetch opcode** — Read the byte at PC from the bus.
2. **Decode** — Look up the opcode in a 256-entry table to determine the instruction handler, addressing mode, operand size, and base cycle count.
3. **Resolve operand** — Apply the addressing mode to compute the effective address or immediate value.
4. **Execute** — Run the instruction handler, which may read/write memory and update flags.
5. **Advance** — Increment PC by the instruction size and add the cycle count.

In nes-rs, this is implemented in `Cpu::step()`:

```rust
pub(crate) fn step(&mut self, bus: &mut Bus, ppu: &mut Ppu) -> StepResult {
    if self.nmi_pending {
        self.handle_nmi(bus, ppu);
        return Ok(StepOk::Jump { cycles: 7 });
    }
    if self.irq_pending && !self.get_flag(flags::IRQ_DISABLE) {
        self.handle_irq(bus, ppu);
        return Ok(StepOk::Jump { cycles: 7 });
    }
    let opcode = bus.read(self.pc, ppu);
    let entry = opcodes::lookup(opcode)?;
    let operand = addr::resolve(entry.mode, self, bus, ppu);
    let ok = (entry.handler)(self, bus, ppu, operand, entry.base_cycles)?;
    self.apply(ok);
    Ok(ok)
}
```

Illegal opcodes return `StepErr::IllegalOpcode`, and the `BRK` instruction returns `StepErr::Break`.

## The stack

The 6502 stack lives in page 1 of memory (`$0100`–`$01FF`). The stack pointer (SP) is an 8-bit offset within this page — it starts at `$FD` after reset and grows **downward** (push decrements SP, pop increments it).

Instructions that use the stack:
- `PHA` / `PLA` — Push/pull accumulator
- `PHP` / `PLP` — Push/pull processor status
- `JSR` / `RTS` — Subroutine call/return (pushes/pops PC)
- `BRK` / `RTI` — Interrupt entry/return (pushes/pops PC and status)

## Interrupts

The 6502 supports three types of interrupts:

### RESET

Triggered at power-on. The CPU reads the 16-bit **reset vector** from `$FFFC`–`$FFFD` and jumps to that address. SP is set to `$FD`, and the IRQ disable flag is set.

### NMI (Non-Maskable Interrupt)

The PPU fires an NMI at the start of every vertical blanking period (scanline 241, cycle 1). NMI cannot be disabled — it always fires when the PPU's NMI-enable bit (PPUCTRL bit 7) is set. The interrupt sequence:

1. Push PC high byte, PC low byte, and status onto the stack.
2. Set the IRQ disable flag.
3. Load PC from the NMI vector at `$FFFA`–`$FFFB`.
4. Consumes 7 CPU cycles.

NMI is the primary synchronization mechanism between CPU and PPU. Games typically use the NMI handler to update scroll registers, transfer sprite data, and perform other time-sensitive PPU operations during the VBlank window.

### IRQ (Interrupt Request)

A maskable interrupt that can come from the APU frame counter, the APU DMC channel, or mapper hardware (e.g., MMC3's scanline counter). IRQ is only serviced when the I (IRQ disable) flag is clear. The sequence is identical to NMI, but the vector is read from `$FFFE`–`$FFFF`.

## Power-on state

After reset, the CPU enters this state:

| Register | Value |
|----------|-------|
| A, X, Y | 0 |
| SP | `$FD` |
| P | `$24` (IRQ disabled, unused bit set) |
| PC | Value at `$FFFC`–`$FFFD` (reset vector) |
