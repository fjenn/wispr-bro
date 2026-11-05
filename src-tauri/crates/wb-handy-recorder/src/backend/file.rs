// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Wispr Bro contributors. Added to the vendored
// handy-recorder 0.1.0 (see Cargo.toml).
//! A microphone that plays prepared samples in real time (test only).
//!
//! When the application calls [`crate::set_file_source`], every recorder
//! opened afterwards uses this backend instead of the platform's: whatever
//! device the config names, the stream delivers the loader's samples in
//! 10 ms blocks paced by the wall clock, then silence. Each start of the
//! stream (a cold open, or `set_running(true)` on a prepared one) asks the
//! loader again and plays from the beginning, so every recording hears the
//! whole file, and a driver may swap the file between recordings. The rest
//! of the library (resampling, chunking, the watchdog) and the
//! application's sink run unchanged.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::{
    Backend, BackendError, BackendErrorKind, DataCallback, DeviceFormat, ErrorCallback, InputData,
    InputStream, OpenDevice, SampleFormat,
};
use crate::{FileSource, FileSourceLoader, InputDevice, Permission};

/// Block length.
const BLOCK: Duration = Duration::from_millis(10);

pub(crate) struct FileBackend {
    loader: Arc<FileSourceLoader>,
}

impl FileBackend {
    pub fn new(loader: Arc<FileSourceLoader>) -> Self {
        Self { loader }
    }
}

fn device(label: &str) -> InputDevice {
    InputDevice {
        id: "file:test-mic".into(),
        name: format!("Test microphone ({label})"),
        occurrence: 0,
        backend: "File".into(),
        is_default: true,
        id_is_stable: true,
        channels: Some(1),
        is_monitor: false,
    }
}

fn unavailable() -> BackendError {
    BackendError::new(
        BackendErrorKind::DeviceNotAvailable,
        "the test microphone's file cannot be read",
    )
}

/// `samples` at `to` Hz (linear interpolation; files are speech).
fn resample(source: &FileSource, to: u32) -> Vec<f32> {
    if source.sample_rate == to || source.samples.is_empty() {
        return source.samples.clone();
    }
    let ratio = f64::from(source.sample_rate) / f64::from(to);
    let n = (source.samples.len() as f64 / ratio).floor() as usize;
    (0..n)
        .map(|i| {
            let x = i as f64 * ratio;
            let j = x.floor() as usize;
            let frac = (x - j as f64) as f32;
            let a = source.samples[j];
            let b = source.samples.get(j + 1).copied().unwrap_or(a);
            a + (b - a) * frac
        })
        .collect()
}

impl Backend for FileBackend {
    fn list_input_devices(&self) -> Result<Vec<InputDevice>, BackendError> {
        let label = (self.loader)().map(|s| s.label).unwrap_or_default();
        Ok(vec![device(&label)])
    }

    /// Any id opens the file: the app's chosen microphone is replaced.
    fn open_device(&self, _id: Option<&str>) -> Result<Box<dyn OpenDevice>, BackendError> {
        let source = (self.loader)().ok_or_else(unavailable)?;
        Ok(Box::new(FileOpenDevice {
            device: device(&source.label),
            format: DeviceFormat {
                sample_rate: source.sample_rate.max(1),
                channels: 1,
                sample_format: SampleFormat::F32,
            },
            loader: self.loader.clone(),
        }))
    }

    fn permission_status(&self) -> Permission {
        Permission::Granted
    }

    fn denial_is_silent(&self) -> bool {
        false
    }
}

struct FileOpenDevice {
    device: InputDevice,
    format: DeviceFormat,
    loader: Arc<FileSourceLoader>,
}

impl OpenDevice for FileOpenDevice {
    fn info(&self) -> &InputDevice {
        &self.device
    }

    fn format(&self) -> DeviceFormat {
        self.format
    }

    fn start(
        self: Box<Self>,
        mut data: DataCallback,
        _error: ErrorCallback,
        run: bool,
    ) -> Result<Box<dyn InputStream>, BackendError> {
        let state = Arc::new(StreamState {
            alive: AtomicBool::new(true),
            running: AtomicBool::new(run),
            restart: AtomicBool::new(true),
        });
        let thread_state = state.clone();
        let loader = self.loader;
        let rate = self.format.sample_rate;
        let block = (rate as usize / 100).max(1);
        let thread = std::thread::Builder::new()
            .name("handy-recorder-file".into())
            .spawn(move || {
                let silence = vec![0.0f32; block];
                let mut padded = vec![0.0f32; block];
                let mut samples: Vec<f32> = Vec::new();
                let mut pos = 0usize;
                let mut next = Instant::now();
                while thread_state.alive.load(Ordering::Acquire) {
                    if !thread_state.running.load(Ordering::Acquire) {
                        std::thread::sleep(Duration::from_millis(2));
                        next = Instant::now();
                        continue;
                    }
                    if thread_state.restart.swap(false, Ordering::AcqRel) {
                        samples = loader().map(|s| resample(&s, rate)).unwrap_or_default();
                        pos = 0;
                        next = Instant::now();
                    }
                    if pos < samples.len() {
                        let end = (pos + block).min(samples.len());
                        let chunk = &samples[pos..end];
                        if chunk.len() == block {
                            data(InputData::F32(chunk));
                        } else {
                            padded.fill(0.0);
                            padded[..chunk.len()].copy_from_slice(chunk);
                            data(InputData::F32(&padded));
                        }
                        pos = end;
                    } else {
                        data(InputData::F32(&silence));
                    }
                    next += BLOCK;
                    let now = Instant::now();
                    if next > now {
                        std::thread::sleep(next - now);
                    } else if now - next > Duration::from_millis(200) {
                        // Far behind (a stalled machine): do not burst.
                        next = now;
                    }
                }
            })
            .map_err(|e| BackendError::new(BackendErrorKind::Other, e.to_string()))?;
        Ok(Box::new(FileStream {
            state,
            thread: Mutex::new(Some(thread)),
        }))
    }
}

struct StreamState {
    alive: AtomicBool,
    running: AtomicBool,
    /// The next block reloads the source and starts it again.
    restart: AtomicBool,
}

struct FileStream {
    state: Arc<StreamState>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl InputStream for FileStream {
    fn set_running(&mut self, running: bool) -> Result<(), BackendError> {
        let was = self.state.running.swap(running, Ordering::AcqRel);
        if running && !was {
            self.state.restart.store(true, Ordering::Release);
        }
        Ok(())
    }
}

impl Drop for FileStream {
    fn drop(&mut self) {
        self.state.alive.store(false, Ordering::Release);
        if let Some(thread) = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn fixed(source: FileSource) -> Arc<FileSourceLoader> {
        Arc::new(move || Some(source.clone()))
    }

    fn voice(len: usize, rate: u32) -> FileSource {
        FileSource {
            samples: (0..len).map(|i| (i % 100) as f32 / 100.0 + 0.001).collect(),
            sample_rate: rate,
            label: "t.wav".into(),
        }
    }

    fn counting(voiced: &Arc<AtomicUsize>, total: &Arc<AtomicUsize>) -> DataCallback {
        let (v, t) = (voiced.clone(), total.clone());
        Box::new(move |d| {
            if let InputData::F32(s) = d {
                t.fetch_add(s.len(), Ordering::Relaxed);
                v.fetch_add(s.iter().filter(|x| **x != 0.0).count(), Ordering::Relaxed);
            }
        })
    }

    fn wait_for(counter: &AtomicUsize, at_least: usize) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while counter.load(Ordering::Relaxed) < at_least && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn plays_the_source_in_real_time_then_silence_and_again_on_each_start() {
        let backend = FileBackend::new(fixed(voice(1_600, 16_000))); // 100 ms
        let device = backend.open_device(Some("any:mic")).unwrap();
        assert_eq!(device.format().sample_rate, 16_000);
        let voiced = Arc::new(AtomicUsize::new(0));
        let total = Arc::new(AtomicUsize::new(0));
        let mut stream = device
            .start(counting(&voiced, &total), Box::new(|_| {}), false)
            .unwrap();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(total.load(Ordering::Relaxed), 0, "stopped: no audio");
        let started = Instant::now();
        stream.set_running(true).unwrap();
        wait_for(&total, 3_200);
        // 200 ms of audio cannot arrive faster than real time.
        assert!(
            started.elapsed() >= Duration::from_millis(180),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(
            voiced.load(Ordering::Relaxed),
            1_600,
            "the file once, then silence"
        );
        stream.set_running(false).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        stream.set_running(true).unwrap();
        wait_for(&voiced, 3_200);
        drop(stream);
        assert_eq!(
            voiced.load(Ordering::Relaxed),
            3_200,
            "replayed from the start"
        );
    }

    #[test]
    fn each_start_reads_the_current_file_at_the_device_rate() {
        // A driver swaps the file between takes; a 48 kHz file after a
        // 16 kHz open is resampled.
        let current = Arc::new(Mutex::new(voice(160, 16_000)));
        let c = current.clone();
        let backend = FileBackend::new(Arc::new(move || Some(c.lock().unwrap().clone())));
        let device = backend.open_device(None).unwrap();
        let voiced = Arc::new(AtomicUsize::new(0));
        let total = Arc::new(AtomicUsize::new(0));
        let mut stream = device
            .start(counting(&voiced, &total), Box::new(|_| {}), true)
            .unwrap();
        wait_for(&total, 800);
        assert_eq!(voiced.load(Ordering::Relaxed), 160);
        *current.lock().unwrap() = voice(4_800, 48_000); // 100 ms
        stream.set_running(false).unwrap();
        stream.set_running(true).unwrap();
        wait_for(&total, 800 + 3_200);
        assert_eq!(voiced.load(Ordering::Relaxed), 160 + 1_600);
        drop(stream);
        assert!(
            FileBackend::new(Arc::new(|| None))
                .open_device(None)
                .is_err()
        );
    }

    #[test]
    fn a_recorder_opened_with_a_file_source_records_the_file_through_the_pipeline() {
        // 48 kHz source, resampled to the 16 kHz speech format like a real
        // microphone.
        crate::set_file_source(Some(fixed(FileSource {
            samples: vec![0.25; 4_800], // 100 ms at 48 kHz
            sample_rate: 48_000,
            label: "t.wav".into(),
        })));
        let recorder = crate::Recorder::open(crate::RecorderConfig {
            device: Some("some-real-mic".into()),
            ..crate::RecorderConfig::speech()
        })
        .unwrap();
        crate::set_file_source(None);
        assert_eq!(recorder.info().device.backend, "File");
        assert_eq!(recorder.info().format.sample_rate, 16_000);
        recorder.start(crate::CollectingSink::new()).unwrap();
        std::thread::sleep(Duration::from_millis(400));
        let samples = recorder.stop().unwrap().sink.into_samples();
        assert!(samples.len() >= 4_000, "{}", samples.len());
        let loud = samples.iter().filter(|s| s.abs() > 0.1).count();
        assert!((1_200..=1_800).contains(&loud), "~100 ms of signal: {loud}");
        assert!(!crate::file_source_active());
    }
}
