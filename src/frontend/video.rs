//! Video scaling modes and framebuffer-to-window mapping.

use raylib::prelude::*;

use crate::nes;

/// NES native resolution as floats for scaling math.
const SCREEN_W: f32 = nes::SCREEN_WIDTH as f32;
const SCREEN_H: f32 = nes::SCREEN_HEIGHT as f32;

/// How the NES framebuffer is scaled to fill the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScaleMode {
    /// Native resolution centered in the window, no scaling.
    Centered,
    /// Uniform scaling that preserves the 256:240 aspect ratio.
    AspectFit,
    /// Stretches to fill the entire window.
    Stretch,
}

impl ScaleMode {
    /// Total number of modes (for the UI toggle group).
    pub(super) const COUNT: i32 = 3;

    /// Display labels for the toggle group (semicolon-separated).
    pub(super) const LABELS: &str = "Centered;Aspect Fit;Stretch";

    /// Converts a toggle group index to a mode.
    pub(super) fn from_index(i: i32) -> Self {
        match i {
            0 => Self::Centered,
            1 => Self::AspectFit,
            _ => Self::Stretch,
        }
    }

    /// Converts this mode to a toggle group index.
    pub(super) fn to_index(self) -> i32 {
        match self {
            Self::Centered => 0,
            Self::AspectFit => 1,
            Self::Stretch => 2,
        }
    }
}

/// Computes the destination rectangle for the NES framebuffer
/// given the current window size and scaling mode.
///
/// `centered_scale` is only used by [`ScaleMode::Centered`] and
/// multiplies the native resolution by an integer factor.
pub(super) fn scale_dest(mode: ScaleMode, win_w: f32, win_h: f32, centered_scale: i32) -> Rectangle {
    match mode {
        ScaleMode::Centered => {
            let scale = (centered_scale.max(1)) as f32;
            let scaled_w = SCREEN_W * scale;
            let scaled_h = SCREEN_H * scale;
            Rectangle::new(
                (win_w - scaled_w) / 2.0,
                (win_h - scaled_h) / 2.0,
                scaled_w,
                scaled_h,
            )
        }
        ScaleMode::AspectFit => {
            let scale = (win_w / SCREEN_W).min(win_h / SCREEN_H);
            let w = SCREEN_W * scale;
            let h = SCREEN_H * scale;
            let x = (win_w - w) / 2.0;
            let y = (win_h - h) / 2.0;
            Rectangle::new(x, y, w, h)
        }
        ScaleMode::Stretch => Rectangle::new(0.0, 0.0, win_w, win_h),
    }
}

/// Source rectangle covering the entire NES framebuffer.
pub(super) fn framebuffer_src() -> Rectangle {
    Rectangle::new(0.0, 0.0, SCREEN_W, SCREEN_H)
}
