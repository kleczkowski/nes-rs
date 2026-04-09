//! Keyboard and gamepad input with configurable bindings.

use raylib::consts::{GamepadAxis, GamepadButton};
use raylib::prelude::*;

use crate::nes::Buttons;

// ── Controller trait ────────────────────────────────────────────

/// Polls a physical input device and returns NES button state.
pub(super) trait Controller {
    /// Returns the NES buttons currently held down.
    fn poll(&self, rl: &RaylibHandle) -> Buttons;
}

// ── Keyboard ────────────────────────────────────────────────────

/// Names of NES buttons in display order.
const BUTTON_NAMES: [&str; 8] = ["A", "B", "Select", "Start", "Up", "Down", "Left", "Right"];

/// NES buttons in the same order as [`BUTTON_NAMES`].
const BUTTON_FLAGS: [Buttons; 8] = [
    Buttons::A,
    Buttons::B,
    Buttons::SELECT,
    Buttons::START,
    Buttons::UP,
    Buttons::DOWN,
    Buttons::LEFT,
    Buttons::RIGHT,
];

/// Keyboard-to-NES-controller mapping with configurable bindings.
pub(super) struct Keyboard {
    /// Keyboard key assigned to each NES button (indexed by button order).
    keys: [KeyboardKey; 8],
    /// If `Some(i)`, button `i` is waiting for a new key press.
    listening: Option<usize>,
    /// Whether the config panel is visible.
    visible: bool,
}

impl Keyboard {
    pub(super) fn new() -> Self {
        Self {
            keys: [
                KeyboardKey::KEY_Z,         // A
                KeyboardKey::KEY_X,         // B
                KeyboardKey::KEY_BACKSPACE, // Select
                KeyboardKey::KEY_ENTER,     // Start
                KeyboardKey::KEY_UP,        // Up
                KeyboardKey::KEY_DOWN,      // Down
                KeyboardKey::KEY_LEFT,      // Left
                KeyboardKey::KEY_RIGHT,     // Right
            ],
            listening: None,
            visible: false,
        }
    }

    pub(super) fn close(&mut self) {
        self.listening = None;
    }

    /// Draws keyboard binding rows inside a parent panel.
    /// Handles key capture for rebinding.
    pub(super) fn draw_bindings(
        &mut self,
        draw: &mut RaylibDrawHandle<'_>,
        x: i32,
        y: i32,
        _w: i32,
    ) {
        // Capture key press for active listener.
        if let Some(idx) = self.listening
            && let Some(key) = draw.get_key_pressed()
        {
            if key != KeyboardKey::KEY_ESCAPE
                && key != KeyboardKey::KEY_F1
                && let Some(slot) = self.keys.get_mut(idx)
            {
                *slot = key;
            }
            self.listening = None;
        }

        let row_h = 26;
        for (i, name) in BUTTON_NAMES.iter().enumerate() {
            let ry = y + (i as i32) * row_h;
            let _ = draw.gui_label(Rectangle::new(x as f32, ry as f32, 80.0, 22.0), name);
            let label = if self.listening == Some(i) {
                "[ Press a key... ]".to_owned()
            } else {
                key_display_name(self.keys.get(i).copied().unwrap_or(KeyboardKey::KEY_NULL))
            };
            if draw.gui_button(
                Rectangle::new((x + 90) as f32, ry as f32, 160.0, 22.0),
                &label,
            ) {
                self.listening = Some(i);
            }
        }
    }
}

impl Controller for Keyboard {
    fn poll(&self, rl: &RaylibHandle) -> Buttons {
        if self.visible {
            return Buttons::empty();
        }
        let mut buttons = Buttons::empty();
        for (i, &key) in self.keys.iter().enumerate() {
            if rl.is_key_down(key)
                && let Some(&flag) = BUTTON_FLAGS.get(i)
            {
                buttons |= flag;
            }
        }
        buttons
    }
}

// ── Gamepad ─────────────────────────────────────────────────────

/// Maps a generic USB gamepad (Xbox, PS, etc.) to NES buttons.
///
/// Layout:
/// - D-pad / left stick → Up/Down/Left/Right
/// - A (Xbox) / Cross (PS) → NES B
/// - B (Xbox) / Circle (PS) → NES A
/// - Start → Start
/// - Back/Select → Select
pub(super) struct Gamepad {
    /// Raylib gamepad index (0 = first controller).
    pad: i32,
}

/// Gamepad button → NES button display mapping.
const GAMEPAD_MAP: [(&str, &str); 8] = [
    ("A / Cross", "B"),
    ("B / Circle", "A"),
    ("Back / Share", "Select"),
    ("Start / Options", "Start"),
    ("D-pad Up / Stick", "Up"),
    ("D-pad Down / Stick", "Down"),
    ("D-pad Left / Stick", "Left"),
    ("D-pad Right / Stick", "Right"),
];

impl Gamepad {
    pub(super) fn new(pad: i32) -> Self {
        Self { pad }
    }

    /// Draws gamepad info + mapping inside a parent panel.
    pub(super) fn draw_info(&self, draw: &mut RaylibDrawHandle<'_>, x: i32, y: i32, _w: i32) {
        let status = if draw.is_gamepad_available(self.pad) {
            let name = draw
                .get_gamepad_name(self.pad)
                .unwrap_or_else(|| "Unknown".into());
            format!("Gamepad: {name}")
        } else {
            "Gamepad: not connected".into()
        };
        let _ = draw.gui_label(Rectangle::new(x as f32, y as f32, 300.0, 20.0), &status);

        let row_h = 22;
        for (i, (gamepad_btn, nes_btn)) in GAMEPAD_MAP.iter().enumerate() {
            let ry = y + 24 + (i as i32) * row_h;
            let _ = draw.gui_label(
                Rectangle::new(x as f32, ry as f32, 150.0, 20.0),
                gamepad_btn,
            );
            let _ = draw.gui_label(
                Rectangle::new((x + 160) as f32, ry as f32, 80.0, 20.0),
                &format!("→ {nes_btn}"),
            );
        }
    }
}

impl Controller for Gamepad {
    fn poll(&self, rl: &RaylibHandle) -> Buttons {
        if !rl.is_gamepad_available(self.pad) {
            return Buttons::empty();
        }
        let mut b = Buttons::empty();
        let pad = self.pad;

        // Face buttons
        if rl.is_gamepad_button_down(pad, GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN) {
            b |= Buttons::B;
        }
        if rl.is_gamepad_button_down(pad, GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT) {
            b |= Buttons::A;
        }
        if rl.is_gamepad_button_down(pad, GamepadButton::GAMEPAD_BUTTON_MIDDLE_RIGHT) {
            b |= Buttons::START;
        }
        if rl.is_gamepad_button_down(pad, GamepadButton::GAMEPAD_BUTTON_MIDDLE_LEFT) {
            b |= Buttons::SELECT;
        }

        // D-pad
        if rl.is_gamepad_button_down(pad, GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_UP) {
            b |= Buttons::UP;
        }
        if rl.is_gamepad_button_down(pad, GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_DOWN) {
            b |= Buttons::DOWN;
        }
        if rl.is_gamepad_button_down(pad, GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_LEFT) {
            b |= Buttons::LEFT;
        }
        if rl.is_gamepad_button_down(pad, GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_RIGHT) {
            b |= Buttons::RIGHT;
        }

        // Left analog stick (deadzone 0.5)
        let lx = rl.get_gamepad_axis_movement(pad, GamepadAxis::GAMEPAD_AXIS_LEFT_X);
        let ly = rl.get_gamepad_axis_movement(pad, GamepadAxis::GAMEPAD_AXIS_LEFT_Y);
        if lx < -0.5 {
            b |= Buttons::LEFT;
        }
        if lx > 0.5 {
            b |= Buttons::RIGHT;
        }
        if ly < -0.5 {
            b |= Buttons::UP;
        }
        if ly > 0.5 {
            b |= Buttons::DOWN;
        }

        b
    }
}

// ── Helpers ─────────────────────────────────────────────────────

fn key_display_name(key: KeyboardKey) -> String {
    let debug = format!("{key:?}");
    debug.strip_prefix("KEY_").unwrap_or(&debug).to_owned()
}
