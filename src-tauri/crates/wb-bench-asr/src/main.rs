// SPDX-License-Identifier: GPL-3.0-or-later
//! RS-09 speech-model bench (R5 §7 RS-09, DEC-51). Not product code.
//!
//! Drives transcribe-cpp 0.4.0 exactly the way the app's engine worker does
//! (`src-tauri/src/engine_supervisor/worker.rs`): `init_backends_default()`
//! (or the CPU-only mask for the CPU path), `Model::load_with` with
//! `Backend::Auto` (Metal on Apple Silicon) or `Backend::Cpu`, one
//! `Session`, and `RunOptions { language, ..Default::default() }` with the
//! language passed only when the model advertises it (Handy's
//! `transcribe_cpp_run_plan`). Streams use `StreamOptions::default()` and
//! 480-sample feeds, and take `text().full` after `finalize` (Handy's
//! `live.rs`). Long-form chunking uses Handy's own `Chunker` and
//! `join_transcripts`, fed with Silero VAD probabilities scored by Handy's
//! `SileroVad` in a separate `vad` run (a process can register only one
//! backend mask, and the app scores VAD on the CPU outside the worker too).
//!
//! Every subcommand writes one JSON document to `--out`. Scoring, splitting
//! and the report tables live in `scripts/wb-bench-asr.ts`.
//!
//! ```text
//! wb-bench-asr vad    --wav W --vad SILERO.gguf [--start S --end E] --out O
//! wb-bench-asr run    --model M.gguf [--cpu] --wav W --jobs JOBS.json --out O
//! wb-bench-asr chunk  --model M.gguf [--cpu] --wav W --start S --end E
//!                     [--language L] --probs PROBS.json [--push 480] --out O
//! wb-bench-asr stream --model M.gguf [--cpu] --wav W --start S --end E
//!                     [--language L] --out O
//! ```
//! `S`/`E` are sample offsets in the 16 kHz mono WAV.

use anyhow::{anyhow, bail, Context, Result};
use handy_app_lib::audio_toolkit::chunking::{join_transcripts, Chunker};
use handy_app_lib::audio_toolkit::vad::{SpeechScorer, VadFrame, VoiceActivityDetector};
use handy_app_lib::audio_toolkit::{read_wav_samples, SileroVad};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use transcribe_cpp::{
    Backend, BackendMask, Feature, Model, ModelOptions, RunOptions, Session, StreamOptions,
    TimestampKind,
};

const RATE: f64 = 16_000.0;
/// Handy's recorder frame, as the live stream is fed (supervisor tests'
/// `feed_paced`, recorder callback).
const STREAM_FEED: usize = 480;

fn main() {
    if let Err(e) = real_main() {
        eprintln!("wb-bench-asr: {e:#}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<()> {
    let args = Args::parse(std::env::args().skip(1).collect())?;
    let out = PathBuf::from(args.req("out")?);
    let doc = match args.cmd.as_str() {
        "vad" => cmd_vad(&args)?,
        "run" => cmd_run(&args)?,
        "chunk" => cmd_chunk(&args)?,
        "stream" => cmd_stream(&args)?,
        other => bail!("unknown subcommand '{other}' (vad, run, chunk, stream)"),
    };
    std::fs::write(&out, serde_json::to_vec_pretty(&doc)?)
        .with_context(|| format!("writing {}", out.display()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Arguments

#[derive(Debug)]
struct Args {
    cmd: String,
    flags: HashMap<String, String>,
}

impl Args {
    fn parse(argv: Vec<String>) -> Result<Self> {
        let mut it = argv.into_iter();
        let cmd = it.next().ok_or_else(|| anyhow!("missing subcommand"))?;
        let mut flags = HashMap::new();
        let mut it = it.peekable();
        while let Some(a) = it.next() {
            let key = a
                .strip_prefix("--")
                .ok_or_else(|| anyhow!("unexpected argument '{a}'"))?
                .to_string();
            let value = match it.peek() {
                Some(next) if !next.starts_with("--") => it.next().unwrap_or_default(),
                _ => "true".to_string(),
            };
            flags.insert(key, value);
        }
        Ok(Self { cmd, flags })
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.flags.get(key).map(String::as_str)
    }

    fn req(&self, key: &str) -> Result<&str> {
        self.get(key).ok_or_else(|| anyhow!("missing --{key}"))
    }

    fn usize(&self, key: &str) -> Result<Option<usize>> {
        self.get(key)
            .map(|v| v.parse().with_context(|| format!("--{key} {v}")))
            .transpose()
    }

    fn cpu(&self) -> bool {
        self.get("cpu") == Some("true")
    }
}

/// `[start, end)` of `audio`, clamped; the whole clip when both are absent.
fn clip(audio: &[f32], start: Option<usize>, end: Option<usize>) -> Result<&[f32]> {
    let start = start.unwrap_or(0);
    let end = end.unwrap_or(audio.len()).min(audio.len());
    if start >= end {
        bail!("empty clip {start}..{end}");
    }
    Ok(&audio[start..end])
}

// ---------------------------------------------------------------------------
// Memory

#[derive(Debug, Clone, Copy, Serialize, Default)]
struct Mem {
    /// `getrusage` peak resident set, bytes (macOS reports bytes).
    peak_rss: u64,
    /// Current and lifetime-peak physical footprint (what Activity Monitor
    /// calls Memory; includes Metal buffers in unified memory).
    phys_footprint: u64,
    peak_phys_footprint: u64,
}

fn mem() -> Mem {
    let mut m = Mem::default();
    // SAFETY: plain syscalls filling caller-owned, zeroed structs.
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut ru) == 0 {
            m.peak_rss = ru.ru_maxrss as u64;
        }
        #[cfg(target_os = "macos")]
        {
            let mut info: libc::rusage_info_v4 = std::mem::zeroed();
            let ptr = &mut info as *mut libc::rusage_info_v4 as *mut libc::rusage_info_t;
            if libc::proc_pid_rusage(libc::getpid(), libc::RUSAGE_INFO_V4, ptr) == 0 {
                m.phys_footprint = info.ri_phys_footprint;
                m.peak_phys_footprint = info.ri_lifetime_max_phys_footprint;
            }
        }
    }
    m
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

// ---------------------------------------------------------------------------
// Engine, as the worker drives it

#[derive(Debug, Clone, Serialize)]
struct Loaded {
    arch: String,
    variant: String,
    backend: String,
    device: String,
    languages: Vec<String>,
    supports_streaming: bool,
    supports_long_form: bool,
    supports_language_detect: bool,
    max_audio_ms: i64,
    max_timestamp_kind: String,
}

fn init_backends(cpu: bool) -> Result<f64> {
    let t = Instant::now();
    transcribe_cpp::init_logging();
    if cpu {
        transcribe_cpp::init_backends_with(None::<&Path>, BackendMask::CPU)?;
    } else {
        transcribe_cpp::init_backends_default()?;
    }
    Ok(ms(t))
}

fn load(path: &Path, cpu: bool) -> Result<(Model, Session, Loaded)> {
    let backend = if cpu { Backend::Cpu } else { Backend::Auto };
    let model = Model::load_with(
        path,
        &ModelOptions {
            backend,
            device: None,
        },
    )
    .with_context(|| format!("loading {}", path.display()))?;
    let session = model.session()?;
    let caps = model.capabilities()?;
    let device = model
        .device()
        .ok()
        .map(|d| format!("{d:?}"))
        .unwrap_or_else(|| "unknown".into());
    let info = Loaded {
        arch: model.arch(),
        variant: model.variant(),
        backend: model.backend(),
        device,
        languages: caps.languages.clone(),
        supports_streaming: caps.supports_streaming,
        supports_long_form: model.supports(Feature::LongForm),
        supports_language_detect: caps.supports_language_detect,
        max_audio_ms: caps.max_audio_ms,
        max_timestamp_kind: format!("{:?}", caps.max_timestamp_kind),
    };
    Ok((model, session, info))
}

/// Handy's `transcribe_cpp_run_plan` without translation: a language hint
/// only when the model advertises it, else the model's own detection.
fn run_options(language: Option<&str>, model_languages: &[String]) -> RunOptions {
    RunOptions {
        language: language
            .filter(|l| model_languages.iter().any(|m| m == l))
            .map(str::to_string),
        ..Default::default()
    }
}

#[derive(Debug, Clone, Serialize)]
struct RunResult {
    id: String,
    audio_ms: f64,
    ms: f64,
    text: String,
    language_hint: Option<String>,
    detected_language: Option<String>,
    error: Option<String>,
    /// Word timings (ms from the clip start) where the family reports them;
    /// `TimestampKind::Auto` is the default the app runs with.
    words: Vec<WordTime>,
}

#[derive(Debug, Clone, Serialize)]
struct WordTime {
    t0: i64,
    t1: i64,
    text: String,
}

fn run_one(session: &mut Session, pcm: &[f32], id: &str, opts: &RunOptions) -> RunResult {
    let t = Instant::now();
    let result = session.run(pcm, opts);
    let elapsed = ms(t);
    let (text, detected_language, error, words) = match result {
        Ok(tr) => {
            let words = tr
                .words
                .iter()
                .map(|w| WordTime {
                    t0: w.t0_ms,
                    t1: w.t1_ms,
                    text: w.text.clone(),
                })
                .collect();
            (tr.text, tr.language, None, words)
        }
        Err(e) => (String::new(), None, Some(e.to_string()), Vec::new()),
    };
    RunResult {
        id: id.to_string(),
        audio_ms: pcm.len() as f64 / RATE * 1000.0,
        ms: elapsed,
        text,
        language_hint: opts.language.clone(),
        detected_language,
        error,
        words,
    }
}

// ---------------------------------------------------------------------------
// vad: Handy's SileroVad probabilities for one clip, scored from a fresh state

#[derive(Debug, Serialize, Deserialize)]
struct Probs {
    start: usize,
    end: usize,
    frame_samples: usize,
    probs: Vec<f32>,
}

fn cmd_vad(args: &Args) -> Result<serde_json::Value> {
    let audio = read_wav_samples(args.req("wav")?)?;
    let start = args.usize("start")?.unwrap_or(0);
    let pcm = clip(&audio, Some(start), args.usize("end")?)?;
    // Handy's threshold for chunking does not matter: only scores are used.
    let mut vad = SileroVad::new(args.req("vad")?, 0.5)?;
    let frame = vad.frame_samples();
    let whole = pcm.len() / frame * frame;
    let t = Instant::now();
    let mut probs = vad.score(&pcm[..whole])?;
    if whole < pcm.len() {
        let mut tail = pcm[whole..].to_vec();
        tail.resize(frame, 0.0);
        probs.extend(vad.score(&tail)?);
    }
    let doc = Probs {
        start,
        end: start + pcm.len(),
        frame_samples: frame,
        probs,
    };
    let mut v = serde_json::to_value(doc)?;
    v["ms"] = ms(t).into();
    Ok(v)
}

// ---------------------------------------------------------------------------
// run: cold load, a list of clips, unload, reload, one more run

#[derive(Debug, Deserialize)]
struct Job {
    id: String,
    start: usize,
    end: usize,
    language: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Jobs {
    jobs: Vec<Job>,
    /// Job re-run after unload and reload (default: the first).
    reload_job: Option<String>,
}

fn cmd_run(args: &Args) -> Result<serde_json::Value> {
    let cpu = args.cpu();
    let audio = read_wav_samples(args.req("wav")?)?;
    let jobs: Jobs = serde_json::from_slice(&std::fs::read(args.req("jobs")?)?)?;
    let first = jobs.jobs.first().ok_or_else(|| anyhow!("no jobs"))?;
    let model_path = PathBuf::from(args.req("model")?);
    // `--words`: word timings for the split pass only; benchmark runs keep
    // the app's default (`TimestampKind::Auto`).
    let words = args.get("words") == Some("true");
    let word_kind = match args.get("timestamps") {
        Some("token") => TimestampKind::Token,
        Some("segment") => TimestampKind::Segment,
        _ => TimestampKind::Word,
    };
    let opts_for = |language: Option<&str>, languages: &[String]| RunOptions {
        timestamps: if words {
            word_kind
        } else {
            TimestampKind::Auto
        },
        ..run_options(language, languages)
    };

    let mem_start = mem();
    let init_ms = init_backends(cpu)?;
    let t = Instant::now();
    let (model, mut session, info) = load(&model_path, cpu)?;
    let load_ms = ms(t);
    let mem_loaded = mem();

    // The first run after a load pays one-time costs (Metal pipelines,
    // scratch buffers): what "first take after ASR unload" adds in §5.1.
    let pcm = clip(&audio, Some(first.start), Some(first.end))?;
    let first_run = run_one(
        &mut session,
        pcm,
        &first.id,
        &opts_for(first.language.as_deref(), &info.languages),
    );

    let mut results = Vec::new();
    for job in &jobs.jobs {
        let pcm = clip(&audio, Some(job.start), Some(job.end))?;
        let opts = opts_for(job.language.as_deref(), &info.languages);
        let r = run_one(&mut session, pcm, &job.id, &opts);
        eprintln!(
            "{}: {:.0} ms for {:.1} s{}",
            r.id,
            r.ms,
            r.audio_ms / 1000.0,
            r.error
                .as_deref()
                .map(|e| format!(" ERROR {e}"))
                .unwrap_or_default()
        );
        results.push(r);
    }
    let mem_after_runs = mem();

    // Unload (drop session and model, as the worker does) and reload.
    drop(session);
    drop(model);
    let mem_unloaded = mem();
    let t = Instant::now();
    let (_model2, mut session2, _) = load(&model_path, cpu)?;
    let reload_ms = ms(t);
    let reload_id = jobs.reload_job.clone().unwrap_or_else(|| first.id.clone());
    let job = jobs
        .jobs
        .iter()
        .find(|j| j.id == reload_id)
        .ok_or_else(|| anyhow!("reload job {reload_id} not found"))?;
    let pcm = clip(&audio, Some(job.start), Some(job.end))?;
    let opts = opts_for(job.language.as_deref(), &info.languages);
    let after_reload = run_one(&mut session2, pcm, &job.id, &opts);
    let after_reload_warm = run_one(&mut session2, pcm, &job.id, &opts);

    Ok(serde_json::json!({
        "model": model_path,
        "cpu": cpu,
        "info": info,
        "init_ms": init_ms,
        "load_ms": load_ms,
        "first_run": first_run,
        "results": results,
        "reload_ms": reload_ms,
        "after_reload": after_reload,
        "after_reload_warm": after_reload_warm,
        "mem": {
            "start": mem_start,
            "loaded": mem_loaded,
            "after_runs": mem_after_runs,
            "unloaded": mem_unloaded,
            "end": mem(),
        },
    }))
}

// ---------------------------------------------------------------------------
// chunk: Handy's live long-form chunking, replayed with recorded VAD scores

/// Replays Silero probabilities scored earlier for the same clip from a
/// fresh state, so the Chunker sees exactly what Handy's `SileroVad` gives.
struct ReplayScorer {
    frame_samples: usize,
    probs: Vec<f32>,
    next: usize,
}

impl VoiceActivityDetector for ReplayScorer {
    fn push_frame<'a>(&'a mut self, frame: &'a [f32]) -> Result<VadFrame<'a>> {
        let p = self.score(frame)?;
        Ok(if p[0] > 0.5 {
            VadFrame::Speech(frame)
        } else {
            VadFrame::Noise
        })
    }

    fn frame_samples(&self) -> usize {
        self.frame_samples
    }
}

impl SpeechScorer for ReplayScorer {
    fn score(&mut self, frames: &[f32]) -> Result<Vec<f32>> {
        if !frames.len().is_multiple_of(self.frame_samples) {
            bail!("expected whole frames, got {} samples", frames.len());
        }
        let n = frames.len() / self.frame_samples;
        let end = self.next + n;
        if end > self.probs.len() {
            bail!("replay ran out of scores at frame {}", self.probs.len());
        }
        let out = self.probs[self.next..end].to_vec();
        self.next = end;
        Ok(out)
    }
}

#[derive(Debug, Serialize)]
struct ChunkResult {
    index: usize,
    /// Samples pushed (from the clip start) when the Chunker handed it back;
    /// `None` for chunks only cut by `finish()` after key-up.
    emitted_at: Option<usize>,
    samples: usize,
    run: RunResult,
}

fn cmd_chunk(args: &Args) -> Result<serde_json::Value> {
    let cpu = args.cpu();
    let audio = read_wav_samples(args.req("wav")?)?;
    let start = args.usize("start")?;
    let pcm = clip(&audio, start, args.usize("end")?)?.to_vec();
    let probs: Probs = serde_json::from_slice(&std::fs::read(args.req("probs")?)?)?;
    if Some(probs.start) != start.or(Some(0)) {
        bail!("VAD scores are for a clip starting at {}", probs.start);
    }
    let push = args.usize("push")?.unwrap_or(STREAM_FEED);
    let language = args.get("language").map(str::to_string);

    init_backends(cpu)?;
    let (_model, mut session, info) = load(Path::new(args.req("model")?), cpu)?;
    let opts = run_options(language.as_deref(), &info.languages);
    // Warm the session as an app that has already taken a dictation would be.
    let warm = run_one(
        &mut session,
        &pcm[..pcm.len().min(16_000 * 5)],
        "warm",
        &opts,
    );

    let frame_samples = probs.frame_samples;
    let scores = probs.probs;
    let mut chunker = Chunker::new(Box::new(move || {
        Ok(Box::new(ReplayScorer {
            frame_samples,
            probs: scores.clone(),
            next: 0,
        }) as Box<dyn SpeechScorer>)
    }));

    let mut chunks: Vec<ChunkResult> = Vec::new();
    let mut pushed = 0usize;
    for piece in pcm.chunks(push) {
        pushed += piece.len();
        for c in chunker.push(piece.to_vec()) {
            let r = run_one(&mut session, &c, &format!("c{}", chunks.len()), &opts);
            chunks.push(ChunkResult {
                index: chunks.len(),
                emitted_at: Some(pushed),
                samples: c.len(),
                run: r,
            });
        }
    }
    // Key-up: cut and decode what is left.
    let t = Instant::now();
    for c in chunker.finish() {
        let r = run_one(&mut session, &c, &format!("c{}", chunks.len()), &opts);
        chunks.push(ChunkResult {
            index: chunks.len(),
            emitted_at: None,
            samples: c.len(),
            run: r,
        });
    }
    let after_key_up_ms = ms(t);
    let texts: Vec<&str> = chunks.iter().map(|c| c.run.text.as_str()).collect();
    let joined = join_transcripts(&texts);

    Ok(serde_json::json!({
        "info": info,
        "language_hint": opts.language,
        "warm_ms": warm.ms,
        "samples": pcm.len(),
        "chunks": chunks,
        "after_key_up_ms": after_key_up_ms,
        "text": joined,
        "mem": mem(),
    }))
}

// ---------------------------------------------------------------------------
// stream: a streaming model fed as the recorder feeds it

fn cmd_stream(args: &Args) -> Result<serde_json::Value> {
    let cpu = args.cpu();
    let audio = read_wav_samples(args.req("wav")?)?;
    let pcm = clip(&audio, args.usize("start")?, args.usize("end")?)?;
    let language = args.get("language").map(str::to_string);

    init_backends(cpu)?;
    let (_model, mut session, info) = load(Path::new(args.req("model")?), cpu)?;
    if !info.supports_streaming {
        bail!("model does not support streaming");
    }
    let opts = run_options(language.as_deref(), &info.languages);
    let warm = run_one(
        &mut session,
        &pcm[..pcm.len().min(16_000 * 5)],
        "warm",
        &opts,
    );

    let mut stream = session.stream(&opts, &StreamOptions::default())?;
    let mut feed_ms_total = 0.0;
    let mut feed_ms_max: f64 = 0.0;
    let mut feeds = 0usize;
    // Wall time a real-time feed would have been behind, if compute ever
    // fell behind the audio clock.
    let mut behind_ms: f64 = 0.0;
    for frame in pcm.chunks(STREAM_FEED) {
        let t = Instant::now();
        stream.feed(frame)?;
        let f = ms(t);
        feeds += 1;
        feed_ms_total += f;
        feed_ms_max = feed_ms_max.max(f);
        // Each frame arrives one frame-length after the last.
        behind_ms = (behind_ms + f - frame.len() as f64 / RATE * 1000.0).max(0.0);
    }
    let t = Instant::now();
    let update = stream.finalize()?;
    let finalize_ms = ms(t);
    let text = stream.text();
    Ok(serde_json::json!({
        "info": info,
        "language_hint": opts.language,
        "warm_ms": warm.ms,
        "audio_ms": pcm.len() as f64 / RATE * 1000.0,
        "feeds": feeds,
        "feed_ms_total": feed_ms_total,
        "feed_ms_max": feed_ms_max,
        "backlog_at_key_up_ms": behind_ms,
        "finalize_ms": finalize_ms,
        "is_final": update.is_final,
        "text": text.full,
        "committed": text.committed,
        "mem": mem(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn langs(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn language_hint_only_when_advertised() {
        let en = langs(&["en"]);
        assert_eq!(run_options(Some("en"), &en).language.as_deref(), Some("en"));
        assert_eq!(run_options(Some("nl"), &en).language, None);
        assert_eq!(run_options(None, &en).language, None);
        assert_eq!(run_options(Some("en"), &[]).language, None);
    }

    #[test]
    fn args_parse_flags_and_switches() {
        let a = Args::parse(
            ["run", "--model", "m.gguf", "--cpu", "--out", "o.json"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        )
        .unwrap();
        assert_eq!(a.cmd, "run");
        assert_eq!(a.req("model").unwrap(), "m.gguf");
        assert!(a.cpu());
        assert_eq!(a.req("out").unwrap(), "o.json");
        assert!(a.req("wav").is_err());
    }

    #[test]
    fn clip_bounds() {
        let audio = vec![0.0f32; 100];
        assert_eq!(clip(&audio, Some(10), Some(20)).unwrap().len(), 10);
        assert_eq!(clip(&audio, None, Some(1000)).unwrap().len(), 100);
        assert!(clip(&audio, Some(50), Some(50)).is_err());
    }

    #[test]
    fn replay_scorer_hands_back_scores_in_order() {
        let mut s = ReplayScorer {
            frame_samples: 4,
            probs: vec![0.1, 0.9, 0.2],
            next: 0,
        };
        assert_eq!(s.score(&[0.0; 8]).unwrap(), vec![0.1, 0.9]);
        assert!(s.score(&[0.0; 3]).is_err());
        assert_eq!(s.score(&[0.0; 4]).unwrap(), vec![0.2]);
        assert!(s.score(&[0.0; 4]).is_err());
    }

    /// Handy's Chunker leaves a clip of 40 s or less in one piece, and the
    /// replayed scores drive its cuts on a longer one.
    #[test]
    fn chunker_with_replayed_scores() {
        let frame = 512;
        let secs = 100usize;
        let n = secs * 16_000 / frame;
        // Speech everywhere, with a 1 s pause every 10 s.
        let probs: Vec<f32> = (0..n)
            .map(|i| {
                let t = (i * frame) as f64 / 16_000.0;
                if t % 10.0 > 9.0 {
                    0.0
                } else {
                    1.0
                }
            })
            .collect();
        let p = probs.clone();
        let mut chunker = Chunker::new(Box::new(move || {
            Ok(Box::new(ReplayScorer {
                frame_samples: frame,
                probs: p.clone(),
                next: 0,
            }) as Box<dyn SpeechScorer>)
        }));
        let mut chunks = Vec::new();
        for piece in vec![0.0f32; n * frame].chunks(480) {
            chunks.extend(chunker.push(piece.to_vec()));
        }
        chunks.extend(chunker.finish());
        let total: usize = chunks.iter().map(Vec::len).sum();
        assert_eq!(total, n * frame);
        assert!(chunks.len() >= 3, "{} chunks", chunks.len());
        assert!(chunks.iter().all(|c| c.len() <= 45 * 16_000));
    }
}
