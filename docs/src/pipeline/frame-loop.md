# Frame Loop & Synchronization

The emulation loop is the heart of nes-rs. It converts wall-clock time into the correct number of CPU, PPU, and APU cycles, keeping all three subsystems synchronized.

## High-level flow

```mermaid
flowchart TD
    A[Window event loop] --> B[Measure wall-clock delta]
    B --> RW{R held?}
    RW -->|Yes| RWR[Pop snapshot, restore]
    RWR --> J
    RW -->|No| C[Convert to CPU cycles]
    C --> FF{Tab held?}
    FF -->|Yes| FFM["Multiply dt by 4×"]
    FF -->|No| FFN[Use normal dt]
    FFM --> D
    FFN --> D
    D{Cycles remaining?} -->|Yes| E[Execute one CPU instruction]
    E --> F[Tick APU for N cycles]
    F --> G[Tick PPU via Bresenham]
    G --> H[Handle interrupts]
    H --> D
    D -->|No| I[Flush audio samples]
    I --> SS[Save snapshot to ring buffer]
    SS --> J[Upload framebuffer to GPU]
    J --> K[Draw frame]
    K --> A
```

## Time-to-cycles conversion

The frontend measures how much real time has passed since the last frame using `rl.get_time()` and passes the delta (in milliseconds) to `Nes::update()`:

```rust
let cpu_clock_hz = self.region.cpu_clock_hz();
let cpu_cycles_per_ms = f64::from(cpu_clock_hz) / 1000.0;
let target_cycles = (dt_ms * cpu_cycles_per_ms) as u64;
```

For NTSC at 60 FPS, each frame is ~16.67 ms, which works out to approximately 29,830 CPU cycles per frame. The delta is capped at 33 ms to prevent the emulator from trying to catch up after a stall (e.g., from an OS file dialog).

## The inner loop

Within `update()`, the emulator runs one CPU instruction at a time:

```rust
while cycles_run < target_cycles {
    let step = self.cpu_step();
    let cycles = step.cycles();
    cycles_run += cycles;

    // APU: tick once per CPU cycle
    for _ in 0..cycles {
        apu.tick();
        // Down-sample to 44.1 kHz via Bresenham
    }

    // PPU: tick ppu_num/ppu_den dots per CPU cycle
    for _ in 0..cycles {
        ppu_frac += ppu_num;
        while ppu_frac >= ppu_den {
            ppu_frac -= ppu_den;
            ppu.tick(mapper, &mut fb);
        }
    }
}
```

This is an **instruction-level** synchronization approach — after each CPU instruction completes, the APU and PPU are caught up to the same point in time. This is accurate enough for most games while avoiding the overhead of cycle-exact interleaving.

## CPU-PPU synchronization

The PPU runs faster than the CPU — 3 dots per CPU cycle on NTSC, 3.2 on PAL. The non-integer PAL ratio is handled with a Bresenham accumulator:

```mermaid
flowchart LR
    subgraph "Per CPU cycle (PAL)"
        A["ppu_frac += 16"] --> B{"ppu_frac >= 5?"}
        B -->|Yes| C["PPU tick, ppu_frac -= 5"]
        C --> B
        B -->|No| D[Next CPU cycle]
    end
```

Over 5 CPU cycles, this produces exactly 16 PPU ticks — 3 ticks on some cycles and 4 on others, distributed evenly.

## Interrupt flow

Interrupts are checked at the start of each CPU step, before the instruction executes:

```mermaid
flowchart TD
    A[CPU step] --> B{NMI pending?}
    B -->|Yes| C[Push PC + status, load NMI vector]
    B -->|No| D{IRQ pending AND I flag clear?}
    D -->|Yes| E[Push PC + status, load IRQ vector]
    D -->|No| F[Fetch and execute instruction]
    C --> G[7 cycles consumed]
    E --> G
    F --> G
```

NMI sources:
- PPU VBlank (scanline 241, cycle 1) when NMI is enabled in PPUCTRL

IRQ sources:
- APU frame counter (4-step mode, step 3)
- APU DMC end-of-sample
- Mapper scanline counter (MMC3)

## Double buffering

The PPU renders into a **back buffer**. When a frame is complete (the PPU wraps from the pre-render scanline back to scanline 0), the back buffer and front buffer are swapped:

```rust
TickOutput::FrameReady => {
    std::mem::swap(&mut self.fb, &mut self.fb_front);
    self.frame_ready = true;
}
```

The frontend always reads from `fb_front`, which holds the last complete frame. This prevents tearing from displaying a partially-rendered frame.

## Rewind

After each frame, the frontend calls `emu.snapshot()` to capture the full emulator state (CPU, PPU, APU, Bus, framebuffer) and pushes it into a `VecDeque<Snapshot>` ring buffer with a capacity of 600 entries (~10 seconds at 60 fps).

When the user holds R, the frontend pops one snapshot per frame from the back of the buffer and calls `emu.restore()`. This replays the game in reverse. When R is released, the emulator resumes forward from the restored state.

### Snapshot cost

Snapshots are cheap to clone because PRG-ROM (the largest component, up to 512 KB) is stored as `Arc<[u8]>` — cloning it is just a reference count bump. The mutable state (2 KB RAM, 2 KB VRAM, OAM, registers, APU channels) totals ~5 KB. The framebuffer adds 245 KB. Total: ~250 KB per snapshot, or ~150 MB for the full 600-entry buffer.

## Fast forward

Holding Tab multiplies the wall-clock delta by 4× before passing it to `update()`. The emulator runs 4× the normal CPU cycles per frame, producing 4 frames of PPU output (only the last is displayed). Audio is produced at 4× rate and overflows the ring buffer, which is expected.
