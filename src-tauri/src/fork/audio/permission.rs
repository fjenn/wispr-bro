// SPDX-License-Identifier: GPL-3.0-or-later
//! Microphone permission revoked while the app runs (R5 S-08, DEC-55).
//!
//! RS-11 found that macOS does not cut a running take when the Microphone
//! switch is turned off: the app keeps the mic "until it is quit", and
//! macOS offers **Quit & Reopen** / **Later**. Nothing can be detected
//! mid-take, so the handling is:
//!
//! 1. The take finishes normally (nothing to do).
//! 2. If the app is asked to quit while a take is recording (Quit & Reopen,
//!    ⌘Q, logout), [`on_exit`] stops the take and saves its audio as a WAV
//!    under `<app data>/recovered/` before the process ends.
//! 3. At the next launch, [`on_ready`] hands each saved WAV to Handy's file
//!    transcription (`TranscriptionJobManager::transcribe_file`), which adds
//!    it to History and transcribes it once a model is loaded; the file is
//!    removed once History has its copy, and an inbox entry says so.
//! 4. The next take that cannot open the mic gets `PermissionDenied`, and the
//!    Flow Bar shows "Microphone access is off" with Fix (S-03's message;
//!    `fork::audio::enable_prepared` reads the permission fresh at launch,
//!    so a revoked mic is never prepared).
//!
//! The decisions are pure functions over two small traits ([`RunningTake`],
//! [`Transcriber`]) so the tests drive them with fakes.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime};

use crate::fork::ForkRuntime;
use crate::managers::audio::AudioRecordingManager;

/// Folder under the app data dir for takes saved at quit.
pub const RECOVERED_DIR: &str = "recovered";
/// Shorter than this (16 kHz samples, 0.3 s) is not worth offering.
pub const MIN_SAMPLES: usize = 4_800;
const SAMPLE_RATE: usize = 16_000;

/// A take that may be recording.
pub trait RunningTake {
    fn is_recording(&self) -> bool;
    /// Stops the take and returns its 16 kHz mono samples, without
    /// transcribing or saving anything.
    fn stop_and_take(&self) -> Option<Vec<f32>>;
}

impl RunningTake for AudioRecordingManager {
    fn is_recording(&self) -> bool {
        AudioRecordingManager::is_recording(self)
    }

    fn stop_and_take(&self) -> Option<Vec<f32>> {
        // Handy's state machine knows the binding; the fork's takes use one
        // of the two transcribe bindings.
        let generation = self.cancel_generation();
        ["transcribe", "transcribe_with_post_process"]
            .into_iter()
            .find_map(|binding| self.stop_recording(binding, generation))
    }
}

/// File name of a take saved at `now_ms`.
pub fn recovered_name(now_ms: i64) -> String {
    format!("{}-recovered-{now_ms}.wav", crate::fork::FILE_PREFIX)
}

/// Writes 16 kHz mono float samples as a WAV (written aside, then renamed,
/// so a crash never leaves a partial file under the final name).
pub fn write_wav(path: &Path, samples: &[f32]) -> Result<(), String> {
    let partial = path.with_extension("wav.partial");
    crate::audio_toolkit::save_wav_file(&partial, samples)
        .map_err(|e| e.to_string())
        .and_then(|()| std::fs::rename(&partial, path).map_err(|e| e.to_string()))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&partial);
        })
}

/// If a take is recording, stops it and saves its audio in `dir`. Returns
/// the file, or `None` when nothing was recording or it was too short.
pub fn save_running_take(take: &dyn RunningTake, dir: &Path, now_ms: i64) -> Option<PathBuf> {
    if !take.is_recording() {
        return None;
    }
    let samples = take.stop_and_take()?;
    if samples.len() < MIN_SAMPLES {
        log::info!(
            "fork audio: the take running at quit was too short to keep ({} ms)",
            samples.len() * 1000 / SAMPLE_RATE
        );
        return None;
    }
    if let Err(e) = std::fs::create_dir_all(dir) {
        log::error!("fork audio: cannot create the folder for the take running at quit: {e}");
        return None;
    }
    let path = dir.join(recovered_name(now_ms));
    match write_wav(&path, &samples) {
        Ok(()) => {
            log::info!(
                "fork audio: saved the take running at quit ({} ms of audio)",
                samples.len() * 1000 / SAMPLE_RATE
            );
            Some(path)
        }
        Err(e) => {
            log::error!("fork audio: cannot save the take running at quit: {e}");
            None
        }
    }
}

/// Takes saved at an earlier quit, oldest first.
pub fn pending(dir: &Path) -> Vec<PathBuf> {
    let prefix = format!("{}-recovered-", crate::fork::FILE_PREFIX);
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "wav")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&prefix))
        })
        .collect();
    files.sort();
    files
}

/// Adds a recording to History and queues its transcription.
pub trait Transcriber {
    fn transcribe_file(&self, path: &Path) -> Result<(), String>;
}

/// Hands every pending take to `t`; removes each one History took. Returns
/// how many were handed over. A file History refused stays for the next
/// launch.
pub fn recover(dir: &Path, t: &dyn Transcriber) -> usize {
    let mut done = 0;
    for path in pending(dir) {
        match t.transcribe_file(&path) {
            Ok(()) => {
                done += 1;
                if let Err(e) = std::fs::remove_file(&path) {
                    log::warn!("fork audio: cannot remove a recovered take after queuing it: {e}");
                }
            }
            Err(e) => log::warn!("fork audio: cannot queue a recovered take: {e}"),
        }
    }
    done
}

/// The recovery folder for this app (portable-aware).
pub fn recovered_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    crate::portable::data_dir()
        .cloned()
        .or_else(|| app.path().app_data_dir().ok())
        .map(|d| d.join(RECOVERED_DIR))
}

/// From the plugin's `RunEvent::ExitRequested` and `RunEvent::Exit`: save a
/// take that is still recording. Runs once per process.
pub fn on_exit<R: Runtime>(app: &AppHandle<R>) {
    static DONE: AtomicBool = AtomicBool::new(false);
    let Some(rm) = app
        .try_state::<Arc<AudioRecordingManager>>()
        .map(|m| Arc::clone(&m))
    else {
        return;
    };
    if !rm.is_recording() || DONE.swap(true, Ordering::AcqRel) {
        return;
    }
    let Some(dir) = recovered_dir(app) else {
        log::error!("fork audio: no app data dir; the take running at quit is lost");
        return;
    };
    save_running_take(rm.as_ref(), &dir, crate::fork::context::now_ms());
}

struct HandyJobs<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Transcriber for HandyJobs<R> {
    fn transcribe_file(&self, path: &Path) -> Result<(), String> {
        let jobs = self
            .0
            .try_state::<Arc<crate::managers::transcription_jobs::TranscriptionJobManager>>()
            .ok_or("Handy's transcription jobs are not running")?;
        jobs.transcribe_file(path.to_path_buf())
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// From `fork::on_ready` (product builds): offer the takes saved at the last
/// quit, off the main thread.
pub fn on_ready<R: ForkRuntime>(app: &AppHandle<R>) {
    let Some(dir) = recovered_dir(app) else {
        return;
    };
    if pending(&dir).is_empty() {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("wb-take-recover".into())
        .spawn(move || {
            let n = recover(&dir, &HandyJobs(app.clone()));
            if n > 0 {
                log::info!("fork audio: {n} take(s) saved at the last quit are in History");
                crate::fork::notify::post(
                    &app,
                    crate::fork::notify::catalog::Notice::take_recovered(n as u32),
                );
            }
        });
    if let Err(e) = spawned {
        log::warn!("fork audio: cannot recover the saved takes: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A take that is (or is not) recording with some samples.
    struct FakeTake {
        recording: bool,
        samples: Option<Vec<f32>>,
        stopped: RefCell<u32>,
    }

    impl RunningTake for FakeTake {
        fn is_recording(&self) -> bool {
            self.recording
        }
        fn stop_and_take(&self) -> Option<Vec<f32>> {
            *self.stopped.borrow_mut() += 1;
            self.samples.clone()
        }
    }

    fn take(recording: bool, seconds: f32) -> FakeTake {
        let n = (seconds * SAMPLE_RATE as f32) as usize;
        FakeTake {
            recording,
            samples: Some((0..n).map(|i| ((i % 64) as f32 / 64.0) - 0.5).collect()),
            stopped: RefCell::new(0),
        }
    }

    struct FakeHistory {
        accept: bool,
        got: RefCell<Vec<(PathBuf, usize)>>,
    }

    impl Transcriber for FakeHistory {
        fn transcribe_file(&self, path: &Path) -> Result<(), String> {
            let samples =
                crate::audio_toolkit::read_wav_samples(path).map_err(|e| e.to_string())?;
            self.got
                .borrow_mut()
                .push((path.to_path_buf(), samples.len()));
            if self.accept {
                Ok(())
            } else {
                Err("History is full".into())
            }
        }
    }

    #[test]
    fn a_take_recording_at_quit_is_saved_and_offered_at_the_next_launch() {
        let dir = tempfile::tempdir().unwrap();
        let t = take(true, 18.0);
        let saved = save_running_take(&t, dir.path(), 1_700_000_000_123).unwrap();
        assert_eq!(*t.stopped.borrow(), 1);
        assert_eq!(
            saved.file_name().unwrap().to_str().unwrap(),
            "wispr-bro-recovered-1700000000123.wav"
        );
        assert_eq!(pending(dir.path()), vec![saved.clone()]);
        // No partial file left behind.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);

        let history = FakeHistory {
            accept: true,
            got: RefCell::new(vec![]),
        };
        assert_eq!(recover(dir.path(), &history), 1);
        // Every sample arrived, then the file went (History has its copy).
        assert_eq!(history.got.borrow()[0].1, 18 * SAMPLE_RATE);
        assert!(pending(dir.path()).is_empty());
        assert!(!saved.exists());
    }

    #[test]
    fn nothing_is_saved_when_no_take_is_recording_or_it_is_too_short() {
        let dir = tempfile::tempdir().unwrap();
        let idle = take(false, 5.0);
        assert!(save_running_take(&idle, dir.path(), 1).is_none());
        assert_eq!(
            *idle.stopped.borrow(),
            0,
            "an idle recorder is never stopped"
        );
        assert!(save_running_take(&take(true, 0.1), dir.path(), 2).is_none());
        let none = FakeTake {
            recording: true,
            samples: None,
            stopped: RefCell::new(0),
        };
        assert!(save_running_take(&none, dir.path(), 3).is_none());
        assert!(pending(dir.path()).is_empty());
    }

    #[test]
    fn a_take_history_refused_stays_for_the_next_launch() {
        let dir = tempfile::tempdir().unwrap();
        save_running_take(&take(true, 2.0), dir.path(), 10).unwrap();
        save_running_take(&take(true, 3.0), dir.path(), 20).unwrap();
        let history = FakeHistory {
            accept: false,
            got: RefCell::new(vec![]),
        };
        assert_eq!(recover(dir.path(), &history), 0);
        assert_eq!(pending(dir.path()).len(), 2);
        // Oldest first.
        let order: Vec<usize> = history.got.borrow().iter().map(|g| g.1).collect();
        assert_eq!(order, [2 * SAMPLE_RATE, 3 * SAMPLE_RATE]);
    }

    #[test]
    fn only_our_recovered_wavs_are_pending() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "wispr-bro-recovered-5.wav",
            "wispr-bro-recovered-6.wav.partial",
            "wispr-bro-123.wav",
            "notes.txt",
        ] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        let names: Vec<String> = pending(dir.path())
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["wispr-bro-recovered-5.wav"]);
        assert!(pending(&dir.path().join("missing")).is_empty());
    }
}
