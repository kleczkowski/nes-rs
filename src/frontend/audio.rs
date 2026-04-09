//! Callback-based audio output using a lock-free ring buffer.
//!
//! The emulator writes samples into a fixed-size ring buffer from
//! the main thread.  Raylib's audio thread reads them via a
//! registered callback.  No mutex — just two atomic indices.

use std::sync::atomic::{AtomicUsize, Ordering};

use raylib::ffi;
use raylib::prelude::*;

use crate::nes;

/// Raylib sub-buffer size in frames.
const BUFFER_SIZE: i32 = 2048;

/// Ring buffer capacity (must be a power of two).
const RING_CAP: usize = 1 << 16; // 65 536 samples ≈ 1.49 s
const RING_MASK: usize = RING_CAP - 1;

/// Lock-free single-producer single-consumer ring buffer.
///
/// The data array is large (128 KB) but lives in a `static`, not on
/// the stack, so the clippy lint does not apply.
#[allow(clippy::large_stack_arrays)]
struct Ring {
    data: [i16; RING_CAP],
    /// Next position the producer will write to.
    write: AtomicUsize,
    /// Next position the consumer will read from.
    read: AtomicUsize,
}

impl Ring {
    #[allow(clippy::large_stack_arrays)]
    const fn new() -> Self {
        Self {
            data: [0i16; RING_CAP],
            write: AtomicUsize::new(0),
            read: AtomicUsize::new(0),
        }
    }

    /// Pushes a batch of samples (producer — main thread only).
    fn push(&self, samples: &[i16]) {
        let mut w = self.write.load(Ordering::Relaxed);
        for &s in samples {
            // Safety: only one producer, and RING_CAP is large
            // enough that we never lap the consumer.
            #[allow(unsafe_code)]
            unsafe {
                let ptr = self.data.as_ptr().add(w & RING_MASK).cast_mut();
                ptr.write(s);
            }
            w = w.wrapping_add(1);
        }
        self.write.store(w, Ordering::Release);
    }

    /// Pops up to `n` samples into `dst` (consumer — callback only).
    /// Returns the number of samples written.  Fills the rest of
    /// `dst` with `fill`.
    fn pop_into(&self, dst: &mut [i16], fill: i16) -> usize {
        let r = self.read.load(Ordering::Relaxed);
        let avail = self.write.load(Ordering::Acquire).wrapping_sub(r);
        let n = avail.min(dst.len());
        for i in 0..n {
            #[allow(unsafe_code)]
            unsafe {
                let ptr = self.data.as_ptr().add((r.wrapping_add(i)) & RING_MASK);
                *dst.get_unchecked_mut(i) = ptr.read();
            }
        }
        let last = if n > 0 {
            dst.get(n - 1).copied().unwrap_or(fill)
        } else {
            fill
        };
        if let Some(tail) = dst.get_mut(n..) {
            tail.fill(last);
        }
        self.read.store(r.wrapping_add(n), Ordering::Release);
        n
    }
}

// SAFETY: Ring is designed for exactly one producer + one consumer
// on different threads, synchronised via atomic write/read indices.
#[allow(unsafe_code)]
unsafe impl Sync for Ring {}

static RING: Ring = Ring::new();

/// Pushes a batch of audio samples (main thread).
pub(crate) fn queue_samples(samples: &[i16]) {
    RING.push(samples);
}

/// Raylib audio callback (audio thread).
#[allow(unsafe_code)]
extern "C" fn audio_callback(buffer: *mut std::ffi::c_void, frames: std::ffi::c_uint) {
    let buf: &mut [i16] =
        unsafe { std::slice::from_raw_parts_mut(buffer.cast::<i16>(), frames as usize) };
    let _ = RING.pop_into(buf, 0);
}

/// Creates a 44 100 Hz mono 16-bit stream with the pull callback.
pub(crate) fn init_audio_stream(audio: &RaylibAudio) -> AudioStream<'_> {
    audio.set_audio_stream_buffer_size_default(BUFFER_SIZE);
    let stream = audio.new_audio_stream(nes::SAMPLE_RATE, 16, 1);

    #[allow(unsafe_code)]
    unsafe {
        ffi::SetAudioStreamCallback(*stream.as_ref(), Some(audio_callback));
    }

    stream.play();
    stream
}
