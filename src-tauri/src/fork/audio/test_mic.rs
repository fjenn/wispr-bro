// SPDX-License-Identifier: GPL-3.0-or-later
//! A test microphone fed from a WAV file (Mac waves, test builds only).
//!
//! `WISPRBRO_TEST_MIC_WAV=<path>` makes the app's microphone play that file
//! in real time, then silence, every time a take opens or starts the mic
//! (the vendored recorder's `set_file_source`, `crates/wb-handy-recorder`).
//! The file is read again at each take, so a driver may copy the next
//! take's WAV over it between takes (`wb_mic_load`, `scripts/wb-lib/app.sh`).
//! Everything after the device is the real pipeline: resampling, Handy's
//! VAD and levels, the Flow Bar, transcription and delivery. So a scripted
//! take needs no speakers and no live room (the owner's rule: pre-recorded
//! audio, never speakers).
//!
//! A missing or unreadable file plays silence; the real microphone is never
//! opened while the variable is set.
//!
//! Honoured only by a test identity (`com.wisprbro.<slice>check`) or a debug
//! build, and never by the shipping identifier `com.wisprbro.desktop`
//! ([`allowed`]). Set from `fork::early_init`, before any recorder opens.

use std::path::Path;
use std::sync::OnceLock;

/// The environment variable.
pub const ENV: &str = "WISPRBRO_TEST_MIC_WAV";

/// What [`init`] did, logged once the logger runs ([`log_status`]).
static STATUS: OnceLock<String> = OnceLock::new();

/// `true` when a build with `identifier` may replace its microphone.
pub fn allowed(identifier: &str, debug_build: bool) -> bool {
    identifier != crate::fork::settings::PRODUCT_IDENTIFIER
        && (crate::fork::is_test_identity(identifier) || debug_build)
}

/// Reads [`ENV`] and, when [`allowed`], installs the file as the
/// microphone. Returns whether it did.
pub fn init(identifier: &str) -> bool {
    let Some(path) = std::env::var_os(ENV).filter(|p| !p.is_empty()) else {
        return false;
    };
    if !allowed(identifier, cfg!(debug_assertions)) {
        let _ = STATUS.set(format!(
            "fork audio: {ENV} ignored ({identifier} is not a test build)"
        ));
        return false;
    }
    let path = std::path::PathBuf::from(path);
    let _ = STATUS.set(format!(
        "fork audio: test microphone plays {} at each take ({})",
        path.display(),
        match load(&path) {
            Ok(source) => format!(
                "now {:.1} s at {} Hz",
                source.samples.len() as f64 / f64::from(source.sample_rate),
                source.sample_rate
            ),
            Err(e) => format!("not readable yet: {e}"),
        }
    ));
    handy_recorder::set_file_source(Some(std::sync::Arc::new(move || {
        Some(source_or_silence(&path))
    })));
    true
}

/// The file's samples, or a moment of silence when it cannot be read (a
/// driver swapping it): once the variable is set the real microphone is
/// never opened.
fn source_or_silence(path: &Path) -> handy_recorder::FileSource {
    load(path).unwrap_or_else(|e| {
        log::warn!("fork audio: test microphone file: {e}; silence");
        handy_recorder::FileSource {
            samples: Vec::new(),
            sample_rate: 16_000,
            label: "silence".into(),
        }
    })
}

/// Logs what [`init`] did (from `fork::on_ready`).
pub fn log_status() {
    if let Some(status) = STATUS.get() {
        if handy_recorder::file_source_active() {
            log::info!("{status}");
        } else {
            log::warn!("{status}");
        }
    }
}

/// Decodes a WAV (integer or float, any channel count) to mono samples.
pub fn load(path: &Path) -> Result<handy_recorder::FileSource, String> {
    let reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels.max(1));
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?,
        hound::SampleFormat::Int => {
            let scale = 2f32.powi(i32::from(spec.bits_per_sample.clamp(1, 32)) - 1);
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|s| s as f32 / scale))
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?
        }
    };
    let samples = interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect();
    Ok(handy_recorder::FileSource {
        samples,
        sample_rate: spec.sample_rate,
        label: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_test_builds_may_replace_the_microphone() {
        // The shipping identity never, debug build or not.
        assert!(!allowed("com.wisprbro.desktop", false));
        assert!(!allowed("com.wisprbro.desktop", true));
        // Test identities, release or debug.
        assert!(allowed("com.wisprbro.mac2check", false));
        assert!(allowed("com.wisprbro.mac2relcheck", false));
        // Anything else only as a debug build (plain Handy dev runs).
        assert!(allowed("com.pais.handy", true));
        assert!(!allowed("com.pais.handy", false));
        assert!(!allowed("com.wisprbro.check", false), "no slice name");
        assert!(!allowed("com.wisprbro.desktopx", false));
    }

    #[test]
    fn the_shipping_identity_ignores_the_variable() {
        // `init` reads the process environment; the gate decides first, so
        // this never installs a source (other tests' recorders stay real).
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("take.wav");
        write_wav(&wav, 2, &[0.5, -0.5, 0.25, 0.25]);
        // SAFETY (test): no other test reads this variable.
        std::env::set_var(ENV, &wav);
        assert!(!init("com.wisprbro.desktop"));
        std::env::remove_var(ENV);
        assert!(!handy_recorder::file_source_active());
    }

    #[test]
    fn a_wav_decodes_to_mono_at_its_rate() {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("take.wav");
        write_wav(&wav, 2, &[0.5, -0.5, 0.25, 0.25]);
        let source = load(&wav).unwrap();
        assert_eq!(source.sample_rate, 22_050);
        assert_eq!(source.label, "take.wav");
        assert_eq!(source.samples.len(), 2);
        assert!(source.samples[0].abs() < 1e-3);
        assert!((source.samples[1] - 0.25).abs() < 1e-3);
        assert!(load(&dir.path().join("missing.wav")).is_err());
        let silence = source_or_silence(&dir.path().join("missing.wav"));
        assert!(silence.samples.is_empty());
        assert_eq!(silence.sample_rate, 16_000);
    }

    fn write_wav(path: &Path, channels: u16, samples: &[f32]) {
        let spec = hound::WavSpec {
            channels,
            sample_rate: 22_050,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for s in samples {
            w.write_sample((s * 32_767.0) as i16).unwrap();
        }
        w.finalize().unwrap();
    }
}
