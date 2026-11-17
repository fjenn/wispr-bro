// SPDX-License-Identifier: GPL-3.0-or-later
//! Equivalence run: the cleanup eval items through the in-process engine and
//! through a real `llama-server` sidecar, request for request.
//!
//! Each item becomes the request the app sends for one block of a take
//! (`post_process::ask`): lead-ins stripped, the Light system prompt, the
//! `<transcript>` user turn, the JSON schema, `max_tokens`, temperature 0,
//! seed 42, streamed. The prompt builder, the guard chain, the deadlines and
//! the language detector are the app's own files, compiled from
//! `src-tauri/src` unchanged (`#[path]` below). Not run here: the app's
//! "already clean" gate and its paragraph chunking (they live in
//! `post_process.rs`, which needs Tauri), so every item goes to the model as
//! one request.
//!
//! ```text
//! cargo run --release --example eval -- \
//!   --model ~/wb-bench/llm/qwen35-2b-muesli-v12-Q4_K_M.gguf \
//!   --items ../../../eval/cleanup/items.jsonl --out <dir> \
//!   [--sidecar "/Applications/Wispr Bro.app/Contents/MacOS/llama-server"] \
//!   [--listener] [--cpu] [--limit N] [--no-engine] [--no-deadline] [--extra-items FILE]
//! ```
//!
//! `--listener` also runs the items through the engine's in-process listener
//! (`wb_ios_llm::serve`), over HTTP like the app.
//!
//! Writes `<dir>/inprocess.jsonl`, `<dir>/listener.jsonl` and `<dir>/sidecar.jsonl` (the replay
//! format of `scripts/wb-eval-cleanup.ts --replay`) and `<dir>/summary.json`.

#![allow(dead_code)]

#[path = "../../../src/fork/llm/deadline.rs"]
mod deadline;
#[path = "../../../src/fork/llm/guards.rs"]
mod guards;
#[path = "../../../src/audio_toolkit/lang_id.rs"]
mod lang_id;
#[path = "../../../src/fork/llm/leadins.rs"]
mod leadins;
#[path = "../../../src/fork/llm/markers.rs"]
mod markers;
#[path = "../../../src/fork/llm/prompt.rs"]
mod prompt;

/// The two names the app's files import from the rest of the app.
mod fork {
    pub mod settings {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub enum CleanupLevel {
            None,
            #[default]
            Light,
            Medium,
        }
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum StylePreset {
            Formal,
            Relaxed,
            VeryRelaxed,
            Upbeat,
        }
    }
}
mod audio_toolkit {
    pub use crate::lang_id::detect_output_language;
}

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use fork::settings::CleanupLevel;
use serde_json::{json, Value};
use wb_ios_llm::{Config, Device, Engine, Stop, CHAT_COMPLETIONS};

/// `fork/llm/chat.rs` `ChatRequest::body`.
fn body(system: &str, user: &str, max_tokens: u32) -> Value {
    json!({
        "model": "local",
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
        "temperature": 0,
        "seed": 42,
        "max_tokens": max_tokens,
        "stream": true,
        "chat_template_kwargs": { "enable_thinking": false },
        "response_format": prompt::response_format(),
    })
}

#[derive(Default, Debug, Clone)]
struct Reply {
    content: String,
    finish: Option<String>,
    done: bool,
    prompt_n: i64,
    prompt_ms: f64,
    predicted_n: i64,
    predicted_ms: f64,
    cache_n: i64,
    wall_ms: f64,
    first_ms: Option<f64>,
    status: u16,
    error: Option<String>,
}

/// `fork/llm/chat.rs` `StreamState::feed`, on whole lines.
fn feed(reply: &mut Reply, line: &str, t0: Instant) {
    let Some(data) = line.trim().strip_prefix("data:") else {
        return;
    };
    let data = data.trim();
    if data == "[DONE]" {
        reply.done = true;
        return;
    }
    let Ok(v) = serde_json::from_str::<Value>(data) else {
        reply.error = Some("malformed chunk".into());
        return;
    };
    if let Some(c) = v["choices"].get(0) {
        if let Some(t) = c["delta"]["content"].as_str() {
            if reply.first_ms.is_none() && !t.is_empty() {
                reply.first_ms = Some(ms(t0.elapsed()));
            }
            reply.content.push_str(t);
        }
        if let Some(f) = c["finish_reason"].as_str() {
            reply.finish = Some(f.to_owned());
        }
    }
    if let Some(t) = v.get("timings").filter(|t| t.is_object()) {
        reply.prompt_n = t["prompt_n"].as_i64().unwrap_or(0);
        reply.prompt_ms = t["prompt_ms"].as_f64().unwrap_or(0.0);
        reply.predicted_n = t["predicted_n"].as_i64().unwrap_or(0);
        reply.predicted_ms = t["predicted_ms"].as_f64().unwrap_or(0.0);
        reply.cache_n = t["cache_n"].as_i64().unwrap_or(0);
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

trait Backend {
    fn name(&self) -> &'static str;
    fn chat(&mut self, body: &Value, timeout: Duration) -> Reply;
}

struct InProcess(Engine);

impl Backend for InProcess {
    fn name(&self) -> &'static str {
        "inprocess"
    }
    fn chat(&mut self, body: &Value, timeout: Duration) -> Reply {
        let t0 = Instant::now();
        let stop = Stop::with_deadline(t0 + timeout);
        let mut reply = Reply::default();
        let mut buf: Vec<u8> = Vec::new();
        let status = self
            .0
            .request(CHAT_COMPLETIONS, &body.to_string(), &stop, |bytes| {
                buf.extend_from_slice(bytes);
                while let Some(i) = buf.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = buf.drain(..=i).collect();
                    feed(&mut reply, &String::from_utf8_lossy(&line), t0);
                }
            });
        reply.wall_ms = ms(t0.elapsed());
        match status {
            Ok(s) => reply.status = s,
            Err(e) => reply.error = Some(e.to_string()),
        }
        if reply.status != 200 && reply.error.is_none() {
            reply.error = Some(format!(
                "status {}: {}",
                reply.status,
                String::from_utf8_lossy(&buf)
            ));
        }
        if stop.deadline_passed() && !reply.done {
            reply.error = Some("deadline".into());
        }
        reply
    }
}

struct Sidecar {
    child: Child,
    socket: PathBuf,
}

impl Sidecar {
    /// The app's `SpawnSpec::args` for a Unix socket.
    fn start(binary: &Path, config: &Config, dir: &Path) -> Self {
        let socket = dir.join("llm.sock");
        let _ = std::fs::remove_file(&socket);
        let mut args = config.server_args();
        args.remove(0);
        // `-m <model> --alias local`, then the listener, then the rest.
        args.splice(
            4..4,
            ["--host".to_owned(), socket.to_string_lossy().into_owned()],
        );
        let child = Command::new(binary)
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start llama-server");
        let me = Self { child, socket };
        let t0 = Instant::now();
        loop {
            if me.get("/health").is_some_and(|b| b.contains("\"ok\"")) {
                break;
            }
            assert!(t0.elapsed() < Duration::from_secs(60), "sidecar not ready");
            std::thread::sleep(Duration::from_millis(50));
        }
        me
    }

    fn get(&self, path: &str) -> Option<String> {
        let mut s = UnixStream::connect(&self.socket).ok()?;
        write!(
            s,
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .ok()?;
        let mut out = String::new();
        s.read_to_string(&mut out).ok()?;
        Some(out)
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

impl Backend for Sidecar {
    fn name(&self) -> &'static str {
        "sidecar"
    }
    fn chat(&mut self, body: &Value, timeout: Duration) -> Reply {
        http_chat(&self.socket, None, body, timeout)
    }
}

/// The engine behind its in-process listener (`wb_ios_llm::serve`), reached
/// the way the app reaches the sidecar: HTTP on a Unix socket, bearer key.
struct Listener {
    _server: wb_ios_llm::serve::Server,
    socket: PathBuf,
}

const LISTENER_KEY: &str = "eval-key";

impl Listener {
    fn start(config: &Config, dir: &Path) -> Self {
        use wb_ios_llm::serve::{Listen, Server, State};
        let socket = dir.join("inproc.sock");
        let server = Server::start(
            Listen::Unix(socket.clone()),
            Some(LISTENER_KEY.into()),
            config.clone(),
        )
        .expect("listener binds");
        let t0 = Instant::now();
        while server.state() != State::Ready {
            assert!(
                matches!(server.state(), State::Loading),
                "{:?}",
                server.state()
            );
            assert!(t0.elapsed() < Duration::from_secs(60), "listener not ready");
            std::thread::sleep(Duration::from_millis(20));
        }
        Self {
            _server: server,
            socket,
        }
    }
}

impl Backend for Listener {
    fn name(&self) -> &'static str {
        "listener"
    }
    fn chat(&mut self, body: &Value, timeout: Duration) -> Reply {
        http_chat(&self.socket, Some(LISTENER_KEY), body, timeout)
    }
}

/// One streamed chat completion over HTTP on a Unix socket.
fn http_chat(socket: &Path, key: Option<&str>, body: &Value, timeout: Duration) -> Reply {
    {
        let t0 = Instant::now();
        let mut reply = Reply::default();
        let payload = body.to_string();
        let run = || -> std::io::Result<Reply> {
            let mut s = UnixStream::connect(socket)?;
            s.set_read_timeout(Some(timeout))?;
            let auth = key
                .map(|k| format!("Authorization: Bearer {k}\r\n"))
                .unwrap_or_default();
            write!(
                s,
                "POST {CHAT_COMPLETIONS} HTTP/1.1\r\nHost: localhost\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload.len()
            )?;
            s.write_all(payload.as_bytes())?;
            let mut r = BufReader::new(s);
            let mut line = String::new();
            r.read_line(&mut line)?;
            let mut reply = Reply {
                status: line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|c| c.parse().ok())
                    .unwrap_or(0),
                ..Reply::default()
            };
            let mut rest = String::new();
            loop {
                line.clear();
                if r.read_line(&mut line)? == 0 {
                    break;
                }
                if reply.status == 200 {
                    // Chunk-size lines are not `data:` lines and are skipped.
                    feed(&mut reply, &line, t0);
                } else {
                    rest.push_str(&line);
                }
                if t0.elapsed() > timeout {
                    reply.error = Some("deadline".into());
                    break;
                }
            }
            if reply.status != 200 {
                reply.error = Some(format!("status {}: {}", reply.status, rest.trim()));
            }
            Ok(reply)
        };
        match run() {
            Ok(r) => reply = r,
            Err(e) => {
                reply.error = Some(
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) {
                        "deadline".into()
                    } else {
                        e.to_string()
                    },
                )
            }
        }
        reply.wall_ms = ms(t0.elapsed());
        reply
    }
}

struct Row {
    id: String,
    words: usize,
    raw: Option<String>,
    accepted: bool,
    reasons: Vec<&'static str>,
    reply: Reply,
    deadline_ms: f64,
}

fn run(
    backend: &mut dyn Backend,
    items: &[Value],
    out: &Path,
    model: &Path,
    no_deadline: bool,
) -> Vec<Row> {
    let system = prompt::cleanup_system();
    // One warm-up request (Metal pipelines, the system prompt in the cache),
    // not scored: `eval_run.rs` does the same.
    let warm = "um so this is a warm up";
    let _ = backend.chat(
        &body(
            &system,
            &prompt::cleanup_user(warm),
            prompt::max_tokens(warm),
        ),
        Duration::from_secs(120),
    );
    let mut file = std::fs::File::create(out).expect("out file");
    writeln!(
        file,
        "{}",
        json!({ "meta": {
            "runner": format!("src-tauri/crates/wb-ios-llm/examples/eval.rs ({}; one request per item, no gate, no chunking)", backend.name()),
            "model": model.file_name().map(|f| f.to_string_lossy().to_string()),
            "engine": backend.name(),
            "items": items.len(),
            "levels": ["light"],
            "language_hint": false,
            "language_mode": "none",
            "style": Value::Null,
            "system_prompt_light": system,
        }})
    )
    .unwrap();
    let mut rows = Vec::new();
    for item in items {
        let input = item["input"].as_str().unwrap_or_default();
        let (stripped, _) = leadins::strip(input, None);
        let core = stripped.trim();
        let timeout = if no_deadline {
            Duration::from_secs(120)
        } else {
            deadline::request(core)
        };
        let reply = backend.chat(
            &body(
                &system,
                &prompt::cleanup_user(core),
                prompt::max_tokens(core),
            ),
            timeout,
        );
        let timed_out = reply.error.as_deref() == Some("deadline");
        let truncated = reply.finish.as_deref() == Some("length");
        let raw = (reply.error.is_none() && !truncated).then(|| reply.content.clone());
        let verdict = raw.as_deref().map(|raw| {
            guards::check(
                raw,
                &guards::Context {
                    input: core,
                    level: CleanupLevel::Light,
                    language: None,
                    system_prompt: &system,
                    terms: &[],
                },
            )
        });
        let accepted = verdict.as_ref().is_some_and(|v| v.accepted());
        let reasons: Vec<&'static str> = verdict
            .as_ref()
            .map(|v| v.reasons.iter().map(|r| r.as_str()).collect())
            .unwrap_or_default();
        let text = verdict.as_ref().and_then(|v| v.text.clone());
        // `post_process`: an accepted reply is inserted, anything else falls
        // back to the deterministic text with its lead-ins stripped.
        let output = verdict
            .as_ref()
            .and_then(|v| v.accepted_text())
            .map(str::to_owned)
            .unwrap_or_else(|| leadins::strip_all(core, None).0);
        let pre = text.clone().unwrap_or_else(|| core.to_owned());
        let expected = item["expected"]["light"].as_str().unwrap_or_default();
        let lang = |t: &str| lang_id::detect_output_language(t, &[]);
        let error = reply
            .error
            .clone()
            .filter(|_| !timed_out)
            .or(truncated.then(|| "reply cut off at the token limit".to_owned()));
        writeln!(
            file,
            "{}",
            json!({
                "id": item["id"],
                "level": "light",
                "language": item["language"],
                "language_used": Value::Null,
                "input": input,
                "output": output,
                "pre": pre,
                "outcome": if accepted { "applied" } else if timed_out { "timeout" } else if error.is_some() { "error" } else { "guard" },
                "why": Value::Null,
                "llm_ms": reply.wall_ms,
                "lang_id": { "expected": lang(expected), "output": lang(&output), "pre": lang(&pre) },
                "wall_ms": reply.wall_ms,
                "chunks": [{
                    "input": core,
                    "raw": raw,
                    "text": text,
                    "accepted": accepted,
                    "reasons": reasons,
                    "ms": reply.wall_ms,
                    "deadline_ms": ms(timeout),
                    "error": error,
                    "timed_out": timed_out,
                }],
                "timings": {
                    "prompt_n": reply.prompt_n, "prompt_ms": reply.prompt_ms,
                    "predicted_n": reply.predicted_n, "predicted_ms": reply.predicted_ms,
                    "cache_n": reply.cache_n, "first_ms": reply.first_ms,
                },
            })
        )
        .unwrap();
        rows.push(Row {
            id: item["id"].as_str().unwrap_or_default().to_owned(),
            words: core.split_whitespace().count(),
            raw,
            accepted,
            reasons,
            reply,
            deadline_ms: ms(timeout),
        });
        if rows.len() % 50 == 0 {
            eprintln!("eval: {} {}/{}", backend.name(), rows.len(), items.len());
        }
    }
    rows
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

fn stats(rows: &[Row], filter: impl Fn(&Row) -> bool) -> Value {
    let mut wall: Vec<f64> = rows
        .iter()
        .filter(|r| filter(r))
        .map(|r| r.reply.wall_ms)
        .collect();
    wall.sort_by(f64::total_cmp);
    let picked: Vec<&Row> = rows.iter().filter(|r| filter(r)).collect();
    let n = picked.len().max(1) as f64;
    let sum = |f: &dyn Fn(&Row) -> f64| picked.iter().map(|r| f(r)).sum::<f64>();
    json!({
        "n": picked.len(),
        "wall_ms_p50": percentile(&wall, 50.0),
        "wall_ms_p95": percentile(&wall, 95.0),
        "wall_ms_max": wall.last().copied(),
        "prompt_ms_mean": sum(&|r| r.reply.prompt_ms) / n,
        "predicted_ms_mean": sum(&|r| r.reply.predicted_ms) / n,
        "tokens_per_s": sum(&|r| r.reply.predicted_n as f64) / (sum(&|r| r.reply.predicted_ms) / 1000.0),
        "cached_prompt_tokens_mean": sum(&|r| r.reply.cache_n as f64) / n,
        "new_prompt_tokens_mean": sum(&|r| r.reply.prompt_n as f64) / n,
    })
}

fn summary(rows: &[Row]) -> Value {
    let n = rows.len().max(1) as f64;
    let accepted = rows.iter().filter(|r| r.accepted).count();
    let timeouts = rows
        .iter()
        .filter(|r| r.reply.error.as_deref() == Some("deadline"))
        .count();
    let errors = rows.iter().filter(|r| r.reply.error.is_some()).count() - timeouts;
    let mut reasons: std::collections::BTreeMap<&str, usize> = Default::default();
    for r in rows {
        for reason in &r.reasons {
            *reasons.entry(reason).or_default() += 1;
        }
    }
    json!({
        "items": rows.len(),
        "guard_accepted": accepted,
        "guard_pass_rate": accepted as f64 / n,
        "guard_reasons": reasons,
        "timeouts": timeouts,
        "errors": errors,
        "latency_all": stats(rows, |_| true),
        "latency_40_to_60_words": stats(rows, |r| (40..=60).contains(&r.words)),
        "latency_25_and_more_words": stats(rows, |r| r.words >= 25),
    })
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let model = PathBuf::from(arg(&args, "--model").expect("--model"));
    let items_path = PathBuf::from(arg(&args, "--items").expect("--items"));
    let out = PathBuf::from(arg(&args, "--out").expect("--out"));
    let sidecar = arg(&args, "--sidecar").map(PathBuf::from);
    let limit: Option<usize> = arg(&args, "--limit").and_then(|v| v.parse().ok());
    let no_engine = args.iter().any(|a| a == "--no-engine");
    let no_deadline = args.iter().any(|a| a == "--no-deadline");
    std::fs::create_dir_all(&out).expect("out dir");

    let mut items: Vec<Value> = std::fs::read_to_string(&items_path)
        .expect("items")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("item json"))
        .collect();
    if let Some(extra) = arg(&args, "--extra-items") {
        // More items in the same format (the ~50-word takes).
        items.extend(
            std::fs::read_to_string(extra)
                .expect("extra items")
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| serde_json::from_str::<Value>(l).expect("item json")),
        );
    }
    if let Some(n) = limit {
        items.truncate(n);
    }

    let mut config = Config::new(&model);
    if args.iter().any(|a| a == "--cpu") {
        config.device = Device::Cpu;
    }
    if let Some(extra) = arg(&args, "--extra-args") {
        // llama-server arguments appended to the sidecar's (measurements).
        config.extra_args = extra.split_whitespace().map(str::to_owned).collect();
    }
    if let Some(n) = arg(&args, "--threads").and_then(|v| v.parse().ok()) {
        config.threads = n;
    }
    let mut report = json!({
        "llama_cpp": wb_ios_llm::build_info(),
        "model": model.file_name().map(|f| f.to_string_lossy().to_string()),
        "device": format!("{:?}", config.device),
        "server_args": config.server_args(),
        "physical_memory_bytes": wb_ios_llm::physical_memory_bytes(),
    });

    let mut ours: Option<Vec<Row>> = None;
    if !no_engine {
        let before = wb_ios_llm::footprint_bytes().unwrap_or(0);
        let t0 = Instant::now();
        let engine = Engine::load(&config).expect("model loads");
        let load_ms = ms(t0.elapsed());
        let model_bytes = engine.model_bytes();
        let mut backend = InProcess(engine);
        let rows = run(
            &mut backend,
            &items,
            &out.join("inprocess.jsonl"),
            &model,
            no_deadline,
        );
        let loaded = wb_ios_llm::footprint_bytes().unwrap_or(0);
        let t1 = Instant::now();
        drop(backend);
        let unload_ms = ms(t1.elapsed());
        let after = wb_ios_llm::footprint_bytes().unwrap_or(0);
        report["inprocess"] = summary(&rows);
        report["inprocess"]["load_ms"] = json!(load_ms);
        report["inprocess"]["unload_ms"] = json!(unload_ms);
        report["inprocess"]["model_bytes"] = json!(model_bytes);
        report["inprocess"]["footprint_bytes"] =
            json!({ "before_load": before, "loaded_after_run": loaded, "after_unload": after });
        ours = Some(rows);
    }

    let mut listened: Option<Vec<Row>> = None;
    if args.iter().any(|a| a == "--listener") {
        let dir = std::env::temp_dir().join(format!("wb-ios-llm-listen-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("socket dir");
        let t0 = Instant::now();
        let mut backend = Listener::start(&config, &dir);
        let start_ms = ms(t0.elapsed());
        let rows = run(
            &mut backend,
            &items,
            &out.join("listener.jsonl"),
            &model,
            no_deadline,
        );
        drop(backend);
        let _ = std::fs::remove_dir_all(&dir);
        report["listener"] = summary(&rows);
        report["listener"]["start_ms"] = json!(start_ms);
        listened = Some(rows);
    }

    let mut theirs: Option<Vec<Row>> = None;
    if let Some(binary) = sidecar {
        let dir = std::env::temp_dir().join(format!("wb-ios-llm-eval-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("socket dir");
        let t0 = Instant::now();
        let mut backend = Sidecar::start(&binary, &config, &dir);
        let start_ms = ms(t0.elapsed());
        let rows = run(
            &mut backend,
            &items,
            &out.join("sidecar.jsonl"),
            &model,
            no_deadline,
        );
        drop(backend);
        let _ = std::fs::remove_dir_all(&dir);
        report["sidecar"] = summary(&rows);
        report["sidecar"]["start_ms"] = json!(start_ms);
        theirs = Some(rows);
    }

    if let (Some(a), Some(b)) = (&listened, &theirs) {
        let same = a.iter().zip(b).filter(|(x, y)| x.raw == y.raw).count();
        let verdicts = a
            .iter()
            .zip(b)
            .filter(|(x, y)| x.accepted == y.accepted && x.reasons == y.reasons)
            .count();
        report["comparison_listener_vs_sidecar"] = json!({
            "items": a.len(),
            "identical_raw_replies": same,
            "identical_guard_verdicts": verdicts,
        });
    }
    if let (Some(a), Some(b)) = (&ours, &theirs) {
        let mut same_raw = 0;
        let mut same_verdict = 0;
        let mut different: Vec<Value> = Vec::new();
        for (x, y) in a.iter().zip(b) {
            if x.raw == y.raw {
                same_raw += 1;
            } else if different.len() < 20 {
                different.push(json!({ "id": x.id, "inprocess": x.raw, "sidecar": y.raw,
                    "inprocess_error": x.reply.error, "sidecar_error": y.reply.error }));
            }
            if x.accepted == y.accepted && x.reasons == y.reasons {
                same_verdict += 1;
            }
        }
        report["comparison"] = json!({
            "items": a.len(),
            "identical_raw_replies": same_raw,
            "identical_guard_verdicts": same_verdict,
            "different": different,
        });
    }
    std::fs::write(
        out.join("summary.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .expect("summary");
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
