// SPDX-License-Identifier: GPL-3.0-or-later
//! CSP smoke support (F-08, DEC-43).
//!
//! `fork_csp_smoke_media` hands the CSP smoke check (`src/fork/debug/
//! cspSmokeChecks.ts`, run from the debug panel and from the `WISPRBRO_SELFCHECK`
//! script) a local audio file to play through `convertFileSrc(…, "asset")`:
//! the newest recording if there is one, else a tiny silent WAV fixture
//! written into the app data dir (inside the asset scope `$APPDATA/**`).
//! Available only in debug builds, with `--debug`, or while the self-check
//! is on.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::ForkError;

/// File name of the fixture in the app data dir.
pub const FIXTURE_FILE: &str = "fork-csp-smoke.wav";

/// Reply of [`fork_csp_smoke_media`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ForkCspMedia {
    /// Absolute path of the audio file.
    pub path: String,
    /// `"recording"` or `"fixture"`.
    pub source: String,
}

/// 0.1 s of 16 kHz mono 16-bit silence.
pub fn fixture_wav() -> Vec<u8> {
    let samples: u32 = 1600;
    let data_len = samples * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&16_000u32.to_le_bytes());
    out.extend_from_slice(&32_000u32.to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.resize(44 + data_len as usize, 0);
    out
}

/// Newest `*.wav` in `dir`, if any.
pub fn newest_recording(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "wav"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

fn allowed<R: Runtime>(app: &AppHandle<R>) -> bool {
    cfg!(debug_assertions)
        || super::selfcheck::enabled()
        || app
            .try_state::<crate::CliArgs>()
            .map(|a| a.debug)
            .unwrap_or(false)
}

/// See the module docs.
#[tauri::command]
#[specta::specta]
pub fn fork_csp_smoke_media<R: Runtime>(app: AppHandle<R>) -> Result<ForkCspMedia, ForkError> {
    if !allowed(&app) {
        return Err(ForkError::Internal("CSP smoke is debug-only".into()));
    }
    let data = match crate::portable::data_dir() {
        Some(dir) => dir.clone(),
        None => app
            .path()
            .app_data_dir()
            .map_err(|e| ForkError::Internal(format!("no app data dir: {e}")))?,
    };
    if let Some(path) = newest_recording(&data.join("recordings")) {
        return Ok(ForkCspMedia {
            path: path.to_string_lossy().into_owned(),
            source: "recording".into(),
        });
    }
    std::fs::create_dir_all(&data).map_err(|e| ForkError::Internal(e.to_string()))?;
    let path = data.join(FIXTURE_FILE);
    std::fs::write(&path, fixture_wav()).map_err(|e| ForkError::Internal(e.to_string()))?;
    Ok(ForkCspMedia {
        path: path.to_string_lossy().into_owned(),
        source: "fixture".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_is_a_valid_wav() {
        let bytes = fixture_wav();
        let reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
        let spec = reader.spec();
        assert_eq!(
            (spec.channels, spec.sample_rate, spec.bits_per_sample),
            (1, 16_000, 16)
        );
        assert_eq!(reader.duration(), 1600);
    }

    #[test]
    fn newest_recording_picks_the_latest_wav() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(newest_recording(dir.path()), None);
        std::fs::write(dir.path().join("a.wav"), b"x").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(dir.path().join("b.wav"), b"x").unwrap();
        std::fs::write(dir.path().join("c.txt"), b"x").unwrap();
        assert_eq!(newest_recording(dir.path()), Some(dir.path().join("b.wav")));
    }
}
