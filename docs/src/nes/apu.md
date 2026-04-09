# APU — Audio Processing Unit

The APU generates all NES audio through five synthesis channels. It is integrated into the 2A03 CPU chip and is memory-mapped to CPU addresses `$4000`–`$4017`. The APU runs at the same clock rate as the CPU — one tick per CPU cycle.

## Channel overview

| Channel | Registers | Type | Waveform |
|---------|-----------|------|----------|
| Pulse 1 | `$4000`–`$4003` | Square wave | 4 selectable duty cycles (12.5%, 25%, 50%, 75%) |
| Pulse 2 | `$4004`–`$4007` | Square wave | Same as Pulse 1 |
| Triangle | `$4008`–`$400B` | Triangle wave | 32-step fixed waveform |
| Noise | `$400C`–`$400F` | Pseudo-random | 15-bit LFSR with two feedback modes |
| DMC | `$4010`–`$4013` | Sample playback | 1-bit delta-encoded PCM from ROM |

## Pulse channels

The two pulse channels are nearly identical. Each produces a square wave with a programmable frequency and one of four duty cycles:

```text
Duty 0 (12.5%): _ # _ _ _ _ _ _
Duty 1 (25.0%): _ # # _ _ _ _ _
Duty 2 (50.0%): _ # # # # _ _ _
Duty 3 (75.0%): # _ _ # # # # #
```

Each pulse channel has:
- An **11-bit timer** that controls the frequency. The output frequency is `CPU_clock / (16 * (timer_period + 1))`.
- An **envelope generator** that produces either a constant volume (0–15) or a decaying volume from 15 down to 0.
- A **length counter** that silences the channel after a programmed duration.
- A **sweep unit** that periodically shifts the timer period up or down, creating pitch bends.

The sweep units differ between the two channels: Pulse 1 uses one's complement for downward sweeps (negated value minus 1), while Pulse 2 uses two's complement. This means the two channels produce slightly different results when sweeping down — a quirk games must account for.

Pulse timers tick every **2 CPU cycles** (the "even cycle" constraint in the APU), while the triangle timer ticks every CPU cycle.

## Triangle channel

The triangle channel produces a fixed 32-step triangle waveform:

```text
15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
 0,  1,  2,  3,  4,  5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15
```

It has no volume control — the triangle is always at full amplitude or silent. Silencing is achieved by halting the sequencer, which causes the DAC to hold its last output value. This avoids pops that would occur from jumping to zero.

The triangle has:
- An **11-bit timer** (ticks every CPU cycle, not every 2 like pulses).
- A **linear counter** that gates the sequencer for a programmed number of quarter-frame clocks.
- A **length counter** (shared logic with other channels).

When the timer period is very low (< 2), the triangle oscillates at ultrasonic frequencies. The analog output stage of the real NES averages this to roughly 7.5, which nes-rs emulates by returning a fixed value of 7 in this case.

## Noise channel

The noise channel uses a **15-bit linear feedback shift register** (LFSR) to generate pseudo-random output:

```text
feedback = bit0 XOR bit_n
shift_register = (shift_register >> 1) | (feedback << 14)
output = (shift_register & 1) == 0 ? envelope : 0
```

The feedback bit position (`bit_n`) is selectable:
- **Mode 0** — Bit 1 feedback: produces long pseudo-random sequences (white noise).
- **Mode 1** — Bit 6 feedback: produces shorter, more metallic/buzzy sequences.

The noise frequency is set by a 4-bit index into a 16-entry lookup table of timer periods (from 4 to 4068 CPU cycles).

## DMC (Delta Modulation Channel)

The DMC plays back pre-recorded samples stored in ROM. Unlike the other channels, which synthesize waveforms, the DMC reads a stream of bytes from CPU memory and interprets each bit as a delta:

- Bit = 1: increment output level by 2 (capped at 127)
- Bit = 0: decrement output level by 2 (floored at 0)

This produces a 7-bit output (0–127), giving it much greater dynamic range than the other 4-bit channels.

The DMC has:
- A **rate timer** controlled by a 4-bit index into a rate table (54–428 CPU cycles per output change).
- A **sample address** and **sample length** that define the data to play.
- **Loop** and **IRQ** flags for continuous playback and end-of-sample notification.

Sample data is fetched from CPU address space via the bus. In nes-rs, the main emulation loop checks `Apu::dmc_sample_addr()` each cycle and calls `Apu::dmc_fill_sample()` when the DMC needs a new byte.

## Frame sequencer

The frame sequencer is a clock divider that generates periodic signals to clock the envelope, length counter, and sweep units:

| Signal | Clocks | Frequency |
|--------|--------|-----------|
| Quarter frame | Envelopes, triangle linear counter | ~240 Hz |
| Half frame | Length counters, sweep units | ~120 Hz |

The frame sequencer operates in one of two modes:

### 4-step mode (default)

```text
Step 0: quarter frame
Step 1: quarter frame + half frame
Step 2: quarter frame
Step 3: quarter frame + half frame + IRQ (if enabled)
```

### 5-step mode

```text
Step 0: quarter frame
Step 1: quarter frame + half frame
Step 2: quarter frame
Step 3: quarter frame + half frame
Step 4: (idle)
```

The 5-step mode never generates an IRQ. Games select the mode and IRQ inhibit flag by writing to `$4017`.

The step period is region-dependent:
- **NTSC**: 7,457 CPU cycles per step (~240 Hz quarter frame)
- **PAL**: 8,313 CPU cycles per step (~200 Hz quarter frame)

## Nonlinear mixer

The five channel outputs are combined by a nonlinear mixer that approximates the real NES's resistor-based DAC. The mixer uses two lookup tables derived from the NESDev wiki formulas:

**Pulse group** (Pulse 1 + Pulse 2):
```
pulse_out = 95.52 / (8128.0 / (p1 + p2) + 100.0)
```

**TND group** (Triangle + Noise + DMC):
```
tnd_out = 163.67 / (24329.0 / (3*t + 2*n + d) + 100.0)
```

The final output is `pulse_out + tnd_out`, a value between 0.0 and ~1.0. This nonlinear mixing means that channel volumes are not simply additive — adding a channel when others are already loud produces a smaller perceived volume increase, matching the real hardware's behavior.

nes-rs precomputes these lookup tables at initialization (31 entries for pulse, 203 for TND) to avoid floating-point math during mixing.

## Shared units

Several modular units are shared across channels:

### Envelope generator

Used by Pulse 1, Pulse 2, and Noise. Produces a 4-bit volume (0–15) that either:
- Holds a **constant value** (set by the game), or
- **Decays** from 15 to 0 over time (optionally looping back to 15).

Clocked by the frame sequencer's quarter-frame signal.

### Length counter

Used by all channels except DMC. When loaded with a value, it counts down each half-frame. When it reaches zero, the channel is silenced. The counter can be "halted" to hold its value indefinitely.

### Sweep unit

Used by the two pulse channels only. Periodically shifts the timer period right by a programmable amount and adds or subtracts the result, creating rising or falling pitch effects. If the target period would exceed 0x7FF or fall below 8, the channel is muted.

### Linear counter

Used by the triangle channel only. Similar to the length counter but clocked by the quarter-frame signal instead of the half-frame.
