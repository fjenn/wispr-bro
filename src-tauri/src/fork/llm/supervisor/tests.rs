// SPDX-License-Identifier: GPL-3.0-or-later
//! Supervisor tests (S-10). Ported from the TypeWhisper acceptance list
//! (`LlamaCppPluginTests.swift`) where it applies, plus R5's rules: spawn,
//! health timeout, restart once, CPU fallback, backoff, bind race, crash
//! restart, idle stop, wake, abort, model change.
//!
//! The fake launcher runs an in-process HTTP server on the endpoint the
//! supervisor chose (Unix socket or loopback TCP), so the real transport,
//! health poll and chat client are exercised.

use super::*;
use std::sync::atomic::AtomicBool;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// How one fake server behaves.
#[derive(Clone, Copy, Debug)]
enum Behavior {
    /// `/health` is 503 for `load_ms`, then 200.
    Healthy { load_ms: u64 },
    /// The process exits during load with this stderr.
    ExitDuringLoad,
    /// Never becomes healthy.
    NeverHealthy,
    /// Exits with llama-server's bind error.
    BindFail,
}

#[derive(Default)]
struct Shared {
    /// Behaviours for the next launches; the last one repeats.
    plan: Vec<Behavior>,
    launches: Vec<SpawnSpec>,
    /// Chat requests seen.
    chats: u32,
    /// Chat replies take this long.
    chat_delay_ms: u64,
    /// Connections the server saw closed before it answered.
    aborted: u32,
    /// `/health` answers 500 (hung server) when set.
    hung: bool,
}

#[derive(Clone, Default)]
struct FakeLauncher(Arc<Mutex<Shared>>);

/// The `on_exit` callback, taken once.
type ExitSlot = Arc<Mutex<Option<Box<dyn FnOnce() + Send>>>>;

struct FakeServer {
    pid: u32,
    exited: Arc<AtomicBool>,
    on_exit: ExitSlot,
    task: tokio::task::JoinHandle<()>,
    tail: OutputTail,
}

impl FakeServer {
    fn exit(&mut self) {
        self.task.abort();
        if !self.exited.swap(true, Ordering::AcqRel) {
            if let Some(f) = lock(&self.on_exit).take() {
                f();
            }
        }
    }
}

impl Server for FakeServer {
    fn pid(&self) -> u32 {
        self.pid
    }
    fn has_exited(&mut self) -> bool {
        self.exited.load(Ordering::Acquire)
    }
    fn stop(&mut self) {
        self.exit();
    }
    fn tail(&self) -> OutputTail {
        self.tail.clone()
    }
}

static NEXT_PID: AtomicU64 = AtomicU64::new(1000);

/// A registry of live fake servers so a test can crash one.
type Crash = Arc<
    Mutex<
        Vec<(
            Arc<AtomicBool>,
            Arc<Mutex<Option<Box<dyn FnOnce() + Send>>>>,
        )>,
    >,
>;

#[derive(Clone, Default)]
struct Harness {
    launcher: FakeLauncher,
    live: Crash,
}

impl Launcher for Harness {
    fn launch(
        &self,
        spec: &SpawnSpec,
        on_exit: Box<dyn FnOnce() + Send>,
    ) -> io::Result<Box<dyn Server>> {
        let behavior = {
            let mut s = lock(&self.launcher.0);
            s.launches.push(spec.clone());
            let i = s.launches.len() - 1;
            *s.plan
                .get(i)
                .or(s.plan.last())
                .unwrap_or(&Behavior::Healthy { load_ms: 0 })
        };
        let exited = Arc::new(AtomicBool::new(false));
        let on_exit = Arc::new(Mutex::new(Some(on_exit)));
        let mut tail = OutputTail::default();
        tail.push_line("build: 11624 (af0579c78)");
        let shared = self.launcher.0.clone();
        let ready_at = match behavior {
            Behavior::Healthy { load_ms } => Some(Instant::now() + Duration::from_millis(load_ms)),
            _ => None,
        };
        match behavior {
            Behavior::ExitDuringLoad => {
                tail.push_line("llama_model_load: error loading model: bad file");
                tail.push_line("main: exiting due to model loading error");
            }
            Behavior::BindFail => {
                tail.push_line("couldn't bind HTTP server socket, hostname: 127.0.0.1, port: 1");
            }
            _ => {}
        }
        let endpoint = spec.transport.endpoint.clone();
        let key = spec.transport.api_key().to_string();
        let task = if matches!(behavior, Behavior::ExitDuringLoad | Behavior::BindFail) {
            exited.store(true, Ordering::Release);
            tokio::spawn(async {})
        } else {
            tokio::spawn(serve(endpoint, key, ready_at, shared))
        };
        lock(&self.live).push((exited.clone(), on_exit.clone()));
        Ok(Box::new(FakeServer {
            pid: NEXT_PID.fetch_add(1, Ordering::Relaxed) as u32,
            exited,
            on_exit,
            task,
            tail,
        }))
    }
}

impl Harness {
    fn plan(&self, plan: &[Behavior]) {
        lock(&self.launcher.0).plan = plan.to_vec();
    }
    fn launches(&self) -> Vec<SpawnSpec> {
        lock(&self.launcher.0).launches.clone()
    }
    fn shared(&self) -> std::sync::MutexGuard<'_, Shared> {
        lock(&self.launcher.0)
    }
    /// Kills the newest live server as a crash would (`on_exit` fires).
    fn crash_latest(&self) {
        let (exited, on_exit) = lock(&self.live).last().cloned().unwrap();
        if !exited.swap(true, Ordering::AcqRel) {
            if let Some(f) = lock(&on_exit).take() {
                f();
            }
        }
    }
}

async fn serve(
    endpoint: Endpoint,
    key: String,
    ready_at: Option<Instant>,
    shared: Arc<Mutex<Shared>>,
) {
    match endpoint {
        Endpoint::Unix(path) => {
            let l = tokio::net::UnixListener::bind(&path).unwrap();
            loop {
                let Ok((s, _)) = l.accept().await else { return };
                tokio::spawn(handle(s, key.clone(), ready_at, shared.clone()));
            }
        }
        Endpoint::Tcp(port) => {
            let l = tokio::net::TcpListener::bind(("127.0.0.1", port))
                .await
                .unwrap();
            loop {
                let Ok((s, _)) = l.accept().await else { return };
                tokio::spawn(handle(s, key.clone(), ready_at, shared.clone()));
            }
        }
    }
}

async fn handle<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
    mut s: S,
    key: String,
    ready_at: Option<Instant>,
    shared: Arc<Mutex<Shared>>,
) {
    loop {
        let mut head = Vec::new();
        let mut buf = [0u8; 4096];
        let body_start;
        loop {
            match s.read(&mut buf).await {
                Ok(0) | Err(_) => return,
                Ok(n) => head.extend_from_slice(&buf[..n]),
            }
            if let Some(i) = head.windows(4).position(|w| w == b"\r\n\r\n") {
                body_start = i + 4;
                break;
            }
        }
        let text = String::from_utf8_lossy(&head[..body_start]).to_string();
        let len: usize = text
            .lines()
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                k.eq_ignore_ascii_case("content-length")
                    .then(|| v.trim().parse().ok())?
            })
            .unwrap_or(0);
        let mut body = head[body_start..].to_vec();
        while body.len() < len {
            match s.read(&mut buf).await {
                Ok(0) | Err(_) => return,
                Ok(n) => body.extend_from_slice(&buf[..n]),
            }
        }
        let authorized = text
            .lines()
            .any(|l| l.eq_ignore_ascii_case(&format!("authorization: Bearer {key}")));
        let (code, reply) = if text.starts_with("GET /health") {
            let hung = lock(&shared).hung;
            match ready_at {
                _ if hung => (
                    "500 Internal Server Error",
                    r#"{"error":"hung"}"#.to_string(),
                ),
                Some(t) if Instant::now() >= t => ("200 OK", r#"{"status":"ok"}"#.to_string()),
                _ => (
                    "503 Service Unavailable",
                    r#"{"error":{"code":503,"message":"Loading model"}}"#.to_string(),
                ),
            }
        } else if text.starts_with("POST /v1/chat/completions") {
            if !authorized {
                (
                    "401 Unauthorized",
                    r#"{"error":{"message":"Invalid API Key"}}"#.into(),
                )
            } else {
                let delay = {
                    let mut sh = lock(&shared);
                    sh.chats += 1;
                    sh.chat_delay_ms
                };
                if delay > 0 {
                    // Generation in progress: notice a client that hangs up.
                    let mut probe = [0u8; 1];
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(delay)) => {}
                        r = s.read(&mut probe) => {
                            if matches!(r, Ok(0) | Err(_)) {
                                lock(&shared).aborted += 1;
                                return;
                            }
                        }
                    }
                }
                // Streamed, as llama-server answers `stream: true`.
                (
                    "200 OK",
                    concat!(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"{\\\"text\\\":\\\"Hi.\\\"}\"}}]}\n\n",
                        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"timings\":{\"prompt_n\":10,\"prompt_ms\":5.0,\"predicted_n\":4,\"predicted_ms\":8.0}}\n\n",
                        "data: [DONE]\n\n"
                    )
                    .into(),
                )
            }
        } else {
            ("404 Not Found", "{}".into())
        };
        let resp = format!(
            "HTTP/1.1 {code}\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{reply}",
            reply.len()
        );
        if s.write_all(resp.as_bytes()).await.is_err() {
            return;
        }
    }
}

fn setup(plan: &[Behavior]) -> (Harness, Arc<Supervisor>, tempfile::TempDir) {
    let h = Harness::default();
    h.plan(plan);
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::new("/fake/llama-server".into(), dir.path().join("llm"));
    cfg.model = Some(dir.path().join("m.gguf"));
    cfg.health_timeout = Duration::from_millis(400);
    let sup = Supervisor::new(Arc::new(h.clone()), cfg);
    (h, sup, dir)
}

fn has_ngl0(spec: &SpawnSpec) -> bool {
    spec.args()
        .windows(2)
        .any(|w| w[0] == "-ngl" && w[1] == "0")
}

fn sample() -> ChatRequest {
    ChatRequest {
        system: "s".into(),
        user: "u".into(),
        max_tokens: 16,
        response_format: None,
        examples: Vec::new(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn spawns_once_and_reuses_the_server() {
    let (h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 60 }]);
    let a = sup.ensure_ready().await.unwrap();
    let b = sup.ensure_ready().await.unwrap();
    assert_eq!(h.launches().len(), 1);
    assert_eq!(a.transport, b.transport);
    let st = sup.status();
    assert_eq!(st.phase, ServerPhase::Ready);
    assert_eq!(st.transport.as_deref(), Some("unix"));
    assert!(st.ready_ms.unwrap() >= 50);
    // The real client reaches the fake over the socket, with the key.
    let r = sup
        .complete(&sample(), Duration::from_secs(5), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(r.content, r#"{"text":"Hi."}"#);
    assert_eq!(h.shared().chats, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_callers_share_one_start() {
    let (h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 150 }]);
    let (a, b, c) = tokio::join!(sup.ensure_ready(), sup.ensure_ready(), sup.ensure_ready());
    assert!(a.is_ok() && b.is_ok() && c.is_ok());
    assert_eq!(h.launches().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn health_timeout_restarts_once_then_cpu_then_backs_off() {
    let (h, sup, _d) = setup(&[Behavior::NeverHealthy]);
    let err = sup.ensure_ready().await.unwrap_err();
    assert!(
        matches!(err, StartError::Failed(ref m) if m.contains("no /health")),
        "{err:?}"
    );
    let l = h.launches();
    assert_eq!(l.len(), 3, "first try, one restart, one CPU try");
    assert!(!has_ngl0(&l[0]) && !has_ngl0(&l[1]));
    assert!(has_ngl0(&l[2]));
    let st = sup.status();
    assert_eq!(st.phase, ServerPhase::Failed);
    assert_eq!(st.device, "cpu");
    // During the backoff nothing is launched.
    match sup.ensure_ready().await {
        Err(StartError::Backoff(d)) => assert!(d > Duration::from_secs(25)),
        other => panic!("{:?}", other.err()),
    }
    assert_eq!(h.launches().len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn exit_during_load_names_the_last_error_line() {
    let (h, sup, _d) = setup(&[Behavior::ExitDuringLoad]);
    let err = sup.ensure_ready().await.unwrap_err();
    assert_eq!(
        err,
        StartError::Failed(
            "server exited while loading: main: exiting due to model loading error".into()
        )
    );
    assert_eq!(h.launches().len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn gpu_failure_then_cpu_success() {
    let (h, sup, _d) = setup(&[
        Behavior::ExitDuringLoad,
        Behavior::ExitDuringLoad,
        Behavior::Healthy { load_ms: 0 },
    ]);
    sup.ensure_ready().await.unwrap();
    let l = h.launches();
    assert_eq!(l.len(), 3);
    assert!(has_ngl0(&l[2]));
    assert_eq!(sup.status().device, "cpu");
    // CPU stays chosen for later starts in this process.
    sup.stop().await;
    sup.ensure_ready().await.unwrap();
    assert!(has_ngl0(h.launches().last().unwrap()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lost_port_race_retries_without_counting() {
    let (h, sup, _d) = setup(&[
        Behavior::BindFail,
        Behavior::BindFail,
        Behavior::Healthy { load_ms: 0 },
    ]);
    sup.ensure_ready().await.unwrap();
    let l = h.launches();
    assert_eq!(l.len(), 3);
    assert!(
        l.iter().all(|s| !has_ngl0(s)),
        "a bind race is not a GPU failure"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_crash_after_ready_restarts_once_then_waits() {
    let (h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 0 }]);
    sup.ensure_ready().await.unwrap();
    let first = sup.pid().await.unwrap();
    h.crash_latest();
    // The background restart.
    let t0 = Instant::now();
    loop {
        if sup.pid().await.is_some_and(|p| p != first) {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(3), "no restart");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(h.launches().len(), 2);
    assert_eq!(sup.status().restarts, 1);
    // A second crash within a minute: no automatic restart, backoff.
    h.crash_latest();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(h.launches().len(), 2);
    assert!(!sup.is_running().await);
    assert_eq!(sup.status().phase, ServerPhase::Failed);
    assert!(matches!(
        sup.ensure_ready().await,
        Err(StartError::Backoff(_))
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_we_stopped_is_not_restarted() {
    let (h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 0 }]);
    sup.ensure_ready().await.unwrap();
    sup.stop().await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(h.launches().len(), 1);
    assert_eq!(sup.status().phase, ServerPhase::Stopped);
    assert_eq!(sup.status().restarts, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn idle_stop_after_the_timeout_but_never_mid_request() {
    let (h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 0 }]);
    {
        let mut c = lock(&sup.config);
        c.idle_timeout = Duration::from_millis(150);
    }
    sup.ensure_ready().await.unwrap();
    assert!(!sup.idle_tick().await, "just used");
    // A long request keeps it alive past the timeout.
    h.shared().chat_delay_ms = 400;
    let s2 = sup.clone();
    let req = tokio::spawn(async move {
        s2.complete(&sample(), Duration::from_secs(5), &CancellationToken::new())
            .await
    });
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(!sup.idle_tick().await, "request in flight");
    req.await.unwrap().unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(sup.idle_tick().await);
    assert!(!sup.is_running().await);
    assert_eq!(sup.status().phase, ServerPhase::Stopped);
    // The next take starts it again.
    h.shared().chat_delay_ms = 0;
    sup.complete(&sample(), Duration::from_secs(5), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(h.launches().len(), 2);
}

/// S-38: under critical memory pressure the idle owner stops an idle
/// server at once (no idle timeout), never one with a request in flight.
#[tokio::test(flavor = "multi_thread")]
async fn pressure_stop_skips_the_timeout_but_never_a_request() {
    let (h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 0 }]);
    assert!(!sup.stop_if_idle().await, "nothing running");
    sup.ensure_ready().await.unwrap();
    h.shared().chat_delay_ms = 400;
    let s2 = sup.clone();
    let req = tokio::spawn(async move {
        s2.complete(&sample(), Duration::from_secs(5), &CancellationToken::new())
            .await
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(!sup.stop_if_idle().await, "request in flight");
    req.await.unwrap().unwrap();
    // Just used, well inside the 120 s idle timeout: stopped anyway.
    assert!(sup.stop_if_idle().await);
    assert!(!sup.is_running().await);
    assert_eq!(sup.status().phase, ServerPhase::Stopped);
    assert_eq!(sup.status().restarts, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn wake_restarts_a_server_that_does_not_answer() {
    let (h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 0 }]);
    sup.ensure_ready().await.unwrap();
    sup.on_wake().await;
    assert_eq!(h.launches().len(), 1, "healthy: kept");
    h.shared().hung = true;
    let s2 = sup.clone();
    let wake = tokio::spawn(async move { s2.on_wake().await });
    // The new server is hung too until we clear the flag.
    tokio::time::sleep(Duration::from_millis(100)).await;
    h.shared().hung = false;
    wake.await.unwrap();
    assert!(h.launches().len() >= 2);
    assert!(sup.is_running().await);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_aborts_the_request_and_frees_the_slot() {
    let (h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 0 }]);
    sup.ensure_ready().await.unwrap();
    h.shared().chat_delay_ms = 5_000;
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(80)).await;
        c2.cancel();
    });
    let t0 = Instant::now();
    let r = sup
        .complete(&sample(), Duration::from_secs(30), &cancel)
        .await;
    let took = t0.elapsed();
    assert_eq!(r.unwrap_err(), RequestError::Chat(ChatError::Cancelled));
    assert!(took < Duration::from_millis(80 + 100), "{took:?}");
    let t1 = Instant::now();
    while h.shared().aborted == 0 {
        assert!(
            t1.elapsed() < Duration::from_secs(1),
            "server never saw the hang-up"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(t1.elapsed() < Duration::from_millis(100));
    // The server is still usable.
    h.shared().chat_delay_ms = 0;
    sup.complete(&sample(), Duration::from_secs(5), &CancellationToken::new())
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn no_model_means_no_start_and_a_model_change_stops_the_server() {
    let (h, sup, d) = setup(&[Behavior::Healthy { load_ms: 0 }]);
    sup.set_model(None).await;
    assert_eq!(sup.ensure_ready().await.err(), Some(StartError::NoModel));
    assert!(h.launches().is_empty());
    sup.set_model(Some(d.path().join("a.gguf"))).await;
    sup.ensure_ready().await.unwrap();
    sup.set_model(Some(d.path().join("b.gguf"))).await;
    assert!(!sup.is_running().await);
    sup.ensure_ready().await.unwrap();
    let l = h.launches();
    assert_eq!(l[1].model, d.path().join("b.gguf"));
}

#[tokio::test(flavor = "multi_thread")]
async fn long_data_dirs_use_loopback_tcp_with_the_key() {
    let h = Harness::default();
    h.plan(&[Behavior::Healthy { load_ms: 0 }]);
    let dir = tempfile::tempdir().unwrap();
    let deep = dir.path().join("x".repeat(120));
    let mut cfg = Config::new("/fake/llama-server".into(), deep);
    cfg.model = Some(dir.path().join("m.gguf"));
    let sup = Supervisor::new(Arc::new(h.clone()), cfg);
    sup.complete(&sample(), Duration::from_secs(5), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(sup.status().transport.as_deref(), Some("tcp"));
    let args = h.launches()[0].args();
    assert!(args
        .windows(2)
        .any(|w| w[0] == "--host" && w[1] == "127.0.0.1"));
}

// ---------------------------------------------------------------------------
// Real `llama-server` (RS-06). Ignored by default; run with
//   WB_LLAMA_SERVER=<path> WB_LLM_MODEL=<gguf> cargo test -- --ignored real_
// The RS-06 report quotes their output.

fn real_env() -> Option<(PathBuf, PathBuf)> {
    Some((
        std::env::var_os("WB_LLAMA_SERVER")?.into(),
        std::env::var_os("WB_LLM_MODEL")?.into(),
    ))
}

fn real_supervisor(dir: &std::path::Path) -> Option<Arc<Supervisor>> {
    let (bin, model) = real_env()?;
    let mut cfg = Config::new(bin, dir.join("llm"));
    cfg.model = Some(model);
    Some(Supervisor::new(Arc::new(ProcessLauncher), cfg))
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs WB_LLAMA_SERVER and WB_LLM_MODEL"]
async fn real_server_on_the_socket_cleans_a_sentence() {
    let dir = tempfile::tempdir().unwrap();
    let Some(sup) = real_supervisor(dir.path()) else {
        return;
    };
    sup.ensure_ready().await.unwrap();
    assert_eq!(sup.status().transport.as_deref(), Some("unix"));
    let pid = sup.pid().await.unwrap();
    assert!(dir.path().join("llm").join(process::PID_FILE).exists());
    let text = "um so I was thinking we could uh meet on friday no wait thursday";
    let req = ChatRequest {
        system: super::super::prompt::cleanup_system(),
        user: super::super::prompt::cleanup_user(text),
        max_tokens: super::super::prompt::max_tokens(text),
        response_format: Some(super::super::prompt::response_format()),
        examples: Vec::new(),
    };
    let r = sup
        .complete(&req, Duration::from_secs(30), &CancellationToken::new())
        .await
        .unwrap();
    let cleaned = super::super::prompt::parse_reply(&r.content).unwrap();
    eprintln!("cleaned: {cleaned:?} in {:?}", r.total);
    assert!(!cleaned.is_empty());
    // No inet socket while on the Unix socket.
    let out = std::process::Command::new("/usr/sbin/lsof")
        .args(["-a", "-i", "-p", &pid.to_string()])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        "inet sockets: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    sup.stop().await;
    assert!(!process::is_alive(pid as i32));
    assert!(!dir.path().join("llm").join(process::PID_FILE).exists());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs WB_LLAMA_SERVER and WB_LLM_MODEL"]
async fn real_server_killed_mid_request_fails_open_and_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let Some(sup) = real_supervisor(dir.path()) else {
        return;
    };
    sup.ensure_ready().await.unwrap();
    let pid = sup.pid().await.unwrap();
    let long = "word ".repeat(300);
    let req = ChatRequest {
        system: "Repeat the user's text exactly.".into(),
        user: long,
        max_tokens: 1500,
        response_format: None,
        examples: Vec::new(),
    };
    let s2 = sup.clone();
    let task = tokio::spawn(async move {
        s2.complete(&req, Duration::from_secs(60), &CancellationToken::new())
            .await
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    let t0 = Instant::now();
    // SAFETY: kill(2) on the server we started.
    unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    let r = task.await.unwrap();
    eprintln!("request after kill: {r:?} in {:?}", t0.elapsed());
    assert!(matches!(
        r,
        Err(RequestError::Chat(ChatError::Transport(_)))
    ));
    let t1 = Instant::now();
    loop {
        if sup.pid().await.is_some_and(|p| p != pid) && sup.status().phase == ServerPhase::Ready {
            break;
        }
        assert!(t1.elapsed() < Duration::from_secs(30), "no restart");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    eprintln!("restarted in {:?}", t1.elapsed());
    sup.stop().await;
}

// ---------------------------------------------------------------------------
// Mac wave 3 (finding 3): the first take after a fresh start waited 19 s.
// The launch warm-up stopped the server a take was waiting for, the take's
// memory check waited behind the start, and a take's start had no deadline.

#[tokio::test(flavor = "multi_thread")]
async fn warm_up_alone_stops_its_server() {
    let (h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 50 }]);
    assert!(!sup.warm_up().await.unwrap(), "nothing wanted it");
    assert!(!sup.is_running().await);
    assert_eq!(sup.status().phase, ServerPhase::Stopped);
    assert_eq!(h.launches().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_take_that_starts_during_the_warm_up_keeps_its_server() {
    let (h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 300 }]);
    let s2 = sup.clone();
    let warm = tokio::spawn(async move { s2.warm_up().await });
    tokio::time::sleep(Duration::from_millis(80)).await;
    // Key-down: the take starts and its prewarm waits for the same start.
    sup.take_started();
    let s3 = sup.clone();
    let prewarm = tokio::spawn(async move { s3.ensure_ready().await });
    assert!(warm.await.unwrap().unwrap(), "kept for the take");
    prewarm.await.unwrap().unwrap();
    assert!(sup.is_running().await);
    assert_eq!(h.launches().len(), 1, "one start, shared");
    // The take's request uses it at once.
    let t0 = Instant::now();
    sup.complete(&sample(), Duration::from_secs(5), &CancellationToken::new())
        .await
        .unwrap();
    assert!(t0.elapsed() < Duration::from_millis(200));
    assert_eq!(h.launches().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_waiting_on_the_warm_up_keeps_its_server() {
    let (h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 300 }]);
    let s2 = sup.clone();
    let warm = tokio::spawn(async move { s2.warm_up().await });
    tokio::time::sleep(Duration::from_millis(80)).await;
    let s3 = sup.clone();
    let req = tokio::spawn(async move {
        s3.complete(&sample(), Duration::from_secs(5), &CancellationToken::new())
            .await
    });
    assert!(warm.await.unwrap().unwrap());
    req.await.unwrap().unwrap();
    assert!(sup.is_running().await);
    assert_eq!(h.launches().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn is_running_never_waits_for_a_start() {
    let (_h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 600 }]);
    {
        lock(&sup.config).health_timeout = Duration::from_secs(5);
    }
    assert!(!sup.is_running().await);
    let s2 = sup.clone();
    let start = tokio::spawn(async move { s2.ensure_ready().await });
    tokio::time::sleep(Duration::from_millis(80)).await;
    let t0 = Instant::now();
    assert!(sup.is_running().await, "a start in progress counts");
    assert!(
        t0.elapsed() < Duration::from_millis(100),
        "{:?}",
        t0.elapsed()
    );
    start.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_take_that_gives_up_on_a_cold_start_leaves_it_running() {
    let (h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 500 }]);
    {
        lock(&sup.config).health_timeout = Duration::from_secs(5);
    }
    // The take's budget runs out first; it pastes without cleanup.
    let t0 = Instant::now();
    let gave_up = tokio::time::timeout(Duration::from_millis(100), sup.ensure_ready()).await;
    assert!(gave_up.is_err());
    assert!(t0.elapsed() < Duration::from_millis(300));
    // The start was not cut off (no half-started process): it finishes and
    // the next take reuses it.
    let t0 = Instant::now();
    loop {
        if sup.status().phase == ServerPhase::Ready {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(3), "start abandoned");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    sup.complete(&sample(), Duration::from_secs(5), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(h.launches().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_idle_owner_never_stops_the_server_under_a_take() {
    let (_h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 0 }]);
    {
        lock(&sup.config).idle_timeout = Duration::from_millis(100);
    }
    sup.ensure_ready().await.unwrap();
    sup.take_started();
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(!sup.idle_tick().await, "a take is recording");
    sup.take_ended();
    assert!(
        !sup.idle_tick().await,
        "the idle timeout counts from the take's end"
    );
    // Someone waiting for the server holds it too.
    tokio::time::sleep(Duration::from_millis(150)).await;
    sup.waiting.fetch_add(1, Ordering::AcqRel);
    assert!(!sup.idle_tick().await, "a caller is waiting");
    assert!(!sup.stop_if_idle().await, "a caller is waiting");
    sup.waiting.fetch_sub(1, Ordering::AcqRel);
    assert!(sup.idle_tick().await);
    assert!(!sup.is_running().await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_take_that_never_ends_stops_holding_the_server() {
    let (_h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 0 }]);
    {
        lock(&sup.config).idle_timeout = Duration::from_millis(50);
    }
    sup.ensure_ready().await.unwrap();
    sup.take_started();
    *lock(&sup.take_since) = Some(Instant::now() - TAKE_HOLD_MAX);
    *lock(&sup.last_used) = Instant::now() - Duration::from_secs(1);
    assert!(sup.idle_tick().await);
}

/// Note 5: a slow first exec (syspolicyd) timed the GPU starts out and the
/// app fell back to the CPU; the GPU gets one more try at the next start,
/// once per launch.
#[tokio::test(flavor = "multi_thread")]
async fn a_timeout_fallback_tries_the_gpu_again_once() {
    let (h, sup, _d) = setup(&[
        Behavior::NeverHealthy,
        Behavior::NeverHealthy,
        Behavior::NeverHealthy,
        Behavior::Healthy { load_ms: 0 },
    ]);
    assert!(sup.ensure_ready().await.is_err());
    assert_eq!(sup.status().device, "cpu");
    sup.inner.lock().await.backoff_until = None;
    sup.ensure_ready().await.unwrap();
    let l = h.launches();
    assert_eq!(l.len(), 4);
    assert!(!has_ngl0(&l[3]), "back on the GPU");
    assert_eq!(sup.status().device, "gpu");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_gpu_retry_is_used_once_per_launch() {
    let (h, sup, _d) = setup(&[
        Behavior::NeverHealthy,
        Behavior::NeverHealthy,
        Behavior::NeverHealthy,
        // The retry: the GPU times out twice again, then the CPU starts.
        Behavior::NeverHealthy,
        Behavior::NeverHealthy,
        Behavior::Healthy { load_ms: 0 },
    ]);
    assert!(sup.ensure_ready().await.is_err());
    sup.inner.lock().await.backoff_until = None;
    sup.ensure_ready().await.unwrap();
    let l = h.launches();
    assert_eq!(l.len(), 6);
    assert!(!has_ngl0(&l[3]) && !has_ngl0(&l[4]) && has_ngl0(&l[5]));
    // No third GPU try in this launch.
    sup.stop().await;
    sup.ensure_ready().await.unwrap();
    assert!(has_ngl0(h.launches().last().unwrap()));
    assert_eq!(sup.status().device, "cpu");
}

/// A take whose server is still starting (cold, slow) gets
/// `deadline::START_BUDGET`, then pastes the deterministic text; the start
/// carries on for the next take.
#[tokio::test(flavor = "multi_thread")]
async fn a_cold_start_inside_a_take_falls_back_within_the_start_budget() {
    use crate::fork::llm::deadline::START_BUDGET;
    use crate::fork::llm::post_process::{clean, CleanupOutcome, LocalEngine, Request};
    use crate::fork::settings::CleanupLevel;
    let (h, sup, _d) = setup(&[Behavior::Healthy { load_ms: 4_000 }]);
    {
        lock(&sup.config).health_timeout = Duration::from_secs(10);
    }
    let text = "um so I was thinking we could uh meet on friday no wait thursday";
    let req = Request {
        text,
        level: CleanupLevel::Light,
        language: None,
        hint: None,
        terms: &[],
        keep: &[],
        style: None,
        context: &[],
    };
    let t0 = Instant::now();
    let cleaned = clean(
        &LocalEngine {
            supervisor: &sup,
            pill: false,
        },
        &req,
    )
    .await;
    let took = t0.elapsed();
    assert_eq!(cleaned.outcome, CleanupOutcome::Timeout);
    assert!(
        took < START_BUDGET + Duration::from_millis(500),
        "waited {took:?}"
    );
    assert_eq!(h.shared().chats, 0);
    assert_eq!(sup.status().phase, ServerPhase::Starting, "still starting");
    assert_eq!(h.launches().len(), 1);
}
