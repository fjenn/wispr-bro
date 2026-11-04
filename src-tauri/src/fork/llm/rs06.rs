// SPDX-License-Identifier: GPL-3.0-or-later
//! RS-06 bench (S-10): the real supervisor, transport and client against the
//! real `llama-server` sidecar. Ignored by default; run
//!
//! ```text
//! WB_LLAMA_SERVER=<sidecar> WB_LLM_MODEL=<gguf> [WB_ASR_MODEL=<gguf>]
//! [WB_ASR_WAV=<16 kHz wav>] WB_RS06_OUT=<report.json> \
//!   cargo test --lib fork::llm::rs06 -- --ignored --nocapture
//! ```
//!
//! It writes one JSON report; `docs/fork/spikes/RS-06-llm-runtime.md` quotes
//! it. Memory is read with `proc_pid_rusage` (`ri_phys_footprint`, what
//! Activity Monitor calls "Memory", incl. Metal allocations; and
//! `ri_resident_size`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::chat::{CancellationToken, ChatRequest};
use super::prompt;
use super::supervisor::{Config, ProcessLauncher, Supervisor};

fn env_path(k: &str) -> Option<PathBuf> {
    std::env::var_os(k).map(PathBuf::from)
}

/// (phys_footprint, resident) of a pid, bytes.
fn footprint(pid: i32) -> Option<(u64, u64)> {
    let mut info: libc::rusage_info_v4 = unsafe { std::mem::zeroed() };
    // SAFETY: the buffer is a rusage_info_v4 as flavor V4 requires.
    let rc = unsafe {
        libc::proc_pid_rusage(
            pid,
            libc::RUSAGE_INFO_V4,
            (&mut info as *mut libc::rusage_info_v4).cast(),
        )
    };
    (rc == 0).then_some((info.ri_phys_footprint, info.ri_resident_size))
}

/// Samples a pid every 100 ms; returns the peaks when stopped.
struct Sampler {
    stop: Arc<AtomicBool>,
    peak_fp: Arc<AtomicU64>,
    peak_rss: Arc<AtomicU64>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Sampler {
    fn start(pids: Vec<i32>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let peak_fp = Arc::new(AtomicU64::new(0));
        let peak_rss = Arc::new(AtomicU64::new(0));
        let (s, f, r) = (stop.clone(), peak_fp.clone(), peak_rss.clone());
        let handle = std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                let (mut fp, mut rss) = (0, 0);
                for pid in &pids {
                    if let Some((a, b)) = footprint(*pid) {
                        fp += a;
                        rss += b;
                    }
                }
                f.fetch_max(fp, Ordering::Relaxed);
                r.fetch_max(rss, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        Self {
            stop,
            peak_fp,
            peak_rss,
            handle: Some(handle),
        }
    }

    fn finish(mut self) -> (f64, f64) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        (
            mib(self.peak_fp.load(Ordering::Relaxed)),
            mib(self.peak_rss.load(Ordering::Relaxed)),
        )
    }
}

fn mib(b: u64) -> f64 {
    (b as f64 / 1048576.0 * 10.0).round() / 10.0
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let i = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    pct(&v, 50.0)
}

fn p95(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    pct(&v, 95.0)
}

fn stats(mut v: Vec<f64>) -> Value {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    json!({
        "n": v.len(),
        "p50_ms": (pct(&v, 50.0) * 10.0).round() / 10.0,
        "p95_ms": (pct(&v, 95.0) * 10.0).round() / 10.0,
        "max_ms": (v.last().copied().unwrap_or(f64::NAN) * 10.0).round() / 10.0,
    })
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// English items of the F-10 set with at most 50 words (R5 §5.1 Light row).
fn light_inputs() -> Vec<String> {
    include_str!("../../../../eval/cleanup/items.jsonl")
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["language"] == "en")
        .filter_map(|v| v["input"].as_str().map(str::to_owned))
        .filter(|s| s.split_whitespace().count() <= 50)
        .collect()
}

fn cleanup(text: &str) -> ChatRequest {
    ChatRequest {
        system: prompt::cleanup_system(),
        user: prompt::cleanup_user(text),
        max_tokens: prompt::max_tokens(text),
        response_format: Some(prompt::response_format()),
        examples: Vec::new(),
    }
}

fn supervisor(bin: PathBuf, model: PathBuf, dir: &std::path::Path) -> Arc<Supervisor> {
    let mut cfg = Config::new(bin, dir.join("llm"));
    cfg.model = Some(model);
    Supervisor::new(Arc::new(ProcessLauncher), cfg)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "RS-06 bench: needs WB_LLAMA_SERVER, WB_LLM_MODEL"]
async fn rs06_bench() {
    let (Some(bin), Some(model)) = (env_path("WB_LLAMA_SERVER"), env_path("WB_LLM_MODEL")) else {
        eprintln!("rs06: set WB_LLAMA_SERVER and WB_LLM_MODEL");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut report = serde_json::Map::new();
    let never = CancellationToken::new();
    report.insert(
        "model".into(),
        json!(model.file_name().unwrap().to_string_lossy()),
    );
    report.insert(
        "memory_at_start".into(),
        json!(super::memory::snapshot().map(|m| json!({
            "physical_mib": mib(m.physical_bytes), "available_mib": mib(m.available_bytes)
        }))),
    );

    // A. Cold start to /health (page cache warm) and the first request.
    let mut starts = Vec::new();
    let mut first_req = Vec::new();
    for _ in 0..5 {
        let sup = supervisor(bin.clone(), model.clone(), dir.path());
        let t0 = Instant::now();
        sup.ensure_ready().await.unwrap();
        starts.push(ms(t0.elapsed()));
        let t1 = Instant::now();
        sup.complete(
            &cleanup("um so I think we should uh ship it on monday"),
            Duration::from_secs(30),
            &never,
        )
        .await
        .unwrap();
        first_req.push(ms(t1.elapsed()));
        sup.stop().await;
    }
    report.insert("cold_start_to_health".into(), stats(starts));
    report.insert("first_request_after_start".into(), stats(first_req));

    // B. Warm Light latency, ≤ 50 words, with memory sampling.
    let sup = supervisor(bin.clone(), model.clone(), dir.path());
    sup.ensure_ready().await.unwrap();
    let pid = sup.pid().await.unwrap() as i32;
    let sock = dir.path().join("llm").join("llm.sock");
    let mode = std::fs::metadata(&sock).map(|m| {
        use std::os::unix::fs::PermissionsExt;
        format!("{:o}", m.permissions().mode() & 0o777)
    });
    report.insert("socket_mode".into(), json!(mode.ok()));
    let lsof = std::process::Command::new("/usr/sbin/lsof")
        .args(["-a", "-i", "-p", &pid.to_string()])
        .output()
        .unwrap();
    report.insert(
        "inet_sockets".into(),
        json!(String::from_utf8_lossy(&lsof.stdout).lines().count()),
    );
    sup.complete(&cleanup("warm up"), Duration::from_secs(30), &never)
        .await
        .unwrap();
    let inputs = light_inputs();
    let sampler = Sampler::start(vec![pid]);
    let (mut lat, mut prompt_ms, mut gen_tps, mut cached, mut misses, mut bad) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), 0, 0);
    for text in &inputs {
        let r = sup
            .complete(&cleanup(text), Duration::from_secs(30), &never)
            .await
            .unwrap();
        let total = ms(r.total);
        if r.total > prompt::deadline(text) {
            misses += 1;
        }
        if prompt::parse_reply(&r.content).is_none() {
            bad += 1;
        }
        lat.push(total);
        prompt_ms.push(r.prompt_ms);
        if r.generation_ms > 0.0 {
            gen_tps.push(r.generated_tokens as f64 / (r.generation_ms / 1000.0));
        }
        cached.push(r.cached_tokens as f64);
    }
    let (fp, rss) = sampler.finish();
    report.insert(
        "light_warm".into(),
        json!({
            "latency": stats(lat),
            "prompt_eval_ms": stats(prompt_ms),
            "gen_tok_per_s_median": median(gen_tps).round(),
            "cached_prompt_tokens_median": median(cached),
            "deadline_misses": misses,
            "unparsed_replies": bad,
            "server_peak_footprint_mib": fp,
            "server_peak_rss_mib": rss,
        }),
    );

    // C. Abort: cancel a long generation, then time a short request.
    let short = cleanup("ok thanks");
    let mut base = Vec::new();
    for _ in 0..5 {
        base.push(ms(sup
            .complete(&short, Duration::from_secs(30), &never)
            .await
            .unwrap()
            .total));
    }
    let long = ChatRequest {
        system: "Repeat the user's text exactly, then repeat it again.".into(),
        user: "word ".repeat(400),
        max_tokens: 1500,
        response_format: None,
        examples: Vec::new(),
    };
    let (mut cancel_return, mut after) = (Vec::new(), Vec::new());
    for _ in 0..10 {
        let cancel = CancellationToken::new();
        let c2 = cancel.clone();
        let s2 = sup.clone();
        let l2 = long.clone();
        let task =
            tokio::spawn(async move { s2.complete(&l2, Duration::from_secs(60), &c2).await });
        tokio::time::sleep(Duration::from_millis(400)).await;
        let t0 = Instant::now();
        cancel.cancel();
        let r = task.await.unwrap();
        assert!(r.is_err());
        cancel_return.push(ms(t0.elapsed()));
        let r = sup
            .complete(&short, Duration::from_secs(30), &never)
            .await
            .unwrap();
        assert!(!r.content.is_empty());
        after.push(ms(t0.elapsed()));
    }
    let base_p50 = median(base.clone());
    let extra_p95 = (p95(after.clone()) - base_p50).round();
    report.insert(
        "abort".into(),
        json!({
            "cancel_returns": stats(cancel_return),
            "short_request_baseline": stats(base),
            "cancel_to_short_reply": stats(after.clone()),
            "slot_free_extra_ms_p95": extra_p95,
        }),
    );

    // D. Contention: ASR (transcribe-cpp, Metal) busy in this process while
    // cleanups run; combined footprint.
    if let (Some(asr), Some(wav)) = (env_path("WB_ASR_MODEL"), env_path("WB_ASR_WAV")) {
        let _ = transcribe_cpp::init_backends_default();
        let model = transcribe_cpp::Model::load_with(
            &asr,
            &transcribe_cpp::ModelOptions {
                backend: transcribe_cpp::Backend::Auto,
                device: None,
            },
        )
        .unwrap();
        let pcm = crate::audio_toolkit::read_wav_samples(&wav).unwrap();
        let pcm: Vec<f32> = pcm.into_iter().take(16_000 * 20).collect();
        // SAFETY: getpid has no preconditions.
        let me = unsafe { libc::getpid() };
        let idle = footprint(me).map(|(a, _)| mib(a));
        let stop = Arc::new(AtomicBool::new(false));
        let runs = Arc::new(AtomicU64::new(0));
        let (s2, r2) = (stop.clone(), runs.clone());
        let asr_thread = std::thread::spawn(move || {
            let mut session = model.session().unwrap();
            let mut times = Vec::new();
            while !s2.load(Ordering::Relaxed) {
                let t = Instant::now();
                session
                    .run(&pcm, &transcribe_cpp::RunOptions::default())
                    .unwrap();
                times.push(ms(t.elapsed()));
                r2.fetch_add(1, Ordering::Relaxed);
            }
            times
        });
        let sampler = Sampler::start(vec![pid, me]);
        let mut lat = Vec::new();
        for text in inputs.iter().take(40) {
            lat.push(ms(sup
                .complete(&cleanup(text), Duration::from_secs(30), &never)
                .await
                .unwrap()
                .total));
        }
        stop.store(true, Ordering::Relaxed);
        let asr_times = asr_thread.join().unwrap();
        let (fp, rss) = sampler.finish();
        report.insert(
            "contention".into(),
            json!({
                "asr_model": asr.file_name().unwrap().to_string_lossy(),
                "asr_runs_20s_clip": stats(asr_times),
                "light_latency_while_asr_busy": stats(lat),
                "bench_process_footprint_mib_with_asr_loaded": idle,
                "combined_peak_footprint_mib": fp,
                "combined_peak_rss_mib": rss,
            }),
        );
    }

    // E. Sleep stand-in: SIGSTOP 5 s / SIGCONT keeps the server; a server
    // killed while stopped is restarted by on_wake.
    // SAFETY: signals to the server we started.
    unsafe { libc::kill(pid, libc::SIGSTOP) };
    tokio::time::sleep(Duration::from_secs(5)).await;
    unsafe { libc::kill(pid, libc::SIGCONT) };
    let t0 = Instant::now();
    sup.on_wake().await;
    let kept = sup.pid().await == Some(pid as u32);
    let wake_ok_ms = ms(t0.elapsed());
    unsafe { libc::kill(pid, libc::SIGSTOP) };
    unsafe { libc::kill(pid, libc::SIGKILL) };
    tokio::time::sleep(Duration::from_millis(300)).await;
    let t1 = Instant::now();
    sup.on_wake().await;
    // Wait for the background crash-restart or the wake restart.
    loop {
        if sup.pid().await.is_some_and(|p| p != pid as u32) && sup.is_running().await {
            break;
        }
        assert!(t1.elapsed() < Duration::from_secs(30));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let r = sup
        .complete(&cleanup("hello there"), Duration::from_secs(30), &never)
        .await;
    report.insert(
        "wake".into(),
        json!({
            "stopped_5s_then_continued_kept": kept,
            "health_check_ms": wake_ok_ms.round(),
            "killed_while_asleep_back_in_ms": ms(t1.elapsed()).round(),
            "request_after_restart_ok": r.is_ok(),
            "restarts": sup.status().restarts,
        }),
    );
    sup.stop().await;

    let out = serde_json::to_string_pretty(&Value::Object(report)).unwrap();
    println!("{out}");
    if let Some(p) = env_path("WB_RS06_OUT") {
        std::fs::write(p, out).unwrap();
    }
}
