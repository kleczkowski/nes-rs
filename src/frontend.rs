//! Raylib-based frontend for the NES emulator.

pub(crate) mod audio;
mod config;
mod filebrowser;
mod input;

use anyhow::Context;
use raylib::consts::ConfigFlags;
use raylib::ffi;
use raylib::prelude::*;

use crate::nes::{self, Emulator};

/// NES screen dimensions as i32 for raylib API compatibility.
const SCREEN_W: i32 = nes::SCREEN_WIDTH as i32;
const SCREEN_H: i32 = nes::SCREEN_HEIGHT as i32;

/// Window scale factor applied to the NES native resolution.
const SCALE: i32 = 3;
const SCALE_F: f32 = SCALE as f32;

/// Opens a window and runs the emulator loop until the user closes it.
#[allow(clippy::too_many_lines)]
pub(crate) fn run(emu: &mut impl Emulator) -> anyhow::Result<()> {
    tracing::info!(
        width = SCREEN_W * SCALE,
        height = SCREEN_H * SCALE,
        "opening window",
    );
    let (mut rl, thread) = init()
        .size(SCREEN_W * SCALE, SCREEN_H * SCALE)
        .title("nes-rs")
        .build();

    // Video setup.
    let img = Image::gen_image_color(SCREEN_W, SCREEN_H, Color::BLACK);
    let mut texture = rl
        .load_texture_from_image(&thread, &img)
        .context("failed to create screen texture")?;
    texture.set_texture_filter(&thread, TextureFilter::TEXTURE_FILTER_POINT);

    // Audio setup — callback model.
    let rl_audio = RaylibAudio::init_audio_device()
        .map_err(|e| anyhow::anyhow!("failed to initialize audio: {e}"))?;
    let audio_stream = audio::init_audio_stream(&rl_audio);

    let mut config = config::Config::new();
    let mut file_browser = filebrowser::FileBrowser::new();
    let mut prev_time = rl.get_time();

    // Apply initial settings.
    rl.set_target_fps(config.target_fps as u32);
    if config.vsync {
        #[allow(unsafe_code)]
        unsafe {
            ffi::SetWindowState(ConfigFlags::FLAG_VSYNC_HINT as u32);
        };
    }
    audio_stream.set_volume(config.volume / 100.0);

    let mut applied_fps: i32 = config.target_fps;
    let mut applied_vsync: bool = config.vsync;

    while !rl.window_should_close() {
        // ── Apply settings when config panel closes ──────────
        if !config.is_visible() {
            if config.target_fps != applied_fps {
                applied_fps = config.target_fps;
                tracing::debug!(fps = applied_fps, "target FPS changed");
                rl.set_target_fps(applied_fps as u32);
            }
            if config.vsync != applied_vsync {
                applied_vsync = config.vsync;
                tracing::debug!(vsync = applied_vsync, "V-Sync changed");
                if applied_vsync {
                    #[allow(unsafe_code)]
                    unsafe {
                        ffi::SetWindowState(ConfigFlags::FLAG_VSYNC_HINT as u32);
                    };
                } else {
                    #[allow(unsafe_code)]
                    unsafe {
                        ffi::ClearWindowState(ConfigFlags::FLAG_VSYNC_HINT as u32);
                    };
                }
            }
            audio_stream.set_volume(config.volume / 100.0);
            emu.set_sprite_limit(!config.no_sprite_limit);
        }

        // ── Timing ───────────────────────────────────────────
        let now = rl.get_time();
        let dt_ms = (now - prev_time) * 1000.0;
        prev_time = now;

        // ── Input ────────────────────────────────────────────
        if rl.is_key_pressed(KeyboardKey::KEY_F1) {
            config.toggle();
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F3) {
            file_browser.toggle();
        }

        // ── ROM loading ──────────────────────────────────────
        if let Some(path) = file_browser.take_picked() {
            match std::fs::read(&path) {
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
        if rl.is_file_dropped() {
            let dropped = rl.load_dropped_files();
            if let Some(&path) = dropped.paths().first() {
                match std::fs::read(path) {
                    Ok(data) => {
                        if let Err(e) = emu.load_rom(&data) {
                            tracing::error!(path = %path, error = %e, "failed to load dropped ROM");
                        }
                    }
                    Err(e) => {
                        tracing::error!(path = %path, error = %e, "failed to read dropped file");
                    }
                }
            }
        }

        let buttons = config.poll_buttons(&rl);
        emu.set_buttons(0, buttons);

        // ── Emulation ────────────────────────────────────────
        if !config.is_visible() && !file_browser.is_visible() {
            let clamped = dt_ms.min(33.0);
            emu.update(clamped);
        }

        // ── Render ───────────────────────────────────────────
        texture
            .update_texture(emu.framebuffer().as_bytes())
            .context("failed to update screen texture")?;

        let mut draw = rl.begin_drawing(&thread);
        draw.clear_background(Color::BLACK);
        draw.draw_texture_ex(&texture, Vector2::new(0.0, 0.0), 0.0, SCALE_F, Color::WHITE);

        config.draw(&mut draw);
        file_browser.draw(&mut draw);

        if !config.is_visible() && !file_browser.is_visible() {
            draw.draw_fps(4, 4);
        }
    }

    Ok(())
}
