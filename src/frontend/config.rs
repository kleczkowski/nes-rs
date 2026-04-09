//! Unified configuration panel (F1) — controls, audio, video.

use raylib::prelude::*;

use super::input::{Controller, Gamepad, Keyboard};
use super::video::ScaleMode;
use crate::nes::Buttons;

/// All user-facing settings in one panel, toggled by F1.
#[allow(clippy::struct_excessive_bools)]
pub(super) struct Config {
    pub(super) keyboard: Keyboard,
    pub(super) gamepad: Gamepad,

    // ── Audio/Video settings ─────────────────────────────────
    /// Master volume 0–100.
    pub(super) volume: f32,
    /// Target FPS (0 = uncapped).
    pub(super) target_fps: i32,
    /// V-Sync enabled.
    pub(super) vsync: bool,
    /// Remove the 8-sprite-per-scanline limit.
    pub(super) no_sprite_limit: bool,
    /// How the framebuffer is scaled to the window.
    pub(super) scale_mode: ScaleMode,

    visible: bool,
    fps_edit: bool,
    /// Which tab is active: 0 = Controls, 1 = Audio/Video.
    tab: i32,
}

impl Config {
    pub(super) fn new() -> Self {
        Self {
            keyboard: Keyboard::new(),
            gamepad: Gamepad::new(0),
            volume: 80.0,
            target_fps: 60,
            vsync: true,
            no_sprite_limit: false,
            scale_mode: ScaleMode::AspectFit,
            visible: false,
            fps_edit: false,
            tab: 0,
        }
    }

    pub(super) fn toggle(&mut self) {
        self.visible = !self.visible;
        self.fps_edit = false;
        self.keyboard.close();
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    /// Polls keyboard + gamepad together.
    pub(super) fn poll_buttons(&self, rl: &RaylibHandle) -> Buttons {
        if self.visible {
            return Buttons::empty();
        }
        self.keyboard.poll(rl) | self.gamepad.poll(rl)
    }

    /// Draws the unified config panel.
    pub(super) fn draw(&mut self, draw: &mut RaylibDrawHandle<'_>) {
        if !self.visible {
            return;
        }

        let px = 10;
        let py = 10;
        let pw = 400;
        let ph = 480;
        let pad = 8;

        // Background.
        draw.draw_rectangle(px, py, pw, ph, Color::new(20, 20, 20, 240));
        draw.draw_rectangle_lines(px, py, pw, ph, Color::RAYWHITE);

        // Tab bar.
        let _ = draw.gui_toggle_group(
            Rectangle::new((px + pad) as f32, (py + pad) as f32, 180.0, 24.0),
            "Controls;Audio/Video",
            &mut self.tab,
        );

        let content_y = py + 42;

        match self.tab {
            0 => self.draw_controls_tab(draw, px, content_y, pw, pad),
            1 => self.draw_av_tab(draw, px, content_y, pw, pad),
            _ => {}
        }
    }

    fn draw_controls_tab(
        &mut self,
        draw: &mut RaylibDrawHandle<'_>,
        px: i32,
        cy: i32,
        pw: i32,
        pad: i32,
    ) {
        // ── Keyboard bindings ────────────────────────────────
        let _ = draw.gui_label(
            Rectangle::new((px + pad) as f32, cy as f32, 200.0, 20.0),
            "Keyboard",
        );

        self.keyboard
            .draw_bindings(draw, px + pad, cy + 24, pw - pad * 2);

        // ── Gamepad info ─────────────────────────────────────
        let gy = cy + 24 + 8 * 26 + 12;
        self.gamepad.draw_info(draw, px + pad, gy, pw - pad * 2);
    }

    fn draw_av_tab(
        &mut self,
        draw: &mut RaylibDrawHandle<'_>,
        px: i32,
        cy: i32,
        _pw: i32,
        pad: i32,
    ) {
        let row = 32;

        // Volume.
        let vy = cy as f32;
        let _ = draw.gui_slider(
            Rectangle::new((px + pad + 70) as f32, vy, 180.0, 20.0),
            "Volume",
            &format!("{}%", self.volume as i32),
            &mut self.volume,
            0.0,
            100.0,
        );

        // FPS.
        let fy = vy + row as f32;
        if draw.gui_spinner(
            Rectangle::new((px + pad + 70) as f32, fy, 120.0, 20.0),
            "FPS",
            &mut self.target_fps,
            0,
            300,
            self.fps_edit,
        ) {
            self.fps_edit = !self.fps_edit;
        }
        let _ = draw.gui_label(
            Rectangle::new((px + pad + 200) as f32, fy, 90.0, 20.0),
            if self.target_fps == 0 {
                "(uncapped)"
            } else {
                ""
            },
        );

        // V-Sync.
        let vy2 = fy + row as f32;
        let _ = draw.gui_check_box(
            Rectangle::new((px + pad) as f32, vy2, 20.0, 20.0),
            "V-Sync",
            &mut self.vsync,
        );

        // Sprite limit.
        let sy = vy2 + row as f32;
        let _ = draw.gui_check_box(
            Rectangle::new((px + pad) as f32, sy, 20.0, 20.0),
            "No sprite limit",
            &mut self.no_sprite_limit,
        );

        // Scale mode.
        let smy = sy + row as f32;
        let _ = draw.gui_label(
            Rectangle::new((px + pad) as f32, smy, 70.0, 20.0),
            "Scaling",
        );
        let mut idx = self.scale_mode.to_index();
        let _ = draw.gui_toggle_group(
            Rectangle::new(
                (px + pad + 70) as f32,
                smy,
                100.0,
                20.0,
            ),
            ScaleMode::LABELS,
            &mut idx,
        );
        self.scale_mode = ScaleMode::from_index(idx.clamp(0, ScaleMode::COUNT - 1));
    }
}
