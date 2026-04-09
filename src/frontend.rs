//! Raylib-based frontend for the NES emulator.

pub(crate) mod audio;
mod config;
mod filebrowser;
mod input;
mod video;

use std::collections::VecDeque;
use std::path::Path;

use anyhow::Context;
use raylib::consts::ConfigFlags;
use raylib::ffi;
use raylib::prelude::*;

use config::Config;

use crate::nes::{self, Emulator, Region, Snapshot};

/// NES screen dimensions as i32 for raylib API compatibility.
const SCREEN_W: i32 = nes::SCREEN_WIDTH as i32;
const SCREEN_H: i32 = nes::SCREEN_HEIGHT as i32;

/// Default window scale factor applied to the NES native resolution.
const INITIAL_SCALE: i32 = 3;

// ── Initialization helpers ──────────────────────────────────────

/// Creates the main window with default size and flags.
fn init_window() -> (RaylibHandle, RaylibThread) {
    tracing::info!(
        width = SCREEN_W * INITIAL_SCALE,
        height = SCREEN_H * INITIAL_SCALE,
        "opening window",
    );
    let (rl, thread) = init()
        .size(SCREEN_W * INITIAL_SCALE, SCREEN_H * INITIAL_SCALE)
        .title("nes-rs")
        .resizable()
        .build();
    (rl, thread)
}

/// Creates the screen texture (256x240, nearest-neighbor filtering).
fn init_texture(rl: &mut RaylibHandle, thread: &RaylibThread) -> anyhow::Result<Texture2D> {
    let img = Image::gen_image_color(SCREEN_W, SCREEN_H, Color::BLACK);
    let texture = rl
        .load_texture_from_image(thread, &img)
        .context("failed to create screen texture")?;
    texture.set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_POINT);
    Ok(texture)
}

// ── ROM loading helper ──────────────────────────────────────────

/// Reads a ROM file from disk and loads it into the emulator.
fn load_rom_from_path(emu: &mut impl Emulator, path: &Path) {
    match std::fs::read(path) {
        Ok(data) => {
            if let Err(e) = emu.load_rom(&data) {
                tracing::error!(path = %path.display(), error = %e, "failed to load ROM");
            }
        }
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "failed to read ROM file");
        }
    }
}

// ── App state ───────────────────────────────────────────────────

/// Speed multiplier when fast-forwarding (hold Tab).
const FAST_FORWARD_MULT: f64 = 4.0;

/// Maximum number of snapshots kept for rewind (~10 s at 60 fps).
const REWIND_CAPACITY: usize = 600;

/// Tracks settings that need to be synced between the config panel
/// and the system (raylib / emulator).
#[allow(clippy::struct_excessive_bools)]
struct App {
    config: Config,
    prev_time: f64,
    applied_fps: i32,
    applied_vsync: bool,
    prev_region: Region,
    paused: bool,
    muted: bool,
    /// Rewind snapshot ring buffer (oldest at front).
    rewind_buf: VecDeque<Snapshot>,
    /// Whether we are currently rewinding (R held).
    rewinding: bool,
}

impl App {
    fn new(initial_region: Region, region_override: Option<Region>) -> Self {
        let mut config = Config::new();
        config.region_override = region_override;
        Self {
            config,
            prev_time: 0.0,
            applied_fps: 0,
            applied_vsync: false,
            prev_region: initial_region,
            paused: false,
            muted: false,
            rewind_buf: VecDeque::with_capacity(REWIND_CAPACITY),
            rewinding: false,
        }
    }

    /// Applies initial settings and records the baseline for
    /// change-detection in [`sync_settings`](Self::sync_settings).
    fn apply_initial_settings(&mut self, rl: &mut RaylibHandle, audio_stream: &AudioStream<'_>) {
        rl.set_target_fps(self.config.target_fps as u32);
        if self.config.vsync {
            #[allow(unsafe_code)]
            unsafe {
                ffi::SetWindowState(ConfigFlags::FLAG_VSYNC_HINT as u32);
            };
        }
        audio_stream.set_volume(self.config.volume / 100.0);

        self.applied_fps = self.config.target_fps;
        self.applied_vsync = self.config.vsync;
        self.prev_time = rl.get_time();
    }

    /// Pushes changed config values to raylib and the emulator.
    ///
    /// Only acts when the config panel is closed so that edits
    /// are applied atomically.
    fn sync_settings(
        &mut self,
        rl: &mut RaylibHandle,
        audio_stream: &AudioStream<'_>,
        emu: &mut impl Emulator,
    ) {
        if self.config.is_visible() {
            return;
        }

        if self.config.target_fps != self.applied_fps {
            self.applied_fps = self.config.target_fps;
            tracing::debug!(fps = self.applied_fps, "target FPS changed");
            rl.set_target_fps(self.applied_fps as u32);
        }

        if self.config.vsync != self.applied_vsync {
            self.applied_vsync = self.config.vsync;
            tracing::debug!(vsync = self.applied_vsync, "V-Sync changed");
            #[allow(unsafe_code)]
            if self.applied_vsync {
                unsafe { ffi::SetWindowState(ConfigFlags::FLAG_VSYNC_HINT as u32) };
            } else {
                unsafe { ffi::ClearWindowState(ConfigFlags::FLAG_VSYNC_HINT as u32) };
            }
        }

        audio_stream.set_volume(self.config.volume / 100.0);
        emu.set_sprite_limit(!self.config.no_sprite_limit);
        emu.set_region_override(self.config.region_override);
    }

    /// Returns wall-clock delta since the last call, in milliseconds.
    fn advance_timing(&mut self, rl: &RaylibHandle) -> f64 {
        let now = rl.get_time();
        let dt_ms = (now - self.prev_time) * 1000.0;
        self.prev_time = now;
        dt_ms
    }

    /// Handles hotkeys and drag-and-drop ROM loading.
    fn handle_input(&mut self, rl: &mut RaylibHandle, emu: &mut impl Emulator) {
        if rl.is_key_pressed(KeyboardKey::KEY_F1) {
            self.config.toggle();
        }

        if rl.is_key_pressed(KeyboardKey::KEY_F3) {
            if let Some(path) = filebrowser::pick_rom() {
                load_rom_from_path(emu, &path);
                self.paused = false;
                self.rewind_buf.clear();
            }
            // Reset timing so the emulator doesn't try to catch up
            // for the time spent in the dialog.
            self.prev_time = rl.get_time();
        }

        if rl.is_key_pressed(KeyboardKey::KEY_F5) {
            emu.reset();
            self.paused = false;
            self.rewind_buf.clear();
        }

        if rl.is_key_pressed(KeyboardKey::KEY_F11) {
            #[allow(unsafe_code)]
            if rl.is_window_fullscreen() {
                rl.toggle_fullscreen();
                rl.set_window_size(SCREEN_W * INITIAL_SCALE, SCREEN_H * INITIAL_SCALE);
            } else {
                let monitor = unsafe { ffi::GetCurrentMonitor() };
                let mw = unsafe { ffi::GetMonitorWidth(monitor) };
                let mh = unsafe { ffi::GetMonitorHeight(monitor) };
                rl.set_window_size(mw, mh);
                rl.toggle_fullscreen();
            }
            // Recreate timing baseline after mode switch.
            self.prev_time = rl.get_time();
        }

        if rl.is_key_pressed(KeyboardKey::KEY_P) && !self.config.is_visible() {
            self.paused = !self.paused;
            tracing::info!(paused = self.paused, "pause toggled");
        }

        if rl.is_key_pressed(KeyboardKey::KEY_M) {
            self.muted = !self.muted;
            tracing::info!(muted = self.muted, "mute toggled");
        }

        if rl.is_file_dropped() {
            let dropped = rl.load_dropped_files();
            if let Some(path) = dropped.paths().first() {
                load_rom_from_path(emu, Path::new(path));
                self.paused = false;
                self.rewind_buf.clear();
            }
        }
    }

    /// Returns `true` if emulation should be running this frame.
    fn should_run(&self) -> bool {
        !self.paused && !self.config.is_visible()
    }

    /// Returns the effective dt, accounting for fast-forward.
    fn effective_dt(&self, rl: &RaylibHandle, dt_ms: f64) -> f64 {
        let dt = dt_ms.min(33.0);
        if rl.is_key_down(KeyboardKey::KEY_TAB) && self.should_run() {
            dt * FAST_FORWARD_MULT
        } else {
            dt
        }
    }

    /// Auto-adjusts target FPS when the emulator's region changes
    /// (e.g. a PAL ROM was loaded).
    fn sync_region(&mut self, rl: &mut RaylibHandle, emu: &impl Emulator) {
        let current = emu.region();
        if current == self.prev_region {
            return;
        }
        let fps = current.fps();
        self.config.target_fps = fps;
        self.applied_fps = fps;
        rl.set_target_fps(fps as u32);
        tracing::info!(region = %current, fps, "adjusted target FPS for region");
        self.prev_region = current;
    }

    /// Updates the texture from the emulator framebuffer and draws
    /// the frame with the current scale mode.
    fn render(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        texture: &mut Texture2D,
        emu: &impl Emulator,
    ) -> anyhow::Result<()> {
        texture
            .update_texture(emu.framebuffer().as_bytes())
            .context("failed to update screen texture")?;

        let mut draw = rl.begin_drawing(thread);
        draw.clear_background(Color::BLACK);

        let win_w = draw.get_screen_width() as f32;
        let win_h = draw.get_screen_height() as f32;
        let src = video::framebuffer_src();
        let dest =
            video::scale_dest(self.config.scale_mode, win_w, win_h, self.config.centered_scale);
        draw.draw_texture_pro(texture, src, dest, Vector2::zero(), 0.0, Color::WHITE);

        self.config.draw(&mut draw);

        if !self.config.is_visible() {
            draw.draw_fps(4, 4);
            let mut label_y = 24;
            if self.rewinding {
                draw.draw_text("◀◀ REWIND", 4, label_y, 20, Color::ORANGE);
                label_y += 22;
            }
            if self.paused {
                draw.draw_text("PAUSED", 4, label_y, 20, Color::YELLOW);
                label_y += 22;
            }
            if self.muted {
                draw.draw_text("MUTED", 4, label_y, 20, Color::GRAY);
            }
        }

        Ok(())
    }
}

// ── Entry point ─────────────────────────────────────────────────

/// Opens a window and runs the emulator loop until the user closes it.
pub(crate) fn run(emu: &mut impl Emulator, region_override: Option<Region>) -> anyhow::Result<()> {
    let (mut rl, thread) = init_window();
    rl.set_window_min_size(SCREEN_W, SCREEN_H);
    let mut texture = init_texture(&mut rl, &thread)?;

    let rl_audio = RaylibAudio::init_audio_device()
        .map_err(|e| anyhow::anyhow!("failed to initialize audio: {e}"))?;
    let audio_stream = audio::init_audio_stream(&rl_audio);

    let mut app = App::new(emu.region(), region_override);
    app.apply_initial_settings(&mut rl, &audio_stream);

    while !rl.window_should_close() {
        app.sync_settings(&mut rl, &audio_stream, emu);
        let dt_ms = app.advance_timing(&rl);
        app.handle_input(&mut rl, emu);
        app.sync_region(&mut rl, emu);

        // Apply mute (also mute during rewind).
        let volume = if app.muted || app.rewinding {
            0.0
        } else {
            app.config.volume / 100.0
        };
        audio_stream.set_volume(volume);

        // Rewind logic.
        let rewinding_now = rl.is_key_down(KeyboardKey::KEY_R)
            && !app.config.is_visible()
            && !app.rewind_buf.is_empty();

        if rewinding_now {
            // Pop one snapshot per frame and restore it.
            if let Some(snap) = app.rewind_buf.pop_back() {
                emu.restore(&snap);
            }
            app.rewinding = true;
        } else {
            if app.rewinding {
                // Just released R — resume from current state.
                app.rewinding = false;
                app.prev_time = rl.get_time();
            }

            let buttons = app.config.poll_buttons(&rl);
            emu.set_buttons(0, buttons);

            if app.should_run() {
                let dt = app.effective_dt(&rl, dt_ms);
                emu.update(dt);

                // Save a snapshot after each frame (ring buffer).
                if let Some(snap) = emu.snapshot() {
                    if app.rewind_buf.len() >= REWIND_CAPACITY {
                        let _ = app.rewind_buf.pop_front();
                    }
                    app.rewind_buf.push_back(snap);
                }
            }
        }

        app.render(&mut rl, &thread, &mut texture, emu)?;
    }

    Ok(())
}
