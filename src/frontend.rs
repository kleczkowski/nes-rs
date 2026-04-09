//! Raylib-based frontend for the NES emulator.

pub(crate) mod audio;
mod config;
mod filebrowser;
mod input;
mod video;

use std::path::Path;

use anyhow::Context;
use raylib::consts::ConfigFlags;
use raylib::ffi;
use raylib::prelude::*;

use config::Config;

use crate::nes::{self, Emulator, Region};

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
fn init_texture(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
) -> anyhow::Result<Texture2D> {
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

/// Tracks settings that need to be synced between the config panel
/// and the system (raylib / emulator).
struct App {
    config: Config,
    prev_time: f64,
    applied_fps: i32,
    applied_vsync: bool,
    prev_region: Region,
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
        }
    }

    /// Applies initial settings and records the baseline for
    /// change-detection in [`sync_settings`](Self::sync_settings).
    fn apply_initial_settings(
        &mut self,
        rl: &mut RaylibHandle,
        audio_stream: &AudioStream<'_>,
    ) {
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

    /// Handles hotkeys: F1 (config panel), F3 (file picker).
    ///
    /// Also processes drag-and-drop ROM loading.
    fn handle_input(&mut self, rl: &mut RaylibHandle, emu: &mut impl Emulator) {
        if rl.is_key_pressed(KeyboardKey::KEY_F1) {
            self.config.toggle();
        }

        if rl.is_key_pressed(KeyboardKey::KEY_F3) {
            if let Some(path) = filebrowser::pick_rom() {
                load_rom_from_path(emu, &path);
            }
            // Reset timing so the emulator doesn't try to catch up
            // for the time spent in the dialog.
            self.prev_time = rl.get_time();
        }

        if rl.is_file_dropped() {
            let dropped = rl.load_dropped_files();
            if let Some(path) = dropped.paths().first() {
                load_rom_from_path(emu, Path::new(path));
            }
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
        let dest = video::scale_dest(self.config.scale_mode, win_w, win_h);
        draw.draw_texture_pro(texture, src, dest, Vector2::zero(), 0.0, Color::WHITE);

        self.config.draw(&mut draw);

        if !self.config.is_visible() {
            draw.draw_fps(4, 4);
        }

        Ok(())
    }
}

// ── Entry point ─────────────────────────────────────────────────

/// Opens a window and runs the emulator loop until the user closes it.
pub(crate) fn run(
    emu: &mut impl Emulator,
    region_override: Option<Region>,
) -> anyhow::Result<()> {
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

        let buttons = app.config.poll_buttons(&rl);
        emu.set_buttons(0, buttons);

        if !app.config.is_visible() {
            emu.update(dt_ms.min(33.0));
        }

        app.render(&mut rl, &thread, &mut texture, emu)?;
    }

    Ok(())
}
