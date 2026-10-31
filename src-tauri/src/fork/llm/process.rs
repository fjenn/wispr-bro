// SPDX-License-Identifier: GPL-3.0-or-later
//! The `llama-server` child process (R5 §4.5, W-01).
//!
//! Port of the process handling in TypeWhisper's `LlamaCppServer.swift`
//! (GPL-3.0-or-later; see docs/fork/PORTS.md): spawn line, scrubbed env,
//! SIGTERM then SIGKILL, a stale-PID kill at launch, and a watchdog so the
//! child never outlives the app. TypeWhisper downloads its runtime at first
//! use; we never do (the binary is bundled, R5 §4.5).
//!
//! Spawn line (RS-06, b11624):
//! `-m <gguf> --host <sock> -c 4096 -np 1 --cache-ram 256 --reasoning off
//!  --jinja --no-ui --no-slots --offline [-ngl 0]`
//! (`--no-ui` replaces the deprecated `--no-webui`; `--no-slots` hides the
//! slot monitor, which would expose the prompt being processed).
//! with `LLAMA_API_KEY` in an environment that holds nothing else from ours
//! (no `LLAMA_ARG_*`, `HF_*`, proxy or locale variables reach it).

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use super::transport::Transport;

/// Name of the bundled binary (Tauri `externalBin` strips the target triple).
pub const BINARY_NAME: &str = "llama-server";
/// Name of the PID file in the LLM dir.
pub const PID_FILE: &str = "llama-server.pid";
/// Context size: one dictation plus the prompt (R5 §4.5).
pub const CONTEXT_TOKENS: u32 = 4096;
/// Prompt-cache RAM in MiB (R5 §4.5).
pub const CACHE_RAM_MIB: u32 = 256;
/// Model name the server reports; requests address it by this alias.
pub const MODEL_ALIAS: &str = "local";
/// Grace period between SIGTERM and SIGKILL.
pub const TERM_GRACE: Duration = Duration::from_secs(2);

/// GPU (Metal) or CPU only (`-ngl 0`, the fallback after a Metal failure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    Gpu,
    Cpu,
}

/// Everything needed to start one server.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub binary: PathBuf,
    pub model: PathBuf,
    pub transport: Transport,
    pub device: Device,
    /// `<app data>/llm`: where the PID file goes.
    pub state_dir: PathBuf,
    /// `HOME` for the child (llama.cpp reads nothing from it with
    /// `--offline`, but some system frameworks expect it set).
    pub home: Option<PathBuf>,
}

impl SpawnSpec {
    /// Command-line arguments, in order.
    pub fn args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec![
            "-m".into(),
            self.model.clone().into_os_string(),
            "--alias".into(),
            MODEL_ALIAS.into(),
        ];
        args.extend(
            self.transport
                .endpoint
                .server_args()
                .into_iter()
                .map(OsString::from),
        );
        for a in [
            "-c",
            &CONTEXT_TOKENS.to_string(),
            "-np",
            "1",
            "--threads",
            &recommended_threads().to_string(),
            "--cache-ram",
            &CACHE_RAM_MIB.to_string(),
            "--reasoning",
            "off",
            "--jinja",
            "--no-ui",
            "--no-slots",
            "--offline",
        ] {
            args.push(a.into());
        }
        if self.device == Device::Cpu {
            args.extend(["-ngl".into(), "0".into()]);
        }
        args
    }

    /// The child's complete environment.
    pub fn env(&self) -> Vec<(OsString, OsString)> {
        let mut env: Vec<(OsString, OsString)> = vec![
            ("PATH".into(), "/usr/bin:/bin".into()),
            ("LLAMA_API_KEY".into(), self.transport.api_key().into()),
        ];
        if let Some(home) = &self.home {
            env.push(("HOME".into(), home.clone().into_os_string()));
        }
        env
    }

    /// The `Command` (env cleared, stdin closed, output piped).
    pub fn command(&self) -> Command {
        let mut cmd = Command::new(&self.binary);
        cmd.args(self.args())
            .env_clear()
            .envs(self.env())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        if let Some(dir) = self.model.parent() {
            cmd.current_dir(dir);
        }
        cmd
    }
}

/// CPU threads: all but two cores, at most 8 (TypeWhisper's rule). Matters
/// only for the CPU fallback; Metal does the work otherwise.
pub fn recommended_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .saturating_sub(2)
        .clamp(1, 8)
}

/// Last lines of the server's stderr, for error messages. Startup output
/// holds paths and model metadata, never user text (`--no-slots`, default
/// verbosity); it is logged only when a start fails.
#[derive(Debug, Default, Clone)]
pub struct OutputTail {
    text: String,
}

impl OutputTail {
    /// Bytes kept (TypeWhisper keeps 12,000 characters).
    pub const LIMIT: usize = 12_000;

    pub fn push_line(&mut self, line: &str) {
        self.text.push_str(line);
        self.text.push('\n');
        if self.text.len() > Self::LIMIT {
            let mut cut = self.text.len() - Self::LIMIT;
            while !self.text.is_char_boundary(cut) {
                cut += 1;
            }
            self.text.drain(..cut);
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The last line that names an error (TypeWhisper `lastErrorLine`).
    pub fn last_error_line(&self) -> Option<&str> {
        self.text.lines().rev().find(|l| {
            let l = l.to_ascii_lowercase();
            l.contains("error") || l.contains("failed") || l.contains("exiting")
        })
    }

    /// `true` when the server could not bind its address (port race).
    pub fn is_bind_failure(&self) -> bool {
        let t = self.text.to_ascii_lowercase();
        t.contains("address already in use")
            || t.contains("couldn't bind")
            || t.contains("failed to bind")
    }
}

/// Where the sidecar binary is: next to the app's executable (Tauri copies
/// `externalBin` there in dev and into `Contents/MacOS` in a bundle).
/// `WISPRBRO_LLAMA_SERVER` overrides it for tests and the RS-06 bench only in
/// debug builds.
pub fn sidecar_path() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        if let Some(p) = std::env::var_os("WISPRBRO_LLAMA_SERVER") {
            return Some(PathBuf::from(p));
        }
    }
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.join(BINARY_NAME);
    candidate.is_file().then_some(candidate)
}

/// Sends SIGTERM, waits up to `grace`, then SIGKILL. Always reaps the child.
pub fn terminate(child: &mut Child, grace: Duration) -> io::Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    let pid = child_pid(child)?;
    // SAFETY: plain kill(2) on our own child, which has not been reaped yet,
    // so the PID cannot have been reused.
    unsafe { libc::kill(pid, libc::SIGTERM) };
    let until = Instant::now() + grace;
    while Instant::now() < until {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.kill()?;
    child.wait()?;
    Ok(())
}

fn child_pid(child: &Child) -> io::Result<libc::pid_t> {
    libc::pid_t::try_from(child.id()).map_err(|_| io::Error::other("pid out of range"))
}

// ---------------------------------------------------------------------------
// PID file and stale-PID kill

/// Records the running server's PID.
pub fn write_pid_file(dir: &Path, pid: u32) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{PID_FILE}.tmp"));
    std::fs::write(&tmp, pid.to_string())?;
    std::fs::rename(tmp, dir.join(PID_FILE))
}

/// Removes the PID file (normal stop).
pub fn remove_pid_file(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(PID_FILE));
}

/// At launch: if the PID file names a live `llama-server` left by a crashed
/// run, kill it. Returns the PID that was killed. Never kills a process with
/// another executable name (PID reuse).
pub fn kill_stale(dir: &Path) -> Option<u32> {
    let path = dir.join(PID_FILE);
    let pid: u32 = std::fs::read_to_string(&path).ok()?.trim().parse().ok()?;
    let _ = std::fs::remove_file(&path);
    let pid_t = libc::pid_t::try_from(pid).ok().filter(|p| *p > 1)?;
    let name = process_path(pid_t)?;
    if name.file_name().and_then(|n| n.to_str()) != Some(BINARY_NAME) {
        return None;
    }
    // SAFETY: kill(2); the executable check above guards against PID reuse.
    let rc = unsafe { libc::kill(pid_t, libc::SIGKILL) };
    (rc == 0).then_some(pid)
}

/// Executable path of a process (`proc_pidpath`).
#[cfg(target_os = "macos")]
pub fn process_path(pid: libc::pid_t) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: the buffer is PROC_PIDPATHINFO_MAXSIZE bytes as the API needs.
    let n = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    if n <= 0 {
        return None;
    }
    buf.truncate(n as usize);
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(&buf)))
}

#[cfg(not(target_os = "macos"))]
pub fn process_path(_pid: libc::pid_t) -> Option<PathBuf> {
    None
}

// ---------------------------------------------------------------------------
// Parent-death watchdog (kqueue NOTE_EXIT)

/// Forks a tiny watchdog that waits on two kqueue `NOTE_EXIT` events: if
/// `parent` exits first it SIGKILLs `child`; if `child` exits first it just
/// leaves. So the server dies with the app even when the app is killed with
/// SIGKILL or crashes (macOS has no `PR_SET_PDEATHSIG`).
///
/// Returns the watchdog's PID; the caller reaps it ([`reap_in_background`]).
///
/// The forked process runs only raw system calls (`kqueue`, `kevent`,
/// `kill`, `close`, `_exit`), which is what POSIX allows after `fork` in a
/// multithreaded process: no allocation, no locks, no Objective-C.
#[cfg(target_os = "macos")]
pub fn spawn_watchdog(parent: libc::pid_t, child: libc::pid_t) -> io::Result<libc::pid_t> {
    // Everything the child needs is prepared before fork.
    // SAFETY: zeroed kevent structs are valid; fields are set below.
    let mut changes: [libc::kevent; 2] = unsafe { std::mem::zeroed() };
    for (ev, pid) in changes.iter_mut().zip([parent, child]) {
        ev.ident = pid as libc::uintptr_t;
        ev.filter = libc::EVFILT_PROC;
        ev.flags = libc::EV_ADD | libc::EV_ONESHOT;
        ev.fflags = libc::NOTE_EXIT;
    }
    // SAFETY: fork(2). The child branch calls only async-signal-safe system
    // calls and ends in _exit, never returning into Rust code.
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(io::Error::last_os_error());
    }
    if pid > 0 {
        return Ok(pid);
    }
    // --- watchdog process ---
    // SAFETY: raw system calls only, see above.
    unsafe {
        // Drop inherited descriptors (sockets, pipes, files of the app).
        for fd in 3..1024 {
            libc::close(fd);
        }
        let kq = libc::kqueue();
        if kq < 0 {
            libc::_exit(1);
        }
        // Register both and block until the first one fires. A pid that is
        // already gone fails with ESRCH and comes back at once as an
        // EV_ERROR event for that pid, which counts as "exited".
        let mut events: [libc::kevent; 2] = std::mem::zeroed();
        let mut n = libc::kevent(
            kq,
            changes.as_ptr(),
            2,
            events.as_mut_ptr(),
            2,
            std::ptr::null(),
        );
        while n < 0 && *libc::__error() == libc::EINTR {
            n = libc::kevent(
                kq,
                std::ptr::null(),
                0,
                events.as_mut_ptr(),
                2,
                std::ptr::null(),
            );
        }
        // kevent failed: leave the server alone (the stale-PID kill at the
        // next launch is the backstop).
        let mut parent_gone = false;
        if n > 0 {
            for ev in &events[..n as usize] {
                if ev.ident == parent as libc::uintptr_t {
                    parent_gone = true;
                }
            }
        }
        // Otherwise the child exited first (or was never there): just leave.
        if parent_gone {
            libc::kill(child, libc::SIGKILL);
        }
        libc::_exit(0);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn spawn_watchdog(_parent: libc::pid_t, _child: libc::pid_t) -> io::Result<libc::pid_t> {
    Err(io::Error::other("watchdog is macOS only"))
}

/// Reaps a forked helper on a background thread (no zombie).
pub fn reap_in_background(pid: libc::pid_t) {
    std::thread::Builder::new()
        .name("wb-llm-reap".into())
        .spawn(move || {
            let mut status = 0;
            // SAFETY: waitpid on our own child.
            unsafe { libc::waitpid(pid, &mut status, 0) };
        })
        .ok();
}

/// `true` while `pid` exists (kill 0).
pub fn is_alive(pid: libc::pid_t) -> bool {
    // SAFETY: signal 0 only checks existence and permission.
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::llm::transport::{Endpoint, Transport};

    fn spec(device: Device) -> SpawnSpec {
        SpawnSpec {
            binary: "/opt/x/llama-server".into(),
            model: "/m/model.gguf".into(),
            transport: Transport::new(Endpoint::Unix("/d/llm.sock".into()), "KEY".into()),
            device,
            state_dir: "/d".into(),
            home: Some("/Users/a".into()),
        }
    }

    fn strs(v: Vec<OsString>) -> Vec<String> {
        v.into_iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn spawn_line_matches_r5_and_rs06() {
        let threads = recommended_threads().to_string();
        assert_eq!(
            strs(spec(Device::Gpu).args()),
            [
                "-m",
                "/m/model.gguf",
                "--alias",
                "local",
                "--host",
                "/d/llm.sock",
                "-c",
                "4096",
                "-np",
                "1",
                "--threads",
                threads.as_str(),
                "--cache-ram",
                "256",
                "--reasoning",
                "off",
                "--jinja",
                "--no-ui",
                "--no-slots",
                "--offline"
            ]
        );
        let cpu = strs(spec(Device::Cpu).args());
        assert_eq!(&cpu[cpu.len() - 2..], ["-ngl", "0"]);
        // Never a Hugging Face download from the server itself.
        for a in strs(spec(Device::Gpu).args()) {
            assert!(!a.starts_with("-hf") && !a.starts_with("--hf"), "{a}");
        }
    }

    /// R5 §4.5: the bundle ships the sidecar `ci/llama-server/build.sh`
    /// builds (static, Metal embedded, macOS 13.0, no TLS, no UI download,
    /// M1 CPU baseline), under the name `sidecar_path` looks for.
    #[test]
    fn the_overlay_bundles_the_sidecar_build_sh_builds() {
        let overlay: serde_json::Value =
            serde_json::from_str(include_str!("../../../tauri.fork.conf.json")).unwrap();
        assert_eq!(
            overlay["bundle"]["externalBin"],
            serde_json::json!([format!("target/wb-sidecar/{BINARY_NAME}")])
        );
        let script = include_str!("../../../../ci/llama-server/build.sh");
        for needle in [
            r#"OUT_DIR="${ROOT}/src-tauri/target/wb-sidecar""#,
            r#"OUT="${OUT_DIR}/llama-server-${TRIPLE}""#,
            r#"DEPLOYMENT_TARGET="13.0""#,
            r#"TAG="b11624""#,
            "-DBUILD_SHARED_LIBS=OFF",
            "-DGGML_METAL_EMBED_LIBRARY=ON",
            "-DGGML_NATIVE=OFF",
            "-DGGML_BLAS=OFF",
            "-DLLAMA_OPENSSL=OFF",
            "-DLLAMA_BUILD_UI=OFF",
            "-DLLAMA_USE_PREBUILT_UI=OFF",
            "--options runtime",
        ] {
            assert!(script.contains(needle), "build.sh lost {needle}");
        }
        let package: serde_json::Value =
            serde_json::from_str(include_str!("../../../../package.json")).unwrap();
        for s in ["wb:dev", "wb:build"] {
            assert!(
                package["scripts"][s]
                    .as_str()
                    .unwrap()
                    .starts_with("ci/llama-server/build.sh --if-missing && "),
                "{s}"
            );
        }
    }

    /// `ci/inert-hosts.txt` proof for the hosts inside the sidecar binary
    /// (its Hugging Face, Docker-registry and model-catalog download code):
    /// the real binary refuses any HTTPS request because it is built without
    /// TLS, and the app never passes a download flag (`spawn_line_matches_r5_and_rs06`).
    /// Needs `WB_LLAMA_SERVER`; it makes no connection (the refusal happens
    /// before any socket is opened).
    #[test]
    #[ignore = "needs WB_LLAMA_SERVER (the built sidecar)"]
    fn real_sidecar_cannot_make_https_requests() {
        let Some(bin) = std::env::var_os("WB_LLAMA_SERVER") else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let out = Command::new(bin)
            .args(["-hf", "org/repo-GGUF", "--host"])
            .arg(dir.path().join("x.sock"))
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", dir.path())
            .output()
            .unwrap();
        let log = String::from_utf8_lossy(&out.stderr);
        assert!(log.contains("HTTPS is not supported"), "{log}");
        assert!(!out.status.success());
    }

    #[test]
    fn output_tail_names_the_last_error_and_bind_failures() {
        let mut t = OutputTail::default();
        for l in [
            "load_model: loading model '/x.gguf'",
            "llama_model_load: error loading model: unknown model architecture: 'x'",
            "srv load_model: failed to load model",
            "main: exiting due to model loading error",
        ] {
            t.push_line(l);
        }
        assert_eq!(
            t.last_error_line(),
            Some("main: exiting due to model loading error")
        );
        assert!(!t.is_bind_failure());
        t.push_line("couldn't bind HTTP server socket, hostname: 127.0.0.1, port: 5");
        assert!(t.is_bind_failure());
        let mut long = OutputTail::default();
        for _ in 0..2000 {
            long.push_line("ééééé ok");
        }
        assert!(long.as_str().len() <= OutputTail::LIMIT);
        assert_eq!(long.last_error_line(), None);
        assert!((1..=8).contains(&recommended_threads()));
    }

    #[test]
    fn env_is_scrubbed() {
        std::env::set_var("LLAMA_ARG_HOST", "0.0.0.0");
        std::env::set_var("HF_TOKEN", "secret");
        let out = Command::new("/usr/bin/env")
            .env_clear()
            .envs(spec(Device::Gpu).env())
            .output()
            .unwrap();
        std::env::remove_var("LLAMA_ARG_HOST");
        std::env::remove_var("HF_TOKEN");
        let mut lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_owned)
            .collect();
        lines.sort();
        assert_eq!(
            lines,
            ["HOME=/Users/a", "LLAMA_API_KEY=KEY", "PATH=/usr/bin:/bin"]
        );
        // The real command clears the env too.
        let cmd = spec(Device::Gpu).command();
        let envs: Vec<_> = cmd.get_envs().collect();
        assert_eq!(envs.len(), 3);
    }

    #[test]
    fn terminate_escalates_to_sigkill() {
        // A shell that ignores SIGTERM.
        let mut child = Command::new("/bin/sh")
            .args(["-c", "trap '' TERM; while :; do sleep 0.05; done"])
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let t0 = Instant::now();
        terminate(&mut child, Duration::from_millis(300)).unwrap();
        assert!(child.try_wait().unwrap().is_some());
        let took = t0.elapsed();
        assert!(took >= Duration::from_millis(300), "{took:?}");
        assert!(took < Duration::from_secs(3), "{took:?}");

        // A well-behaved child stops on SIGTERM without waiting the grace.
        let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let t0 = Instant::now();
        terminate(&mut child, Duration::from_secs(5)).unwrap();
        assert!(t0.elapsed() < Duration::from_secs(1));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn stale_pid_is_killed_only_if_it_is_llama_server() {
        let dir = tempfile::tempdir().unwrap();
        // A copy of `sleep` named llama-server stands in for a leftover.
        let fake = dir.path().join(BINARY_NAME);
        std::fs::copy("/bin/sleep", &fake).unwrap();
        let mut stale = Command::new(&fake).arg("30").spawn().unwrap();
        write_pid_file(dir.path(), stale.id()).unwrap();
        assert_eq!(kill_stale(dir.path()), Some(stale.id()));
        assert!(stale.wait().unwrap().code().is_none(), "killed by a signal");
        assert!(!dir.path().join(PID_FILE).exists());

        // A reused PID with another name survives.
        let mut other = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        write_pid_file(dir.path(), other.id()).unwrap();
        assert_eq!(kill_stale(dir.path()), None);
        assert!(other.try_wait().unwrap().is_none());
        other.kill().unwrap();
        other.wait().unwrap();

        // Garbage and missing files are ignored.
        std::fs::write(dir.path().join(PID_FILE), "nope").unwrap();
        assert_eq!(kill_stale(dir.path()), None);
        assert_eq!(kill_stale(dir.path()), None);
    }

    /// The watchdog kills the server when its "parent" dies, and leaves
    /// quietly when the server exits first.
    #[cfg(target_os = "macos")]
    #[test]
    fn watchdog_kills_the_child_when_the_parent_dies() {
        let mut parent = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let dog = spawn_watchdog(parent.id() as i32, child.id() as i32).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert!(child.try_wait().unwrap().is_none());
        let t0 = Instant::now();
        parent.kill().unwrap();
        parent.wait().unwrap();
        let status = child.wait().unwrap();
        let took = t0.elapsed();
        assert!(status.code().is_none(), "child killed by a signal");
        assert!(took < Duration::from_millis(500), "{took:?}");
        let mut st = 0;
        // SAFETY: waitpid on our own forked watchdog.
        assert_eq!(unsafe { libc::waitpid(dog, &mut st, 0) }, dog);
        assert!(libc::WIFEXITED(st) && libc::WEXITSTATUS(st) == 0);

        // Child exits first: the watchdog leaves and the parent lives on.
        let mut parent = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let mut child = Command::new("/bin/sleep").arg("0.1").spawn().unwrap();
        let dog = spawn_watchdog(parent.id() as i32, child.id() as i32).unwrap();
        child.wait().unwrap();
        let mut st = 0;
        // SAFETY: as above.
        assert_eq!(unsafe { libc::waitpid(dog, &mut st, 0) }, dog);
        assert!(parent.try_wait().unwrap().is_none());
        parent.kill().unwrap();
        parent.wait().unwrap();
    }
}
