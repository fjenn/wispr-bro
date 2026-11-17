// SPDX-License-Identifier: GPL-3.0-or-later
//! llama.cpp b11624 in process: the request code of `llama-server` (the Mac
//! app's sidecar) without a process, a socket or a listener.
//!
//! The Mac sends an OpenAI chat-completions body to `llama-server` and reads
//! server-sent events back (`fork/llm/chat.rs`). [`Engine::request`] takes
//! the same body and hands back the same bytes, produced by the same code:
//! llama-server's argument parser, chat template (Jinja), JSON-schema
//! grammar, sampler, slot and prompt cache. Nothing is re-implemented, so a
//! cleanup on the phone is the cleanup on the Mac.
//!
//! ```no_run
//! # use wb_ios_llm::*;
//! let engine = Engine::load(&Config::new("/path/model.gguf")).unwrap();
//! let stop = Stop::default();
//! let mut sse = Vec::new();
//! let status = engine
//!     .request("/v1/chat/completions", r#"{"messages":[],"stream":true}"#, &stop, |bytes| {
//!         sse.extend_from_slice(bytes)
//!     })
//!     .unwrap();
//! # let _ = status;
//! ```

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

mod memory;
pub mod serve;
pub use memory::{footprint_bytes, physical_memory_bytes};

/// The chat-completions route.
pub const CHAT_COMPLETIONS: &str = "/v1/chat/completions";

/// Where the model's layers run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    /// Every layer on the GPU (Metal); llama-server's default.
    Gpu,
    /// CPU only (`-ngl 0`): the fallback after a Metal failure, and the
    /// simulator.
    Cpu,
}

impl Device {
    /// The GPU, except in the iOS simulator: its Metal device computes wrong
    /// results with ggml (measured: garbage text, 44 s, then an abort), so
    /// the simulator runs on the CPU, as the speech engine does there.
    pub fn default_for_target() -> Self {
        if cfg!(all(target_os = "ios", target_abi = "sim")) {
            Device::Cpu
        } else {
            Device::Gpu
        }
    }
}

/// How to load a model. The defaults are the Mac sidecar's
/// (`fork/llm/process.rs` `SpawnSpec::args`).
#[derive(Debug, Clone)]
pub struct Config {
    pub model: PathBuf,
    /// `--alias`: the model name requests address.
    pub alias: String,
    /// `-c`: context size in tokens.
    pub context_tokens: u32,
    /// `--threads`.
    pub threads: usize,
    /// `--cache-ram`: host-memory prompt cache in MiB.
    pub cache_ram_mib: u32,
    pub device: Device,
    /// Appended after the arguments above (llama-server syntax).
    pub extra_args: Vec<String>,
}

impl Config {
    pub fn new(model: impl Into<PathBuf>) -> Self {
        Self {
            model: model.into(),
            alias: "local".into(),
            context_tokens: 4096,
            threads: recommended_threads(),
            cache_ram_mib: 256,
            device: Device::default_for_target(),
            extra_args: Vec::new(),
        }
    }

    /// llama-server's command line for this configuration: the Mac's
    /// arguments in the Mac's order, without the listener's (`--host`).
    pub fn server_args(&self) -> Vec<String> {
        let mut args: Vec<String> = vec![
            "llama-server".into(),
            "-m".into(),
            self.model.to_string_lossy().into_owned(),
            "--alias".into(),
            self.alias.clone(),
            "-c".into(),
            self.context_tokens.to_string(),
            "-np".into(),
            "1".into(),
            "--threads".into(),
            self.threads.to_string(),
            "--cache-ram".into(),
            self.cache_ram_mib.to_string(),
        ];
        for a in [
            "--reasoning",
            "off",
            "--jinja",
            "--no-ui",
            "--no-slots",
            "--offline",
            "--fit",
            "off",
        ] {
            args.push(a.into());
        }
        if self.device == Device::Cpu {
            args.extend(["-ngl".into(), "0".into()]);
        }
        args.extend(self.extra_args.iter().cloned());
        args
    }
}

/// The Mac's `process::recommended_threads`.
pub fn recommended_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .saturating_sub(2)
        .clamp(1, 8)
}

/// Why a call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The model did not load (missing file, not a GGUF, out of memory).
    Load(String),
    /// Another engine is loaded: one per process.
    Busy,
    /// The request stopped ([`Stop`]) before the server answered.
    Stopped,
    /// No such route.
    UnknownPath(String),
    /// This build has no llama.cpp (not an Apple arm64 target).
    Unsupported,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Load(m) => write!(f, "model load failed: {m}"),
            Error::Busy => write!(f, "an engine is already loaded"),
            Error::Stopped => write!(f, "stopped"),
            Error::UnknownPath(p) => write!(f, "unknown path {p}"),
            Error::Unsupported => write!(f, "llama.cpp is not built for this target"),
        }
    }
}

impl std::error::Error for Error {}

/// When a request must stop: a cancel flag (Esc, a new take) and/or a
/// deadline. The server looks between tokens, and once a second while it
/// waits for the first one, exactly as it notices a closed socket on the
/// Mac; the request's task is cancelled and its slot freed.
#[derive(Clone, Default)]
pub struct Stop {
    cancel: Arc<AtomicBool>,
    deadline: Option<Instant>,
    probe: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
}

impl std::fmt::Debug for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stop")
            .field("cancelled", &self.is_cancelled())
            .field("deadline", &self.deadline)
            .field("probe", &self.probe.is_some())
            .finish()
    }
}

impl Stop {
    pub fn with_deadline(deadline: Instant) -> Self {
        Self {
            deadline: Some(deadline),
            ..Self::default()
        }
    }

    /// A stop that shares `flag`: setting it cancels the request.
    pub fn with_flag(flag: Arc<AtomicBool>, deadline: Option<Instant>) -> Self {
        Self {
            cancel: flag,
            deadline,
            probe: None,
        }
    }

    /// Also stops when `probe` returns `true` (the listener: the client hung
    /// up). Called wherever the server looks, so it must be cheap.
    pub fn with_probe(mut self, probe: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        self.probe = Some(Arc::new(probe));
        self
    }

    /// The flag behind [`Stop::cancel`].
    pub fn flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    pub fn deadline_passed(&self) -> bool {
        self.deadline.is_some_and(|d| Instant::now() >= d)
    }

    pub fn should_stop(&self) -> bool {
        self.is_cancelled() || self.deadline_passed() || self.probe.as_ref().is_some_and(|p| p())
    }
}

/// Log level of llama.cpp on stderr: 0 errors (default), 1 warnings, 2 info,
/// 3 debug.
pub fn set_verbosity(level: u8) {
    #[cfg(not(wb_ios_llm_stub))]
    // SAFETY: plain C call with an integer.
    unsafe {
        ffi::wb_llm_set_verbosity(i32::from(level))
    }
    #[cfg(wb_ios_llm_stub)]
    let _ = level;
}

/// The linked llama.cpp build, e.g. `b11624-af0579c78`.
pub fn build_info() -> String {
    #[cfg(not(wb_ios_llm_stub))]
    // SAFETY: returns a static NUL-terminated string.
    unsafe {
        std::ffi::CStr::from_ptr(ffi::wb_llm_build_info())
            .to_string_lossy()
            .into_owned()
    }
    #[cfg(wb_ios_llm_stub)]
    String::new()
}

/// A loaded model with llama-server's task loop running on its own thread.
/// Dropping it stops the loop and frees the model (unload).
///
/// Requests may come from any thread; the server queues them for its one
/// slot (`-np 1`), as the sidecar does.
pub struct Engine {
    #[cfg(not(wb_ios_llm_stub))]
    handle: std::ptr::NonNull<ffi::wb_llm>,
    model_bytes: u64,
    context_tokens: u32,
}

// SAFETY: the handle is an opaque pointer to state that llama-server guards
// with its own queue and mutexes; `wb_llm_request` is safe to call from
// several threads, and `Drop` needs `&mut self`.
unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

impl Engine {
    /// Loads the model (memory-mapped) and starts the task loop. Blocks for
    /// the load: call it off the main thread.
    pub fn load(config: &Config) -> Result<Self, Error> {
        #[cfg(wb_ios_llm_stub)]
        {
            let _ = config;
            Err(Error::Unsupported)
        }
        #[cfg(not(wb_ios_llm_stub))]
        {
            use std::ffi::CString;
            let args: Vec<CString> = config
                .server_args()
                .into_iter()
                .map(|a| CString::new(a).map_err(|_| Error::Load("NUL in an argument".into())))
                .collect::<Result<_, _>>()?;
            let argv: Vec<*const std::os::raw::c_char> = args.iter().map(|a| a.as_ptr()).collect();
            let mut err = [0u8; 512];
            // SAFETY: argv holds `argc` valid C strings for the call; `err`
            // is writable for its length.
            let handle = unsafe {
                ffi::wb_llm_start(
                    argv.len() as i32,
                    argv.as_ptr(),
                    err.as_mut_ptr().cast(),
                    err.len(),
                )
            };
            let Some(handle) = std::ptr::NonNull::new(handle) else {
                let end = err.iter().position(|b| *b == 0).unwrap_or(err.len());
                let message = String::from_utf8_lossy(&err[..end]).into_owned();
                return Err(if message.contains("already running") {
                    Error::Busy
                } else {
                    Error::Load(message)
                });
            };
            // SAFETY: a live handle.
            let (model_bytes, context_tokens) = unsafe {
                (
                    ffi::wb_llm_model_bytes(handle.as_ptr()),
                    ffi::wb_llm_n_ctx(handle.as_ptr()).max(0) as u32,
                )
            };
            Ok(Self {
                handle,
                model_bytes,
                context_tokens,
            })
        }
    }

    /// Size of the model's tensors in bytes (what Metal holds once loaded).
    pub fn model_bytes(&self) -> u64 {
        self.model_bytes
    }

    /// Context size of the slot in tokens.
    pub fn context_tokens(&self) -> u32 {
        self.context_tokens
    }

    /// One request through llama-server's handler for `path`. `body` is the
    /// JSON the Mac would POST (empty for GET routes). `sink` gets the
    /// response body: whole for a plain response, piece by piece for a
    /// stream (`"stream": true`: server-sent events, ending in
    /// `data: [DONE]`). Returns the HTTP status llama-server would send.
    ///
    /// Blocks until the response ends or `stop` says so. After a stop the
    /// bytes so far have been delivered, the stream simply ends early (no
    /// `[DONE]`), like a closed connection.
    pub fn request(
        &self,
        path: &str,
        body: &str,
        stop: &Stop,
        sink: impl FnMut(&[u8]),
    ) -> Result<u16, Error> {
        self.request_with(path, body, stop, |_, _| {}, sink)
    }

    /// [`Engine::request`] with the response head: `head(status, is_stream)`
    /// runs once before the first byte reaches `sink` (a listener needs the
    /// status before it can write headers).
    pub fn request_with(
        &self,
        path: &str,
        body: &str,
        stop: &Stop,
        mut head: impl FnMut(u16, bool),
        mut sink: impl FnMut(&[u8]),
    ) -> Result<u16, Error> {
        #[cfg(wb_ios_llm_stub)]
        {
            let _ = (path, body, stop, &mut head, &mut sink);
            Err(Error::Unsupported)
        }
        #[cfg(not(wb_ios_llm_stub))]
        {
            use std::os::raw::{c_char, c_int, c_void};
            struct Call<'a> {
                stop: &'a Stop,
                head: &'a mut dyn FnMut(u16, bool),
                sink: &'a mut dyn FnMut(&[u8]),
                panicked: bool,
            }
            extern "C" fn on_head(user: *mut c_void, status: c_int, is_stream: c_int) {
                // SAFETY: `user` is the `Call` below, alive for the request.
                let call = unsafe { &mut *(user as *mut Call<'_>) };
                let status = u16::try_from(status).unwrap_or(500);
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    (call.head)(status, is_stream != 0)
                }))
                .is_err()
                {
                    call.panicked = true;
                }
            }
            extern "C" fn on_stop(user: *mut c_void) -> c_int {
                // SAFETY: `user` is the `Call` below, alive for the request.
                let call = unsafe { &*(user as *const Call<'_>) };
                c_int::from(call.panicked || call.stop.should_stop())
            }
            extern "C" fn on_data(user: *mut c_void, data: *const c_char, len: usize) {
                // SAFETY: as above; `data` is valid for `len` bytes.
                let call = unsafe { &mut *(user as *mut Call<'_>) };
                let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), len) };
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (call.sink)(bytes)))
                    .is_err()
                {
                    call.panicked = true;
                }
            }
            let path_c =
                std::ffi::CString::new(path).map_err(|_| Error::UnknownPath(path.to_owned()))?;
            let mut call = Call {
                stop,
                head: &mut head,
                sink: &mut sink,
                panicked: false,
            };
            // SAFETY: a live handle; the callbacks only run during the call,
            // on this thread, with `call` alive.
            let status = unsafe {
                ffi::wb_llm_request(
                    self.handle.as_ptr(),
                    path_c.as_ptr(),
                    body.as_ptr().cast(),
                    body.len(),
                    on_stop,
                    on_head,
                    on_data,
                    (&mut call as *mut Call<'_>).cast(),
                )
            };
            match status {
                -1 => Err(Error::UnknownPath(path.to_owned())),
                s if s < 0 => Err(Error::Stopped),
                s => Ok(s as u16),
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        #[cfg(not(wb_ios_llm_stub))]
        // SAFETY: the handle is live and no request borrows `self`.
        unsafe {
            ffi::wb_llm_stop(self.handle.as_ptr())
        }
    }
}

#[cfg(not(wb_ios_llm_stub))]
mod ffi {
    use std::os::raw::{c_char, c_int, c_void};

    #[repr(C)]
    pub struct wb_llm {
        _private: [u8; 0],
    }

    pub type StopFn = extern "C" fn(*mut c_void) -> c_int;
    pub type HeadFn = extern "C" fn(*mut c_void, c_int, c_int);
    pub type DataFn = extern "C" fn(*mut c_void, *const c_char, usize);

    extern "C" {
        pub fn wb_llm_start(
            argc: c_int,
            argv: *const *const c_char,
            err: *mut c_char,
            err_len: usize,
        ) -> *mut wb_llm;
        pub fn wb_llm_request(
            h: *mut wb_llm,
            path: *const c_char,
            body: *const c_char,
            body_len: usize,
            stop: StopFn,
            head: HeadFn,
            data: DataFn,
            user: *mut c_void,
        ) -> c_int;
        pub fn wb_llm_model_bytes(h: *const wb_llm) -> u64;
        pub fn wb_llm_n_ctx(h: *const wb_llm) -> i32;
        pub fn wb_llm_stop(h: *mut wb_llm);
        pub fn wb_llm_set_verbosity(level: c_int);
        pub fn wb_llm_build_info() -> *const c_char;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_the_sidecars_without_the_listener() {
        let mut c = Config::new("/m.gguf");
        c.threads = 6;
        assert_eq!(
            c.server_args(),
            [
                "llama-server",
                "-m",
                "/m.gguf",
                "--alias",
                "local",
                "-c",
                "4096",
                "-np",
                "1",
                "--threads",
                "6",
                "--cache-ram",
                "256",
                "--reasoning",
                "off",
                "--jinja",
                "--no-ui",
                "--no-slots",
                "--offline",
                "--fit",
                "off",
            ]
        );
        c.device = Device::Cpu;
        let cpu = c.server_args();
        assert_eq!(&cpu[cpu.len() - 2..], ["-ngl", "0"]);
    }

    #[test]
    fn stop_by_flag_and_by_deadline() {
        let s = Stop::default();
        assert!(!s.should_stop());
        s.cancel();
        assert!(s.should_stop());
        let past = Stop::with_deadline(Instant::now());
        assert!(past.should_stop());
        let flag = Arc::new(AtomicBool::new(false));
        let shared = Stop::with_flag(flag.clone(), None);
        flag.store(true, Ordering::SeqCst);
        assert!(shared.is_cancelled());
    }

    #[test]
    fn a_missing_model_is_a_load_error() {
        let r = Engine::load(&Config::new("/nonexistent/model.gguf"));
        assert!(matches!(r, Err(Error::Load(_))), "{:?}", r.err());
    }

    #[test]
    fn memory_queries_answer() {
        assert!(physical_memory_bytes() >= 1 << 30);
        assert!(footprint_bytes().is_some_and(|b| b > 0));
    }
}
