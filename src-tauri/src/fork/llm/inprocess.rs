// SPDX-License-Identifier: GPL-3.0-or-later
//! The cleanup model inside the app (iPhone): `crates/wb-ios-llm`, the
//! sidecar's llama.cpp and its server code linked in. An app on iOS cannot
//! start a process, so "start the server" is [`InProcessLauncher`]: a
//! listener thread on the transport the supervisor chose, answering
//! `/health` and chat completions as `llama-server` does. Everything above
//! the launcher (supervisor, health polling, idle stop, chat client, prompt,
//! guards, deadlines) is the Mac's code, unchanged.
//!
//! Also built on the Mac with the `llm-inprocess` feature, where
//! `WB_LLM_INPROCESS=1` makes the eval and the tests use it.

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use wb_ios_llm::serve::{Listen, Server as Listener, State};

use super::process::{Device, OutputTail, SpawnSpec, CACHE_RAM_MIB, CONTEXT_TOKENS};
use super::supervisor::{Launcher, Server};
use super::transport::Endpoint;

/// What the runtime passes as the "sidecar binary" when the server is this
/// launcher (nothing is executed).
pub const MARKER: &str = "in-process";

/// `true` when this build runs the cleanup model in process: always on
/// iPhone; on the Mac only with `WB_LLM_INPROCESS=1` (feature
/// `llm-inprocess`: the eval and tests on the iPhone's engine).
pub fn selected() -> bool {
    cfg!(target_os = "ios") || std::env::var("WB_LLM_INPROCESS").is_ok_and(|v| v == "1")
}

/// Starts the engine in this process.
pub struct InProcessLauncher;

type OnExit = Arc<Mutex<Option<Box<dyn FnOnce() + Send>>>>;

struct InProcessServer {
    listener: Listener,
    tail: Arc<Mutex<OutputTail>>,
    on_exit: OnExit,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn fire(on_exit: &OnExit) {
    if let Some(f) = lock(on_exit).take() {
        f();
    }
}

/// Where the layers run. iPhone: always the CPU. iOS refuses GPU work from
/// an app in the background, where every keyboard take runs (the speech
/// engine is on the CPU for the same reason), and the simulator's Metal
/// device computes wrong results with ggml.
fn device(spec: Device) -> wb_ios_llm::Device {
    // `WB_LLM_DEVICE=cpu` (debug builds): the iPhone's CPU path on the Mac.
    let forced = cfg!(debug_assertions) && std::env::var("WB_LLM_DEVICE").is_ok_and(|v| v == "cpu");
    if cfg!(target_os = "ios") || forced {
        return wb_ios_llm::Device::Cpu;
    }
    match spec {
        Device::Gpu => wb_ios_llm::Device::Gpu,
        Device::Cpu => wb_ios_llm::Device::Cpu,
    }
}

/// Threads for the model. The Mac's rule on the Mac; on iPhone the
/// performance cores (efficiency cores slow a llama.cpp decode down).
fn threads() -> usize {
    if let Some(n) = debug_usize("WB_LLM_THREADS") {
        return n.clamp(1, 16);
    }
    if cfg!(target_os = "ios") {
        wb_ios_llm::performance_cores().clamp(1, 8)
    } else {
        super::process::recommended_threads()
    }
}

/// llama-server arguments beyond the sidecar's.
///
/// iPhone: `--no-repack`. On the CPU llama.cpp otherwise copies the weights
/// into a layout its ARM kernels prefer; the copy is ordinary app memory
/// (about 1.5 GB for this model) on top of a speech model that already
/// holds 1.2-1.5 GB, against an app limit of about 3.4 GB on a 12 GB
/// iPhone: iOS ended the app during the second take. Without the copy the
/// weights stay a mapping of the model file, which iOS can drop and read
/// back and does not count against the app.
fn extra_args() -> Vec<String> {
    if cfg!(debug_assertions) {
        if let Ok(extra) = std::env::var("WB_LLM_EXTRA_ARGS") {
            return extra.split_whitespace().map(str::to_owned).collect();
        }
    }
    if cfg!(target_os = "ios") {
        vec!["--no-repack".into()]
    } else {
        Vec::new()
    }
}

/// A debug-build override (measurements on a phone).
fn debug_usize(var: &str) -> Option<usize> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var(var).ok()?.trim().parse().ok()
}

impl Launcher for InProcessLauncher {
    fn launch(
        &self,
        spec: &SpawnSpec,
        on_exit: Box<dyn FnOnce() + Send>,
    ) -> io::Result<Box<dyn Server>> {
        let mut config = wb_ios_llm::Config::new(&spec.model);
        config.alias = super::process::MODEL_ALIAS.into();
        config.context_tokens = CONTEXT_TOKENS;
        config.cache_ram_mib = CACHE_RAM_MIB;
        config.threads = threads();
        config.device = device(spec.device);
        config.extra_args = extra_args();
        let listen = match &spec.transport.endpoint {
            Endpoint::Unix(path) => Listen::Unix(path.clone()),
            Endpoint::Tcp(port) => Listen::Tcp(*port),
        };
        log::info!(
            "llm: in-process llama.cpp {} ({:?}, {} threads)",
            wb_ios_llm::build_info(),
            config.device,
            config.threads
        );
        let listener = Listener::start(listen, Some(spec.transport.api_key().to_owned()), config)?;
        let tail = Arc::new(Mutex::new(OutputTail::default()));
        let on_exit: OnExit = Arc::new(Mutex::new(Some(on_exit)));
        // A model that does not load is this server's "process exited".
        let state = listener.watch();
        let (t, e) = (tail.clone(), on_exit.clone());
        std::thread::Builder::new()
            .name("wb-llm-watch".into())
            .spawn(move || loop {
                match state() {
                    State::Loading => std::thread::sleep(Duration::from_millis(20)),
                    State::Ready => return,
                    State::Failed(message) => {
                        lock(&t).push_line(&format!("error: {message}"));
                        fire(&e);
                        return;
                    }
                    State::Stopped => {
                        fire(&e);
                        return;
                    }
                }
            })?;
        Ok(Box::new(InProcessServer {
            listener,
            tail,
            on_exit,
        }))
    }
}

impl Server for InProcessServer {
    fn pid(&self) -> u32 {
        std::process::id()
    }

    fn has_exited(&mut self) -> bool {
        matches!(self.listener.state(), State::Failed(_) | State::Stopped)
    }

    /// Stops the listener and frees the model.
    fn stop(&mut self) {
        self.listener.stop();
        fire(&self.on_exit);
    }

    fn tail(&self) -> OutputTail {
        lock(&self.tail).clone()
    }
}
