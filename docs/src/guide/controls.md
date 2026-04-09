# Controls & Configuration

## Hotkeys

| Key | Action |
|-----|--------|
| **F1** | Toggle the configuration panel |
| **F3** | Open the file browser to load a ROM |
| **F5** | Soft reset (re-reads the reset vector without reloading the cartridge) |
| **F11** | Toggle fullscreen |
| **P** | Pause / resume emulation |
| **M** | Mute / unmute audio |
| **R** (hold) | Rewind — walks backward through the last ~10 seconds of gameplay |
| **Tab** (hold) | Fast forward at 4× speed |

## Default controller mapping

The NES controller has 8 buttons. The default keyboard mapping is:

| NES Button | Keyboard Key |
|------------|-------------|
| **A** | Z |
| **B** | X |
| **Select** | Backspace |
| **Start** | Enter |
| **Up** | Arrow Up |
| **Down** | Arrow Down |
| **Left** | Arrow Left |
| **Right** | Arrow Right |

Gamepads are also supported via raylib's built-in joystick handling. When a gamepad is connected, its buttons and D-pad are automatically mapped to the NES controller.

## Configuration panel

Press **F1** to open the in-app configuration panel. It has two tabs:

### Controls tab

Allows you to rebind each NES button to a different keyboard key or gamepad button. Click a button binding and press the desired key to reassign it.

### Audio / Video tab

| Setting | Description | Default |
|---------|-------------|---------|
| **Volume** | Master volume slider (0–100). | 100 |
| **Target FPS** | Frame rate cap. 0 = uncapped. | 60 (NTSC) or 50 (PAL) |
| **V-Sync** | Synchronize with the display refresh rate. | Off |
| **Sprite Limit** | Toggle the hardware 8-sprite-per-scanline limit. Disabling it removes sprite flickering but is not accurate to real hardware. | On |
| **Scale Mode** | How the 256x240 framebuffer is mapped to the window. | Aspect Fit |
| **Scale** | Integer scale factor for Centered mode (1–10). Only visible when Centered is selected. | 3 |
| **Region** | Override the TV region (None = auto-detect from ROM). | None |

### Scale modes

| Mode | Behavior |
|------|----------|
| **Centered** | Display at an integer multiple of 256x240, centered in the window. The scale factor is configurable (1–10). |
| **Aspect Fit** | Scale uniformly to fill the window while preserving the 256:240 aspect ratio. Black bars appear on the shorter axis. |
| **Stretch** | Stretch to fill the entire window, ignoring aspect ratio. |

Settings are applied when the configuration panel is closed, so you can adjust multiple values before they take effect.

## On-screen indicators

When the configuration panel is closed, the following are shown in the top-left corner:

- **FPS counter** — Always visible.
- **REWIND** (orange) — Shown while R is held and the rewind buffer is being consumed.
- **PAUSED** (yellow) — Shown when emulation is paused (P).
- **MUTED** (gray) — Shown when audio is muted (M).

## Rewind

Hold **R** to rewind gameplay. The emulator saves a snapshot of the full emulator state every frame into a ring buffer that holds approximately 10 seconds of history (600 frames at 60 fps). While R is held, the emulator walks backward through the buffer one frame at a time. Audio is muted during rewind.

When you release R, emulation resumes from the rewound position. Any snapshots that were "ahead" of the current position are discarded.

The rewind buffer is cleared when:
- A new ROM is loaded (F3 or drag-and-drop)
- The emulator is reset (F5)
