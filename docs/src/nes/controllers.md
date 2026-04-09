# Controllers

The NES supports two standard controllers, each with 8 buttons: **A**, **B**, **Select**, **Start**, **Up**, **Down**, **Left**, and **Right**.

## Hardware interface

Each controller contains an 8-bit parallel-in, serial-out **shift register**. The CPU reads buttons one at a time through a serial protocol:

### Strobe sequence

1. **Write 1 to `$4016`** — Enable strobe mode. The controller continuously latches the current button state.
2. **Write 0 to `$4016`** — Disable strobe. The button state is frozen in the shift register.
3. **Read `$4016` (controller 1) or `$4017` (controller 2)** — Returns the next button in the sequence, one per read.

### Read order

Buttons are read in this fixed order, one bit per read:

| Read # | Button |
|--------|--------|
| 1 | A |
| 2 | B |
| 3 | Select |
| 4 | Start |
| 5 | Up |
| 6 | Down |
| 7 | Left |
| 8 | Right |

After all 8 buttons have been read, subsequent reads return 1 (on real hardware, this is the behavior of the open data lines).

### Typical game code

```asm
; Latch controller state
LDA #$01
STA $4016       ; strobe on
LDA #$00
STA $4016       ; strobe off — buttons frozen

; Read each button
LDA $4016       ; A button (bit 0)
LDA $4016       ; B button (bit 0)
LDA $4016       ; Select  (bit 0)
; ... and so on for all 8 buttons
```

## Implementation in nes-rs

The controller is modeled as a `Controller` struct with:
- A `buttons` field (updated by the frontend each frame).
- A `latch` byte (snapshot of buttons taken on strobe).
- A `shift_index` counter (0–7, incremented on each read).
- A `strobe` flag.

```rust
pub(super) fn read(&mut self) -> u8 {
    if self.strobe {
        return self.buttons.bits() & 1;
    }
    let bit = if self.shift_index < 8 {
        (self.latch >> self.shift_index) & 1
    } else {
        1
    };
    self.shift_index = self.shift_index.saturating_add(1);
    bit
}
```

The button state is represented as bitflags matching the hardware read order:

```rust
bitflags! {
    pub(crate) struct Buttons: u8 {
        const A      = 1 << 0;
        const B      = 1 << 1;
        const SELECT = 1 << 2;
        const START  = 1 << 3;
        const UP     = 1 << 4;
        const DOWN   = 1 << 5;
        const LEFT   = 1 << 6;
        const RIGHT  = 1 << 7;
    }
}
```

The frontend polls keyboard and gamepad input each frame and calls `emu.set_buttons(player, buttons)` to update the controller state.
