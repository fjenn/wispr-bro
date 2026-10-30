// SPDX-License-Identifier: GPL-3.0-or-later
//! First-word clipping, judged per take from the data instead of by ear
//! (RS-01 round 2, and `--analyze` for an earlier run's takes):
//!
//! 1. fn-down time and first captured sample time (from the run's log);
//! 2. speech onset in the saved audio (`onset.rs`);
//! 3. speech already present in the first captured frames → clipped by us;
//!    leading silence → the speaker started after the mic was live;
//! 4. the RS-09 bench binary (`wb-bench-asr run`, parakeet-unified-en) says
//!    whether the first word "testing" survived.
//!
//! Reads WAVs and runs the bench binary (and ffmpeg for a WAV that is not
//! 16 kHz mono); no network.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::onset::{self, Clip, Onset};

#[derive(Clone, Debug)]
pub struct TakeInput {
    /// "a: cold mic" etc.; round 1's takes use the round name.
    pub variant: String,
    pub n: u32,
    pub wav: Option<PathBuf>,
    pub fn_down_ms: f64,
    pub first_audio_ms: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TakeResult {
    pub variant: String,
    pub n: u32,
    pub wav: Option<String>,
    /// fn press → first captured sample.
    pub fn_to_first_audio_ms: Option<f64>,
    pub onset: Option<Onset>,
    /// fn press → speech onset (first audio + leading zeros + onset).
    pub speech_after_fn_ms: Option<f64>,
    pub clip: Clip,
    pub asr_text: Option<String>,
    /// The transcript starts with "test…" (None: not transcribed).
    pub testing_survived: Option<bool>,
}

/// Whether a transcript of "testing one two three" kept its first word.
pub fn testing_survived(text: &str) -> bool {
    text.split_whitespace()
        .next()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
                .starts_with("test")
        })
        .unwrap_or(false)
}

/// 16 kHz mono f32 samples, via ffmpeg if the file is anything else.
pub fn read_16k_mono(path: &Path, scratch: &Path) -> Result<Vec<f32>, String> {
    let direct = |p: &Path| -> Result<(hound::WavSpec, Vec<f32>), String> {
        let mut r = hound::WavReader::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let spec = r.spec();
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Int => {
                let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
                r.samples::<i32>()
                    .map(|s| s.map(|v| v as f32 / scale))
                    .collect::<Result<_, _>>()
                    .map_err(|e| e.to_string())?
            }
            hound::SampleFormat::Float => r
                .samples::<f32>()
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?,
        };
        Ok((spec, samples))
    };
    let (spec, samples) = direct(path)?;
    if spec.sample_rate == 16_000 && spec.channels == 1 {
        return Ok(samples);
    }
    let _ = std::fs::create_dir_all(scratch);
    let out = scratch.join(format!(
        "{}-16k.wav",
        path.file_stem().and_then(|s| s.to_str()).unwrap_or("take")
    ));
    let status = std::process::Command::new(ffmpeg())
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(path)
        .args(["-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le"])
        .arg(&out)
        .status()
        .map_err(|e| format!("ffmpeg (needed for {}): {e}", path.display()))?;
    if !status.success() {
        return Err(format!("ffmpeg could not convert {}", path.display()));
    }
    Ok(direct(&out)?.1)
}

fn ffmpeg() -> PathBuf {
    ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("ffmpeg"))
}

/// The RS-09 bench binary and English model.
pub struct Asr {
    pub bin: PathBuf,
    pub model: PathBuf,
}

/// The repo's `src-tauri` folder (this crate lives in `src-tauri/crates/`).
fn src_tauri() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub const BUILD_HINT: &str = "cd ~/Projects/wispr-bro/src-tauri/crates/wb-bench-asr && \
     CMAKE_POLICY_VERSION_MINIMUM=3.5 CARGO_TARGET_DIR=../../target cargo build --release --offline";

/// Finds the bench binary and model (`WB_BENCH_ASR` / `WB_ASR_MODEL`
/// override). The error says what is missing and how to fix it.
pub fn locate_asr() -> Result<Asr, String> {
    let bin = std::env::var_os("WB_BENCH_ASR")
        .map(PathBuf::from)
        .unwrap_or_else(|| src_tauri().join("target/release/wb-bench-asr"));
    let model = std::env::var_os("WB_ASR_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join("wb-bench/asr/parakeet-unified-en-0.6b/parakeet-unified-en-0.6b-Q8_0.gguf")
        });
    if !bin.exists() {
        return Err(format!(
            "the speech bench binary is missing ({}). Build it with:\n    {BUILD_HINT}",
            bin.display()
        ));
    }
    if !model.exists() {
        return Err(format!(
            "the English speech model is missing ({}); see docs/fork/spikes/RS-09-asr-bench.md",
            model.display()
        ));
    }
    Ok(Asr { bin, model })
}

/// Transcribes every clip in one bench run (one model load): the clips are
/// joined with 0.5 s of silence into `work/asr-input.wav` and each is a job.
pub fn transcribe(
    asr: &Asr,
    clips: &[(String, Vec<f32>)],
    work: &Path,
) -> Result<HashMap<String, String>, String> {
    if clips.is_empty() {
        return Ok(HashMap::new());
    }
    std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
    let mut all: Vec<f32> = Vec::new();
    let mut jobs = Vec::new();
    for (id, s) in clips {
        let start = all.len();
        all.extend_from_slice(s);
        jobs.push(json!({"id": id, "start": start, "end": all.len(), "language": "en"}));
        all.extend(std::iter::repeat_n(0.0, 8_000));
    }
    let wav = work.join("asr-input.wav");
    crate::mic::write_wav(&wav, &all, 16_000).map_err(|e| e.to_string())?;
    let jobs_file = work.join("asr-jobs.json");
    std::fs::write(&jobs_file, json!({"jobs": jobs}).to_string()).map_err(|e| e.to_string())?;
    let out = work.join("asr-out.json");
    let run = std::process::Command::new(&asr.bin)
        .arg("run")
        .arg("--model")
        .arg(&asr.model)
        .arg("--wav")
        .arg(&wav)
        .arg("--jobs")
        .arg(&jobs_file)
        .arg("--out")
        .arg(&out)
        .output()
        .map_err(|e| format!("{}: {e}", asr.bin.display()))?;
    if !run.status.success() {
        return Err(format!(
            "the speech bench failed: {}",
            String::from_utf8_lossy(&run.stderr).trim()
        ));
    }
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&out).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Ok(parse_results(&v))
}

fn parse_results(v: &Value) -> HashMap<String, String> {
    v["results"]
        .as_array()
        .map(|rs| {
            rs.iter()
                .filter_map(|r| {
                    Some((
                        r["id"].as_str()?.to_string(),
                        r["text"].as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Onset + classification for every take, then one ASR pass. `asr` errors
/// (missing binary, …) leave the transcripts empty and come back as a note.
pub fn analyse(takes: &[TakeInput], work: &Path) -> (Vec<TakeResult>, Option<String>) {
    let mut out = Vec::new();
    let mut clips = Vec::new();
    for t in takes {
        let id = format!("{}-{:02}", crate::round2::slug(&t.variant), t.n);
        let samples = t
            .wav
            .as_ref()
            .and_then(|p| read_16k_mono(p, &work.join("16k")).ok());
        let o = samples.as_ref().and_then(|s| onset::analyse(s, 16_000));
        let latency = t.first_audio_ms.map(|f| f - t.fn_down_ms);
        let speech_after_fn = match (&o, latency) {
            (Some(o), Some(l)) => o.onset_ms.map(|on| l + o.lead_zero_ms + on),
            _ => None,
        };
        if let Some(s) = samples {
            clips.push((id.clone(), s));
        }
        out.push((
            id,
            TakeResult {
                variant: t.variant.clone(),
                n: t.n,
                wav: t.wav.as_ref().map(|p| p.display().to_string()),
                fn_to_first_audio_ms: latency,
                clip: onset::classify(o.as_ref()),
                onset: o,
                speech_after_fn_ms: speech_after_fn,
                asr_text: None,
                testing_survived: None,
            },
        ));
    }
    let note = match locate_asr().and_then(|asr| transcribe(&asr, &clips, work)) {
        Ok(texts) => {
            for (id, r) in &mut out {
                if let Some(t) = texts.get(id) {
                    r.testing_survived = Some(testing_survived(t));
                    r.asr_text = Some(t.clone());
                }
            }
            None
        }
        Err(e) => Some(e),
    };
    (out.into_iter().map(|(_, r)| r).collect(), note)
}

/// The clipping takes recorded in an earlier run's `summary.json`.
pub fn takes_from_summary(v: &Value) -> Vec<TakeInput> {
    let mut out = Vec::new();
    for b in v["blocks"].as_array().into_iter().flatten() {
        if b["kind"] != "clipping" {
            continue;
        }
        let variant = b["round"].as_str().unwrap_or("").to_string();
        for e in b["episodes"].as_array().into_iter().flatten() {
            let Some(down) = e["presses"][0]["down_ms"].as_f64() else {
                continue;
            };
            let m = &e["mic"][0];
            out.push(TakeInput {
                variant: variant.clone(),
                n: e["n"].as_u64().unwrap_or(0) as u32,
                wav: m["wav"].as_str().map(PathBuf::from),
                fn_down_ms: down,
                first_audio_ms: m["first_audio_ms"].as_f64(),
            });
        }
    }
    out
}

/// Counts per variant, in the order variants first appear.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VariantStats {
    pub variant: String,
    pub takes: usize,
    pub clipped_by_us: usize,
    pub not_clipped: usize,
    pub no_speech: usize,
    pub no_audio: usize,
    pub transcribed: usize,
    pub testing_survived: usize,
    pub latency_p50_ms: Option<f64>,
    pub latency_p95_ms: Option<f64>,
    pub speech_after_fn_p50_ms: Option<f64>,
    pub speech_after_fn_min_ms: Option<f64>,
}

pub fn stats(results: &[TakeResult]) -> Vec<VariantStats> {
    let mut order: Vec<String> = Vec::new();
    for r in results {
        if !order.contains(&r.variant) {
            order.push(r.variant.clone());
        }
    }
    order
        .into_iter()
        .map(|v| {
            let rs: Vec<&TakeResult> = results.iter().filter(|r| r.variant == v).collect();
            let count = |c: Clip| rs.iter().filter(|r| r.clip == c).count();
            let mut lat: Vec<f64> = rs.iter().filter_map(|r| r.fn_to_first_audio_ms).collect();
            lat.sort_by(|a, b| a.total_cmp(b));
            let mut sp: Vec<f64> = rs.iter().filter_map(|r| r.speech_after_fn_ms).collect();
            sp.sort_by(|a, b| a.total_cmp(b));
            VariantStats {
                takes: rs.len(),
                clipped_by_us: count(Clip::ClippedByUs),
                not_clipped: count(Clip::NotClipped),
                no_speech: count(Clip::NoSpeech),
                no_audio: count(Clip::NoAudio),
                transcribed: rs.iter().filter(|r| r.testing_survived.is_some()).count(),
                testing_survived: rs
                    .iter()
                    .filter(|r| r.testing_survived == Some(true))
                    .count(),
                latency_p50_ms: crate::summary::percentile(&lat, 50.0),
                latency_p95_ms: crate::summary::percentile(&lat, 95.0),
                speech_after_fn_p50_ms: crate::summary::percentile(&sp, 50.0),
                speech_after_fn_min_ms: sp.first().copied(),
                variant: v,
            }
        })
        .collect()
}

pub fn render_takes(results: &[TakeResult]) -> String {
    let mut s = String::from(
        "  variant              #  fn→audio  speech after fn  verdict          transcript\n",
    );
    for r in results {
        let f = |x: Option<f64>| x.map(|v| format!("{v:.0} ms")).unwrap_or("-".into());
        s.push_str(&format!(
            "  {:<19} {:>2}  {:>8}  {:>15}  {:<15}  {}\n",
            r.variant,
            r.n,
            f(r.fn_to_first_audio_ms),
            f(r.speech_after_fn_ms),
            r.clip.word(),
            r.asr_text.as_deref().unwrap_or("(not transcribed)")
        ));
    }
    s
}

pub fn render_stats(st: &[VariantStats]) -> String {
    let mut s = String::new();
    for v in st {
        let f = |x: Option<f64>| x.map(|v| format!("{v:.0}")).unwrap_or("-".into());
        s.push_str(&format!(
            "  {}: {} take(s); clipped by us {}, not clipped {}, no speech {}, no audio {}; \
             \"testing\" kept {}/{} transcribed; fn→audio p50 {} / p95 {} ms; \
             speech began p50 {} ms (earliest {} ms) after the press\n",
            v.variant,
            v.takes,
            v.clipped_by_us,
            v.not_clipped,
            v.no_speech,
            v.no_audio,
            v.testing_survived,
            v.transcribed,
            f(v.latency_p50_ms),
            f(v.latency_p95_ms),
            f(v.speech_after_fn_p50_ms),
            f(v.speech_after_fn_min_ms),
        ));
    }
    s
}

/// `wb-spike-fn --analyze DIR`: the same analysis for a finished run.
pub fn analyze_dir(dir: &Path) -> i32 {
    let summary = dir.join("summary.json");
    let v: Value = match std::fs::read_to_string(&summary)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Could not read {}: {e}", summary.display());
            return 1;
        }
    };
    let takes = takes_from_summary(&v);
    if takes.is_empty() {
        eprintln!("No clipping takes in {}", summary.display());
        return 1;
    }
    let (results, note) = analyse(&takes, &dir.join("analysis"));
    let st = stats(&results);
    println!("First-word clipping, per take ({}):\n", dir.display());
    print!("{}", render_takes(&results));
    println!();
    print!("{}", render_stats(&st));
    if let Some(n) = &note {
        println!("\n  Transcripts skipped: {n}");
    }
    let out = dir.join("analysis.json");
    let doc = json!({"takes": results, "stats": st, "asr_note": note,
                     "rule": format!("speech within {} ms of the first captured sample = clipped by us", onset::AT_START_MS)});
    match std::fs::write(&out, serde_json::to_string_pretty(&doc).unwrap_or_default()) {
        Ok(()) => {
            println!("\nSaved: {}", out.display());
            0
        }
        Err(e) => {
            eprintln!("Could not write {}: {e}", out.display());
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_word_check() {
        assert!(testing_survived("Testing, one, two, three."));
        assert!(testing_survived("testing one two three"));
        assert!(testing_survived("\"Test one two three\""));
        assert!(!testing_survived("One, two, three, ten."));
        assert!(!testing_survived("Destiny three."));
        assert!(!testing_survived(""));
    }

    #[test]
    fn bench_output_is_parsed() {
        let v = json!({"results": [{"id": "a-01", "text": "Testing one."}, {"id": "b-01", "text": "Two."}]});
        let m = parse_results(&v);
        assert_eq!(m["a-01"], "Testing one.");
        assert_eq!(m.len(), 2);
        assert!(parse_results(&json!({})).is_empty());
    }

    #[test]
    fn takes_come_from_the_clipping_blocks_only() {
        let v = json!({"blocks": [
            {"kind": "hold", "round": "x", "episodes": [{"n": 1, "presses": [{"down_ms": 5}], "mic": [{}]}]},
            {"kind": "clipping", "round": "Show Emoji & Symbols", "episodes": [
                {"n": 1, "presses": [{"down_ms": 1000, "up_ms": 3000}],
                 "mic": [{"first_audio_ms": 1262.5, "wav": "/x/take-01-62.wav"}]}]}
        ]});
        let t = takes_from_summary(&v);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].fn_down_ms, 1000.0);
        assert_eq!(t[0].first_audio_ms, Some(1262.5));
        assert_eq!(t[0].wav.as_deref(), Some(Path::new("/x/take-01-62.wav")));
    }

    #[test]
    fn stats_count_per_variant() {
        let r = |v: &str, clip, lat, kept| TakeResult {
            variant: v.into(),
            n: 1,
            wav: None,
            fn_to_first_audio_ms: Some(lat),
            onset: None,
            speech_after_fn_ms: Some(lat + 300.0),
            clip,
            asr_text: None,
            testing_survived: kept,
        };
        let st = stats(&[
            r("a", Clip::ClippedByUs, 210.0, Some(false)),
            r("a", Clip::NotClipped, 220.0, Some(true)),
            r("b", Clip::NotClipped, 120.0, None),
        ]);
        assert_eq!(st.len(), 2);
        assert_eq!(st[0].variant, "a");
        assert_eq!((st[0].clipped_by_us, st[0].not_clipped), (1, 1));
        assert_eq!((st[0].transcribed, st[0].testing_survived), (2, 1));
        assert_eq!(st[1].latency_p50_ms, Some(120.0));
        assert_eq!(st[1].speech_after_fn_min_ms, Some(420.0));
    }
}
