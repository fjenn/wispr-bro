//! transcribe.cpp inside Handy's own process, for the small CPU-only models:
//! Silero VAD and the spoken-language detector. Everything that can touch a
//! GPU runs in the engine worker instead (`engine_supervisor`).

use anyhow::Result;
use std::path::Path;
use std::sync::OnceLock;

#[cfg(not(target_os = "ios"))]
use transcribe_cpp::BackendMask;

/// Register transcribe.cpp's CPU backends once per process. The allowed mask
/// is fixed at first registration, so it must never include a GPU here.
#[cfg(not(target_os = "ios"))]
pub(crate) fn init_cpu_backends() -> Result<()> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        // Route native/ggml diagnostics into Handy's log; must precede any
        // native call.
        transcribe_cpp::init_logging();
        transcribe_cpp::init_backends_with(None::<&Path>, BackendMask::CPU)
            .map_err(|e| e.to_string())
    })
    .clone()
    .map_err(|e| anyhow::anyhow!("Failed to initialize transcribe.cpp CPU backend: {e}"))
}

/// fork (iOS): the speech engine shares this process (an app cannot start a
/// worker; `engine_supervisor`), and the allowed mask is fixed at the first
/// registration. So every backend is registered, once, whoever asks first;
/// the VAD and the language detector still load on the CPU because they ask
/// for it (`Backend::Cpu`).
#[cfg(target_os = "ios")]
pub(crate) fn init_cpu_backends() -> Result<()> {
    init_all_backends()
        .map_err(|e| anyhow::anyhow!("Failed to initialize transcribe.cpp backends: {e}"))
}

/// iOS: registers all of transcribe.cpp's backends once for the process.
#[cfg(target_os = "ios")]
pub(crate) fn init_all_backends() -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        transcribe_cpp::init_logging();
        transcribe_cpp::init_backends_default().map_err(|e| e.to_string())
    })
    .clone()
}
