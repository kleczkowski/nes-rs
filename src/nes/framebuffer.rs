//! RGBA framebuffer for the NES 256×240 display.

/// NES screen width in pixels.
pub(crate) const WIDTH: usize = 256;

/// NES screen height in pixels.
pub(crate) const HEIGHT: usize = 240;

const BYTES_PER_PIXEL: usize = 4; // RGBA

/// RGBA framebuffer representing the NES screen.
///
/// Pixels are stored in row-major order as `[R, G, B, A]` tuples,
/// matching the layout expected by GPU texture uploads.
#[derive(Clone)]
pub(crate) struct Framebuffer {
    pixels: Vec<u8>,
}

impl Framebuffer {
    /// Creates a new black framebuffer.
    pub(crate) fn new() -> Self {
        Self {
            pixels: vec![0; WIDTH * HEIGHT * BYTES_PER_PIXEL],
        }
    }

    /// Sets a single pixel to the given RGB color (alpha is always 255).
    ///
    /// Coordinates outside the screen bounds are silently ignored.
    pub(crate) fn set_pixel(&mut self, x: usize, y: usize, rgb: [u8; 3]) {
        if x >= WIDTH || y >= HEIGHT {
            return;
        }
        let offset = (y * WIDTH + x) * BYTES_PER_PIXEL;
        if let Some(pixel) = self.pixels.get_mut(offset..offset + BYTES_PER_PIXEL) {
            pixel.copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }

    /// Returns the raw RGBA pixel data.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.pixels
    }

    /// Fills the entire framebuffer with black.
    #[allow(dead_code)]
    pub(crate) fn clear(&mut self) {
        self.pixels.fill(0);
    }
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}
