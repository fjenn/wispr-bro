// SPDX-License-Identifier: MIT
// Copyright (c) 2026 handy-computer. handy-recorder 0.1.0, vendored by Wispr Bro (see Cargo.toml).
//! Cross-platform microphone capture.
//!
//! Handy Recorder owns the real-time audio path. It gives applications audio
//! in the format they asked for, on an ordinary thread, reporting when audio
//! was lost or the stream broke.
//!
//! # Concepts
//!
//! - A [`Recorder`] is an open microphone; `start` and `stop` bracket each
//!   recording.
//! - A [`Sink`] is application code that receives the recording's audio as
//!   fixed-size [`AudioChunk`]s. It is lent to the library by `start` and
//!   handed back by `stop`.
//! - Errors: match on [`ErrorKind`]; its docs say what to do for each.
//!
//! # Example
//!
//! ```no_run
//! use handy_recorder::{CollectingSink, Recorder, RecorderConfig};
//! use std::time::Duration;
//!
//! # fn main() -> Result<(), handy_recorder::Error> {
//! let recorder = Recorder::open_with_failure_handler(RecorderConfig::speech(), |error| {
//!     // Runs on a library thread. `stop` still returns the audio captured so far.
//!     eprintln!("microphone failed: {error}");
//! })?;
//!
//! recorder.start(CollectingSink::new())?;
//! std::thread::sleep(Duration::from_secs(5));
//!
//! let stopped = recorder.stop()?;
//! if !stopped.is_complete() {
//!     // The mic failed or audio was dropped. What was captured is still here.
//!     eprintln!("incomplete recording: {:?}", stopped.end_reason);
//! }
//! let samples = stopped.sink.into_samples();
//! # Ok(())
//! # }
//! ```

mod backend;
mod capture;
mod device;
mod error;
mod sink;
mod types;

#[cfg(feature = "test-backend")]
#[doc(hidden)]
pub mod testing;
#[cfg(test)]
mod tests;

pub use device::{InputDevice, Permission, list_input_devices, permission_status};
pub use error::{Error, ErrorKind};
pub use sink::{AudioChunk, CollectingSink, Sink};
pub use types::{Channels, EndReason, Format, RecorderConfig, RecorderInfo};

use std::fmt;

use std::sync::{Arc, Mutex};

use backend::Backend;
use backend::cpal::CpalBackend;
use backend::file::FileBackend;
use capture::engine::{Engine, Timeouts};

/// Mono samples a test microphone plays instead of the device
/// ([`set_file_source`]). Added by Wispr Bro (test only).
#[derive(Debug, Clone, PartialEq)]
pub struct FileSource {
    /// Mono samples in -1.0..=1.0.
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    /// Shown as the device name ("Test microphone (<label>)").
    pub label: String,
}

/// Returns the source for the next recording: called when a recorder opens
/// and again each time its stream starts, so the application may change
/// the file between recordings. `None` at open fails it like a missing
/// device; `None` later plays silence.
pub type FileSourceLoader = dyn Fn() -> Option<FileSource> + Send + Sync;

static FILE_SOURCE: Mutex<Option<Arc<FileSourceLoader>>> = Mutex::new(None);

/// Makes every recorder opened from now on play the loader's samples in
/// real time, then silence, instead of opening a microphone; `None` goes
/// back to the real device. For test builds of an application: the caller
/// decides when that is allowed. Recorders already open keep their device.
pub fn set_file_source(loader: Option<Arc<FileSourceLoader>>) {
    *FILE_SOURCE.lock().unwrap_or_else(|e| e.into_inner()) = loader;
}

/// Whether a file source is set.
pub fn file_source_active() -> bool {
    FILE_SOURCE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some()
}

/// Sees every chunk delivered to any sink: interleaved samples (real
/// frames only), sample rate, channels. Runs on the delivery thread, before
/// the sink. Added by Wispr Bro (OT2-B: the Flow Bar's speech meter).
pub type ChunkTap = fn(&[f32], u32, u16);

static CHUNK_TAP: std::sync::RwLock<Option<ChunkTap>> = std::sync::RwLock::new(None);

/// Sets (or with `None` clears) the [`ChunkTap`]. Added by Wispr Bro.
pub fn set_chunk_tap(tap: Option<ChunkTap>) {
    *CHUNK_TAP.write().unwrap_or_else(|e| e.into_inner()) = tap;
}

/// Hands one delivered chunk to the tap, if one is set.
pub(crate) fn tap_chunk(samples: &[f32], sample_rate: u32, channels: u16) {
    let tap = *CHUNK_TAP.read().unwrap_or_else(|e| e.into_inner());
    if let Some(tap) = tap {
        tap(samples, sample_rate, channels);
    }
}

/// The loudest chunk's mean square delivered since the last
/// [`take_peak_level_db`] (`f32` bits; positive floats order like their
/// bits), and whether one arrived. Added by Wispr Bro (Design W4).
static PEAK_MEAN_SQUARE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static PEAK_FRESH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Notes one delivered chunk's level (its real frames, every channel).
pub(crate) fn note_chunk_level(samples: &[f32]) {
    if samples.is_empty() {
        return;
    }
    let mean_square = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    if mean_square.is_finite() {
        PEAK_MEAN_SQUARE.fetch_max(
            mean_square.max(0.0).to_bits(),
            std::sync::atomic::Ordering::Relaxed,
        );
        PEAK_FRESH.store(true, std::sync::atomic::Ordering::Release);
    }
}

/// The RMS level, in dBFS, of the loudest chunk delivered to any sink
/// since the previous call, or `None` when none was. Unlike a visualiser
/// scale it has no ceiling, so a level meter can tell loud speech from
/// ordinary speech. Added by Wispr Bro (Design W4).
pub fn take_peak_level_db() -> Option<f32> {
    if !PEAK_FRESH.swap(false, std::sync::atomic::Ordering::AcqRel) {
        return None;
    }
    let mean_square =
        f32::from_bits(PEAK_MEAN_SQUARE.swap(0, std::sync::atomic::Ordering::Relaxed));
    Some(if mean_square > 1e-12 {
        10.0 * mean_square.log10()
    } else {
        -120.0
    })
}

fn selected_backend() -> Arc<dyn Backend> {
    match FILE_SOURCE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
    {
        Some(loader) => Arc::new(FileBackend::new(loader)),
        None => CpalBackend::shared(),
    }
}

/// An open microphone, recording into sinks of type `S`. Owned, `Send`,
/// `Sync`, lifetime-free: store it in a struct, or share it with an `Arc`.
///
/// Every recording uses the same sink type; use an enum or `Box<dyn Sink>`
/// for several kinds.
///
/// Dropping it is `close`.
pub struct Recorder<S> {
    pub(crate) engine: Engine<S>,
}

impl<S: Sink> Recorder<S> {
    /// Opens the device at its OS format. Audio may take seconds to flow
    /// (Bluetooth); the sink's first chunk marks it. A device that never
    /// delivers fails with `NoAudio`.
    ///
    /// Without a failure handler, failures surface only at the next `start` or
    /// `stop`; prefer [`open_with_failure_handler`](Self::open_with_failure_handler).
    pub fn open(config: RecorderConfig) -> Result<Self, Error> {
        Ok(Self {
            engine: Engine::open(selected_backend(), config, None, Timeouts::default())?,
        })
    }

    /// `open`, plus a handler called the moment this recorder fails.
    pub fn open_with_failure_handler(
        config: RecorderConfig,
        handler: impl FnOnce(Error) + Send + 'static,
    ) -> Result<Self, Error> {
        Ok(Self {
            engine: Engine::open(
                selected_backend(),
                config,
                Some(Box::new(handler)),
                Timeouts::default(),
            )?,
        })
    }

    /// What was opened: the device, the format it runs at, and the format
    /// the sink receives.
    pub fn info(&self) -> &RecorderInfo {
        self.engine.info()
    }

    /// Starts a recording into `sink`. Never waits for audio or touches the
    /// device. On error the sink comes back.
    pub fn start(&self, sink: S) -> Result<(), StartError<S>> {
        self.engine.start(sink)
    }

    /// Ends the recording and returns the sink, even after a failure or sink
    /// panic; `end_reason` says what ended it. A recording that ended on its
    /// own still needs this `stop` before the next `start`.
    ///
    /// Errors: `NotRecording`, `StopFromSink` (the recording continues), or the
    /// sink is lost (usually `SinkStalled`).
    pub fn stop(&self) -> Result<Stopped<S>, Error> {
        self.engine.stop()
    }

    /// Closes the microphone. An active recording is discarded; call `stop`
    /// first to keep it.
    pub fn close(self) -> Result<(), Error> {
        self.engine.shutdown()
    }
}

impl<S> Drop for Recorder<S> {
    fn drop(&mut self) {
        if let Err(error) = self.engine.shutdown() {
            log::warn!("closing the recorder on drop failed: {error}");
        }
    }
}

/// `start` failed. The sink is handed back.
pub struct StartError<S> {
    /// `AlreadyRecording`, or the error the recorder failed with.
    pub error: Error,
    pub sink: S,
}

/// For callers that do not need the sink back: `?` drops it.
impl<S> From<StartError<S>> for Error {
    fn from(e: StartError<S>) -> Error {
        e.error
    }
}

impl<S> fmt::Debug for StartError<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StartError")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl<S> fmt::Display for StartError<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot start recording: {}", self.error)
    }
}

impl<S> std::error::Error for StartError<S> {}

/// What `stop` returns: the sink, what ended the recording, and what was lost.
#[derive(Debug)]
pub struct Stopped<S> {
    pub sink: S,
    pub end_reason: EndReason,
    /// Frames lost because the ring was full, usually a slow sink. Counted at
    /// the device's rate, before resampling.
    pub dropped_frames: u64,
}

impl<S> Stopped<S> {
    /// `StopCalled` and no dropped frames.
    pub fn is_complete(&self) -> bool {
        matches!(self.end_reason, EndReason::StopCalled) && self.dropped_frames == 0
    }
}

// Public handles are `Send`, and the recorder is also `Sync`.
const _: () = {
    const fn assert_send<T: Send>() {}
    const fn assert_sync<T: Sync>() {}
    assert_send::<Recorder<CollectingSink>>();
    assert_sync::<Recorder<CollectingSink>>();
    assert_send::<Stopped<CollectingSink>>();
    assert_send::<StartError<CollectingSink>>();
    assert_send::<Error>();
};

/// Added by Wispr Bro (Design W4).
#[cfg(test)]
mod level_tap_tests {
    #[test]
    fn the_peak_level_has_no_ceiling() {
        // Other tests deliver quieter audio meanwhile; the loudest wins.
        super::note_chunk_level(&[0.01; 64]);
        super::note_chunk_level(&[1.0, -1.0, 1.0, -1.0]);
        let db = super::take_peak_level_db().expect("a level");
        assert!(db > -0.1 && db <= 0.01, "{db}");
        super::note_chunk_level(&[0.1; 32]);
        let db = super::take_peak_level_db().expect("a level");
        assert!(db > -20.1, "{db}");
    }

    static TAPPED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    fn count(samples: &[f32], rate: u32, channels: u16) {
        assert_eq!((rate, channels), (16_000, 1));
        TAPPED.fetch_add(samples.len(), std::sync::atomic::Ordering::SeqCst);
    }

    /// Added by Wispr Bro (OT2-B).
    #[test]
    fn the_chunk_tap_sees_chunks_until_cleared() {
        super::set_chunk_tap(Some(count));
        super::tap_chunk(&[0.0; 10], 16_000, 1);
        super::set_chunk_tap(None);
        super::tap_chunk(&[0.0; 10], 16_000, 1);
        assert_eq!(TAPPED.load(std::sync::atomic::Ordering::SeqCst), 10);
    }
}
