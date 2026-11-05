// SPDX-License-Identifier: MIT
// Copyright (c) 2026 handy-computer. handy-recorder 0.1.0, vendored by Wispr Bro (see Cargo.toml).
//! A deterministic backend for tests. The test thread plays the platform's
//! audio thread: [`FakeBackend::push`] calls the running stream's data
//! callback directly, so a test decides exactly when every block arrives.

use std::sync::{Arc, Condvar, Mutex};

use super::{
    Backend, BackendError, BackendErrorKind, DataCallback, DeviceFormat, ErrorCallback,
    InputSample, InputStream, OpenDevice,
};
use crate::{InputDevice, Permission};

/// One fake device. Clones share state, so a test keeps a clone to drive the
/// stream a recorder opened.
#[derive(Clone)]
pub(crate) struct FakeBackend {
    shared: Arc<Shared>,
}

struct Shared {
    device: InputDevice,
    format: DeviceFormat,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// The built stream's callbacks. `None` when no stream is built.
    stream: Option<(DataCallback, ErrorCallback)>,
    /// The built stream is running (always, unless built for
    /// `pause_when_idle`). A stopped stream gets no data blocks.
    running: bool,
    /// Every `set_running` call, in order.
    run_calls: Vec<bool>,
    /// Makes the next `set_running(true)` fail with this error.
    next_run_error: Option<BackendError>,
    next_open_error: Option<BackendError>,
    streams_started: usize,
    /// Makes the next `start` hang until the gate opens.
    start_gate: Option<Gate>,
    /// Makes stream teardown hang until the gate opens.
    teardown_gate: Option<Gate>,
    /// `None` reads as `Granted`.
    permission: Option<Permission>,
    /// Behave like WASAPI (denial fails the stream) instead of CoreAudio
    /// (denial delivers silence).
    denial_fails_stream: bool,
    /// Every `hold_headset` call, in order.
    headset_holds: Vec<bool>,
    /// The device is gone, but the stream keeps running (PulseAudio moves
    /// it to another source): `check_device` reports it.
    removed: bool,
    /// `check_device` panics, as a backend bug would.
    check_panics: bool,
}

/// Holds a platform call until opened, like a driver that hangs.
#[derive(Clone, Debug, Default)]
pub(crate) struct Gate(Arc<(Mutex<bool>, Condvar)>);

impl Gate {
    pub fn open(&self) {
        *self.0.0.lock().unwrap() = true;
        self.0.1.notify_all();
    }

    pub fn wait(&self) {
        let mut open = self.0.0.lock().unwrap();
        while !*open {
            open = self.0.1.wait(open).unwrap();
        }
    }
}

impl FakeBackend {
    pub fn new(format: DeviceFormat) -> Self {
        Self {
            shared: Arc::new(Shared {
                device: InputDevice {
                    id: "fake:mic".into(),
                    name: "Fake Mic".into(),
                    occurrence: 0,
                    backend: "Fake".into(),
                    is_default: true,
                    id_is_stable: true,
                    channels: Some(format.channels),
                    is_monitor: false,
                },
                format,
                state: Mutex::default(),
            }),
        }
    }

    /// Delivers one callback block to the running stream, on this thread.
    /// Returns false if no stream is running (none built, or built and
    /// stopped).
    pub fn push<T: InputSample>(&self, samples: &[T]) -> bool {
        let mut state = self.shared.state.lock().unwrap();
        if !state.running {
            return false;
        }
        match &mut state.stream {
            Some((data, _)) => {
                data(T::wrap(samples));
                true
            }
            None => false,
        }
    }

    /// Whether a built stream is running.
    pub fn is_running(&self) -> bool {
        let state = self.shared.state.lock().unwrap();
        state.stream.is_some() && state.running
    }

    /// Every `set_running` call so far.
    pub fn run_calls(&self) -> Vec<bool> {
        self.shared.state.lock().unwrap().run_calls.clone()
    }

    /// Makes the next `set_running(true)` fail with `error`.
    pub fn fail_next_run(&self, error: BackendError) {
        self.shared.state.lock().unwrap().next_run_error = Some(error);
    }

    /// Reports a stream error, as the platform's error callback would.
    /// Returns false if no stream is running.
    pub fn report_error(&self, error: BackendError) -> bool {
        match &mut self.shared.state.lock().unwrap().stream {
            Some((_, report)) => {
                report(error);
                true
            }
            None => false,
        }
    }

    /// Makes the next `open_device` fail with `error`.
    pub fn fail_next_open(&self, error: BackendError) {
        self.shared.state.lock().unwrap().next_open_error = Some(error);
    }

    /// Makes the next stream start hang until the returned gate opens.
    pub fn hang_next_start(&self) -> Gate {
        let gate = Gate::default();
        self.shared.state.lock().unwrap().start_gate = Some(gate.clone());
        gate
    }

    /// Makes stream teardown hang until the returned gate opens.
    pub fn hang_teardown(&self) -> Gate {
        let gate = Gate::default();
        self.shared.state.lock().unwrap().teardown_gate = Some(gate.clone());
        gate
    }

    pub fn set_permission(&self, permission: Permission) {
        self.shared.state.lock().unwrap().permission = Some(permission);
    }

    pub fn set_denial_fails_stream(&self) {
        self.shared.state.lock().unwrap().denial_fails_stream = true;
    }

    /// Removes the device without failing the stream, as the PulseAudio
    /// server does when it moves the stream to another source.
    pub fn remove_device(&self) {
        self.shared.state.lock().unwrap().removed = true;
    }

    /// Makes `check_device` panic.
    pub fn panic_in_check_device(&self) {
        self.shared.state.lock().unwrap().check_panics = true;
    }

    pub fn is_streaming(&self) -> bool {
        self.shared.state.lock().unwrap().stream.is_some()
    }

    pub fn headset_holds(&self) -> Vec<bool> {
        self.shared.state.lock().unwrap().headset_holds.clone()
    }

    pub fn streams_started(&self) -> usize {
        self.shared.state.lock().unwrap().streams_started
    }
}

impl Backend for FakeBackend {
    fn permission_status(&self) -> Permission {
        self.shared
            .state
            .lock()
            .unwrap()
            .permission
            .unwrap_or(Permission::Granted)
    }

    fn denial_is_silent(&self) -> bool {
        !self.shared.state.lock().unwrap().denial_fails_stream
    }

    fn list_input_devices(&self) -> Result<Vec<InputDevice>, BackendError> {
        Ok(vec![self.shared.device.clone()])
    }

    fn open_device(&self, id: Option<&str>) -> Result<Box<dyn OpenDevice>, BackendError> {
        if let Some(error) = self.shared.state.lock().unwrap().next_open_error.take() {
            return Err(error);
        }
        if id.is_some_and(|id| id != self.shared.device.id) {
            return Err(BackendError::new(
                BackendErrorKind::DeviceNotAvailable,
                format!("No input device with ID {id:?}"),
            ));
        }
        Ok(Box::new(FakeOpenDevice(self.clone())))
    }
}

struct FakeOpenDevice(FakeBackend);

impl OpenDevice for FakeOpenDevice {
    fn info(&self) -> &InputDevice {
        &self.0.shared.device
    }

    fn format(&self) -> DeviceFormat {
        self.0.shared.format
    }

    fn start(
        self: Box<Self>,
        data: DataCallback,
        error: ErrorCallback,
        run: bool,
    ) -> Result<Box<dyn InputStream>, BackendError> {
        let gate = self.0.shared.state.lock().unwrap().start_gate.take();
        if let Some(gate) = gate {
            gate.wait();
        }
        let mut state = self.0.shared.state.lock().unwrap();
        if state.stream.is_some() {
            return Err(BackendError::new(
                BackendErrorKind::DeviceBusy,
                "the fake runs one stream at a time",
            ));
        }
        state.stream = Some((data, error));
        state.running = run;
        state.streams_started += 1;
        drop(state);
        Ok(Box::new(FakeStream(self.0)))
    }
}

struct FakeStream(FakeBackend);

impl InputStream for FakeStream {
    fn set_running(&mut self, running: bool) -> Result<(), BackendError> {
        let mut state = self.0.shared.state.lock().unwrap();
        state.run_calls.push(running);
        if running && let Some(error) = state.next_run_error.take() {
            return Err(error);
        }
        state.running = running;
        Ok(())
    }

    fn hold_headset(&mut self, hold: bool) {
        self.0.shared.state.lock().unwrap().headset_holds.push(hold);
    }

    fn check_device(&mut self) -> Result<(), BackendError> {
        // Read, then panic, so the panic does not poison the fake's state.
        let (removed, panics) = {
            let state = self.0.shared.state.lock().unwrap();
            (state.removed, state.check_panics)
        };
        if panics {
            panic!("fake backend bug");
        }
        if removed {
            return Err(BackendError::new(
                BackendErrorKind::DeviceNotAvailable,
                "the fake device was removed",
            ));
        }
        Ok(())
    }
}

impl Drop for FakeStream {
    fn drop(&mut self) {
        let gate = self.0.shared.state.lock().unwrap().teardown_gate.take();
        if let Some(gate) = gate {
            gate.wait();
        }
        let mut state = self.0.shared.state.lock().unwrap();
        state.stream = None;
        state.running = false;
    }
}
