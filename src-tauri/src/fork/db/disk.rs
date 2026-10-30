// SPDX-License-Identifier: GPL-3.0-or-later
//! A full disk never costs the user their words (S-09).
//!
//! - **While recording:** a watcher checks the free space of the app data
//!   volume every [`POLL`]. Below [`LOW_DISK_BYTES`] it stops the recording
//!   (through `fork::insert`'s session path, the same one sleep uses), and
//!   the take's text goes to the card instead of being pasted
//!   (`CardReason::DiskFull`; S-05 draws the card, S-03 the pill message,
//!   S-18 adds the inbox entry, see [`report_disk_full`]).
//! - **When the WAV cannot be written:** Handy would skip the history row;
//!   the `actions.rs` hook ([`keep_without_audio`]) keeps it, text only, and
//!   removes the partial WAV. The row is marked as having no audio.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Runtime};

/// Free space below which a recording is stopped. A minute of WAV is
/// 1.92 MB, and macOS itself misbehaves well before the disk is this full.
pub const LOW_DISK_BYTES: u64 = 50 * 1024 * 1024;

/// How often the watcher checks while recording.
pub const POLL: Duration = Duration::from_secs(1);

/// The watcher waits this long for the recording to begin before giving up.
const START_GRACE: Duration = Duration::from_secs(5);

/// Bytes available to this user on the volume holding `path`.
#[cfg(unix)]
pub fn free_bytes(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut st = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    // SAFETY: `c` is a valid NUL-terminated path and `st` is writable
    // memory of the right type; statvfs only writes to it.
    let rc = unsafe { libc::statvfs(c.as_ptr(), st.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }
    // SAFETY: statvfs returned 0, so it filled `st`.
    let st = unsafe { st.assume_init() };
    #[allow(clippy::unnecessary_cast)] // the field types differ per platform
    Some((st.f_bavail as u64).saturating_mul(st.f_frsize as u64))
}

#[cfg(not(unix))]
pub fn free_bytes(_path: &Path) -> Option<u64> {
    None
}

/// The watcher's decision for one check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// Keep watching.
    Continue,
    /// The recording ended (or never began): stop watching.
    Done,
    /// Space ran out during the recording.
    DiskFull,
}

/// Pure decision: `recording` now, `seen_recording` before, how long the
/// watcher has run, and the free space (unknown counts as enough).
pub fn check(recording: bool, seen_recording: bool, elapsed: Duration, free: Option<u64>) -> Check {
    if !recording {
        return if seen_recording || elapsed >= START_GRACE {
            Check::Done
        } else {
            Check::Continue
        };
    }
    match free {
        Some(bytes) if bytes < LOW_DISK_BYTES => Check::DiskFull,
        _ => Check::Continue,
    }
}

/// Bumped per take, so a watcher for an older take stops.
static TAKE: AtomicU64 = AtomicU64::new(0);

/// Watches one take's recording (from `fork::on_record_start`).
pub fn on_record_start<R: Runtime>(app: &AppHandle<R>, dir: PathBuf) {
    let take = TAKE.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("fork-disk-watch".into())
        .spawn(move || watch(&app, take, &dir, is_recording::<R>));
    if let Err(e) = spawned {
        log::warn!("disk: could not start the watcher: {e}");
    }
}

fn is_recording<R: Runtime>(app: &AppHandle<R>) -> bool {
    use tauri::Manager;
    app.try_state::<std::sync::Arc<crate::managers::audio::AudioRecordingManager>>()
        .is_some_and(|rm| rm.is_recording())
}

fn watch<R: Runtime>(
    app: &AppHandle<R>,
    take: u64,
    dir: &Path,
    recording: fn(&AppHandle<R>) -> bool,
) {
    let started = Instant::now();
    let mut seen = false;
    loop {
        if TAKE.load(Ordering::SeqCst) != take {
            return;
        }
        let now_recording = recording(app);
        let free = if now_recording { free_bytes(dir) } else { None };
        match check(now_recording, seen, started.elapsed(), free) {
            Check::Continue => {}
            Check::Done => return,
            Check::DiskFull => {
                log::warn!(
                    "disk: {} MB free at {}; stopping the recording",
                    free.unwrap_or(0) / (1024 * 1024),
                    dir.display()
                );
                report_disk_full(dir);
                crate::fork::insert::on_disk_full(app);
                return;
            }
        }
        seen |= now_recording;
        std::thread::sleep(POLL);
    }
}

/// Called when the disk filled up during a take. Only logs for now; S-18
/// turns this into an inbox entry, S-03 into the pill message.
pub fn report_disk_full(dir: &Path) {
    log::warn!(
        "disk: the disk holding {} is full; the take's text goes to the card and stays in History",
        dir.display()
    );
}

/// The `actions.rs` hook for a WAV that could not be saved: the history row
/// is still written (text only). Removes a partial WAV and marks the take.
/// Always `true` (keep the row); a function so the decision lives here.
pub fn keep_without_audio(wav: &Path) -> bool {
    match std::fs::remove_file(wav) {
        Ok(()) => log::warn!(
            "disk: removed the partial recording {}; keeping the text without audio",
            wav.display()
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::warn!("disk: no recording was written; keeping the text without audio")
        }
        Err(e) => log::warn!("disk: could not remove the partial recording: {e}"),
    }
    if let Some(name) = wav.file_name().and_then(|n| n.to_str()) {
        crate::fork::context::take::ledger().mark_audio_missing(name);
    }
    if let Some(dir) = wav.parent() {
        report_disk_full(dir);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Duration = Duration::from_secs(1);

    #[test]
    fn stops_only_while_recording_and_below_the_threshold() {
        assert_eq!(
            check(true, false, S, Some(LOW_DISK_BYTES - 1)),
            Check::DiskFull
        );
        assert_eq!(check(true, true, S, Some(LOW_DISK_BYTES)), Check::Continue);
        assert_eq!(
            check(true, true, S, Some(10 * LOW_DISK_BYTES)),
            Check::Continue
        );
        // Unknown free space never stops a take.
        assert_eq!(check(true, true, S, None), Check::Continue);
    }

    #[test]
    fn waits_for_the_recording_then_ends_with_it() {
        // Not started yet: keep waiting within the grace period.
        assert_eq!(check(false, false, S, None), Check::Continue);
        assert_eq!(check(false, false, START_GRACE, None), Check::Done);
        // Started, then stopped.
        assert_eq!(check(false, true, S, None), Check::Done);
    }

    #[test]
    fn free_bytes_reads_the_volume() {
        let dir = tempfile::tempdir().unwrap();
        let free = free_bytes(dir.path()).expect("statvfs works on a temp dir");
        assert!(free > 0);
        assert_eq!(free_bytes(&dir.path().join("missing/deeper")), None);
    }

    #[test]
    fn a_failed_wav_keeps_the_row_and_marks_the_take() {
        use crate::fork::context::take::{ledger, TakeContext};
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("wispr-bro-disk-full-test.wav");
        std::fs::write(&wav, b"partial").unwrap();
        ledger().bind("wispr-bro-disk-full-test.wav", 1000, || {
            TakeContext::new(1, "2026-10-30".into(), None)
        });
        assert!(keep_without_audio(&wav));
        assert!(!wav.exists(), "the partial WAV is removed");
        let b = ledger().take_bound("wispr-bro-disk-full-test.wav").unwrap();
        assert!(b.audio_missing);
        // Nothing written at all is fine too.
        assert!(keep_without_audio(&dir.path().join("never-written.wav")));
    }
}
