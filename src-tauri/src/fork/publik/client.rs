// SPDX-License-Identifier: GPL-3.0-or-later
//! publik's app protocol: install, wallet, telemetry and the self test.
//!
//! Wispr Bro's own implementation of what publik's Rust module
//! (publikhq.com/sdk/publik.rs.md) does, with the same endpoints, files,
//! request bodies, screens and words, so publik sees the same app behaviour
//! as from its module. Like the module it talks to publik through the system
//! `curl` (macOS ships it), keeps the key off the command line (curl reads
//! the header from stdin) and never blocks the app on telemetry.
//!
//! Differences from the module, all deliberate:
//! - the publik app token comes from the build environment
//!   (`PUBLIK_APP_TOKEN` at compile time), not a committed file, so the
//!   repository never holds it (docs/fork/EDITIONS.md);
//! - files are created with mode 0600 instead of chmod'ed after writing.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{json, Map, Value};

use super::{APP_SLUG, APP_VERSION};

/// publik's site. `PUBLIK_ORIGIN` overrides it (tests), as in the module.
pub const DEFAULT_ORIGIN: &str = "https://publikhq.com";
pub const INSTALLS_PATH: &str = "/api/v1/installs";
pub const WALLET_PATH: &str = "/api/v1/wallet";
pub const TELEMETRY_PATH: &str = "/api/v1/telemetry";

pub const UNREACHABLE: &str = "publik is not reachable. Check your connection.";
pub const NO_TOKEN: &str = "This build has no publik app token. Ask the developer for a new build.";

/// The publik app token baked in by the publik release build.
const BUILD_TOKEN: Option<&str> = option_env!("PUBLIK_APP_TOKEN");

pub const OS: &str = if cfg!(target_os = "macos") {
    "macos"
} else if cfg!(target_os = "windows") {
    "windows"
} else {
    "linux"
};

pub fn arch() -> Option<&'static str> {
    if cfg!(target_arch = "aarch64") {
        Some("arm64")
    } else if cfg!(target_arch = "x86_64") {
        Some("x64")
    } else {
        None
    }
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub fn origin() -> String {
    env_nonempty("PUBLIK_ORIGIN")
        .map(|o| o.trim_end_matches('/').to_string())
        .unwrap_or_else(|| DEFAULT_ORIGIN.to_string())
}

/// `PUBLIK_APP_TOKEN` at run time, else the one built in; only `pat_` tokens.
pub fn app_token() -> Option<String> {
    env_nonempty("PUBLIK_APP_TOKEN")
        .or_else(|| BUILD_TOKEN.map(|t| t.trim().to_string()))
        .filter(|t| t.starts_with("pat_"))
}

/// Where publik is and who this computer is to it: read from the process
/// environment in the app, built by hand in tests.
#[derive(Clone, Debug)]
pub struct Env {
    pub origin: String,
    /// publik's shared folder (`publik_dir`).
    pub dir: Option<PathBuf>,
    pub token: Option<String>,
    /// `PUBLIK_API_KEY`: a key from outside, never wiped or provisioned.
    pub api_key: Option<String>,
    /// How often and how many times the gate looks for a sign-in (3 s, 200
    /// times: 10 minutes).
    pub poll_every: std::time::Duration,
    pub polls: u32,
}

impl Env {
    pub fn from_process() -> Self {
        Env {
            origin: origin(),
            dir: publik_dir(),
            token: app_token(),
            api_key: env_nonempty("PUBLIK_API_KEY"),
            poll_every: std::time::Duration::from_secs(3),
            polls: 200,
        }
    }

    pub fn credential_path(&self) -> Option<PathBuf> {
        self.dir
            .as_ref()
            .map(|d| d.join("apps").join(format!("{APP_SLUG}.json")))
    }

    /// The stored credential, or `{}`.
    pub fn read_credential(&self) -> Value {
        self.credential_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}))
    }

    pub fn write_credential(&self, value: &Value) {
        if let Some(path) = self.credential_path() {
            let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".into());
            if let Err(e) = write_private(&path, &text, false) {
                log::warn!("publik: could not save the credential: {e}");
            }
        }
    }

    /// The key to use: `PUBLIK_API_KEY`, else a stored `pk_` key.
    pub fn key(&self) -> Option<String> {
        self.api_key.clone().or_else(|| {
            self.read_credential()
                .get("key")
                .and_then(Value::as_str)
                .filter(|k| k.starts_with("pk_"))
                .map(str::to_string)
        })
    }

    pub fn choice(&self) -> Option<String> {
        self.read_credential()
            .get("choice")
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    /// Stores the AI choice (`publik`, `own_key`, or none to ask again).
    pub fn set_choice(&self, choice: Option<&str>) {
        if self.credential_path().is_some_and(|p| p.exists()) {
            let mut cred = self.read_credential();
            cred["choice"] = choice.map_or(Value::Null, |c| Value::String(c.to_string()));
            self.write_credential(&cred);
        }
    }

    fn base_url(&self) -> String {
        format!("{}/api/v1", self.origin)
    }
}

// ---------------------------------------------------------------------------
// Files

/// publik's folder, shared by every publik app on this computer.
pub fn publik_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home.map(|h| h.join("Library/Application Support/publik"));
    }
    if cfg!(target_os = "windows") {
        return std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| home.map(|h| h.join("AppData").join("Local")))
            .map(|d| d.join("publik"));
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| home.map(|h| h.join(".config")))
        .map(|d| d.join("publik"))
}

/// Writes a file readable by this user only.
fn write_private(path: &std::path::Path, text: &str, create_new: bool) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if create_new {
        options.create_new(true);
    } else {
        options.create(true).truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(text.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// IDs and time

/// A random UUID v4, lowercase and hyphenated.
pub fn uuid_v4() -> String {
    let mut b = [0u8; 16];
    let filled = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut b))
        .is_ok();
    if !filled {
        // No /dev/urandom (Windows): the clock and the process id, hashed.
        use std::hash::{BuildHasher, Hasher};
        for chunk in b.chunks_mut(8) {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u128(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0),
            );
            h.write_u32(std::process::id());
            chunk.copy_from_slice(&h.finish().to_le_bytes());
        }
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

pub fn is_uuid_v4(s: &str) -> bool {
    let b = s.as_bytes();
    s.len() == 36
        && [8, 13, 18, 23].iter().all(|&i| b[i] == b'-')
        && s.chars()
            .enumerate()
            .all(|(i, c)| [8, 13, 18, 23].contains(&i) || c.is_ascii_hexdigit())
        && b[14] == b'4'
        && matches!(b[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B')
}

pub fn utc_now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn device_name() -> String {
    Command::new("hostname")
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .chars()
                .take(120)
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// HTTP through curl

pub struct Reply {
    pub code: u16,
    pub body: Value,
}

/// Runs curl with `args`, `stdin` on its standard input; the status code is
/// read from the last line (`-w`). A transport failure is `Err`.
fn curl(args: &[String], stdin: &str) -> Result<Reply, String> {
    let mut child = Command::new("curl")
        // A local publik (tests, `PUBLIK_ORIGIN`) is never reached through a
        // proxy; every other address is unaffected.
        .args([
            "-sS",
            "--noproxy",
            "localhost,127.0.0.1",
            "-w",
            "\n%{http_code}",
        ])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| UNREACHABLE.to_string())?;
    if let Some(mut input) = child.stdin.take() {
        let _ = input.write_all(stdin.as_bytes());
    }
    let out = child
        .wait_with_output()
        .map_err(|_| UNREACHABLE.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", text.as_ref()));
    let code: u16 = code.trim().parse().unwrap_or(0);
    if !out.status.success() || code == 0 {
        return Err(UNREACHABLE.to_string());
    }
    Ok(Reply {
        code,
        body: serde_json::from_str(body).unwrap_or(Value::Null),
    })
}

/// curl config with the bearer header, read from stdin (`-K -`).
fn auth_config(key: &str) -> String {
    format!("header = \"Authorization: Bearer {key}\"\n")
}

fn error_message(body: &Value) -> Option<String> {
    body.pointer("/error/message")
        .and_then(Value::as_str)
        .map(str::to_string)
}

// ---------------------------------------------------------------------------
// Install, wallet

/// The body of `POST /api/v1/installs`.
pub fn install_body(token: &str, install_id: &str, device: &str) -> Value {
    let mut body = json!({
        "app_token": token,
        "app_slug": APP_SLUG,
        "app_version": APP_VERSION,
        "os": OS,
        "device_name": device,
        "install_id": install_id,
        "disclosure_version": 1,
        "dialects": ["chat_completions"],
    });
    if let Some(a) = arch() {
        body["arch"] = json!(a);
    }
    body
}

/// Sets this computer up with publik and stores the key.
pub fn provision(env: &Env) -> Result<String, String> {
    let token = env.token.clone().ok_or_else(|| NO_TOKEN.to_string())?;
    let prior_cred = env.read_credential();
    let mut prior = prior_cred
        .get("install_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_string);
    let device = device_name();
    let url = format!("{}{INSTALLS_PATH}", env.origin);
    let mut last_code = 0;
    for _ in 0..2 {
        let install_id = prior.take().unwrap_or_else(uuid_v4);
        let body = install_body(&token, &install_id, &device);
        let reply = curl(
            &[
                "-m".into(),
                "20".into(),
                "-X".into(),
                "POST".into(),
                "-H".into(),
                "content-type: application/json".into(),
                "--data-binary".into(),
                "@-".into(),
                url.clone(),
            ],
            &body.to_string(),
        )?;
        last_code = reply.code;
        let key = reply
            .body
            .get("key")
            .and_then(Value::as_str)
            .filter(|k| !k.is_empty());
        let revoked =
            reply.body.pointer("/error/type").and_then(Value::as_str) == Some("install_revoked");
        if revoked {
            continue;
        }
        match (reply.code, key) {
            (200 | 201, Some(key)) => {
                let mut cred = Map::new();
                cred.insert("key".into(), json!(key));
                cred.insert(
                    "base_url".into(),
                    reply
                        .body
                        .get("base_url")
                        .cloned()
                        .unwrap_or_else(|| json!(env.base_url())),
                );
                cred.insert("install_id".into(), json!(install_id));
                cred.insert(
                    "claim_url".into(),
                    reply.body.get("claim_url").cloned().unwrap_or(Value::Null),
                );
                cred.insert(
                    "choice".into(),
                    prior_cred.get("choice").cloned().unwrap_or(Value::Null),
                );
                env.write_credential(&Value::Object(cred));
                return Ok(key.to_string());
            }
            (200, None) => continue,
            (code, _) => {
                return Err(error_message(&reply.body)
                    .unwrap_or_else(|| format!("publik could not set up this computer ({code}).")))
            }
        }
    }
    Err(format!(
        "publik could not set up this computer ({last_code})."
    ))
}

/// What `GET /api/v1/wallet` answered.
#[derive(Debug)]
pub enum Wallet {
    /// 200 with a `claim_state`.
    Ok(Value),
    /// Anything else: the status and publik's error message, if any.
    Refused(u16, Option<String>),
}

/// `GET /api/v1/wallet`. A transport failure is `Err`.
pub fn read_wallet(env: &Env, key: &str) -> Result<Wallet, String> {
    let url = format!("{}{WALLET_PATH}", env.origin);
    let reply = curl(
        &["-m".into(), "15".into(), "-K".into(), "-".into(), url],
        &auth_config(key),
    )?;
    let has_state = reply.body.get("claim_state").is_some_and(Value::is_string);
    if reply.code == 200 && has_state {
        Ok(Wallet::Ok(reply.body))
    } else {
        Ok(Wallet::Refused(reply.code, error_message(&reply.body)))
    }
}

pub fn is_claimed(wallet: &Value) -> bool {
    wallet.get("claim_state").and_then(Value::as_str) == Some("claimed")
}

pub fn has_plan(wallet: &Value) -> bool {
    let kind = wallet
        .pointer("/plan/kind")
        .and_then(Value::as_str)
        .unwrap_or("none");
    kind != "none"
        || wallet
            .get("balance_micros")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            > 0
}

// ---------------------------------------------------------------------------
// Self test

pub fn self_test_line(env: &Env) -> String {
    let result = env
        .key()
        .map(Ok)
        .unwrap_or_else(|| provision(env))
        .and_then(|key| match read_wallet(env, &key)? {
            Wallet::Ok(_) => Ok(()),
            Wallet::Refused(_, Some(message)) => Err(message),
            Wallet::Refused(code, None) => Err(format!("GET /wallet answered {code}")),
        });
    match result {
        Ok(()) => format!("publik self-test: ok {APP_SLUG} {APP_VERSION}"),
        Err(e) => format!("publik self-test: failed: {e}"),
    }
}

/// With `PUBLIK_SELF_TEST` set (any value): print the result and exit 0.
/// Called first thing at launch, before any window.
pub fn self_test_if_requested() {
    if std::env::var_os("PUBLIK_SELF_TEST").is_some() {
        println!("{}", self_test_line(&Env::from_process()));
        std::process::exit(0);
    }
}

// ---------------------------------------------------------------------------
// Telemetry

fn opted_out_with(env: impl Fn(&str) -> Option<String>, consent: Option<Value>) -> bool {
    let dnt = env("DO_NOT_TRACK")
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_default();
    if !dnt.is_empty() && dnt != "0" && dnt != "false" {
        return true;
    }
    let pt = env("PUBLIK_TELEMETRY")
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_default();
    if matches!(pt.as_str(), "0" | "false" | "off") {
        return true;
    }
    consent.and_then(|c| c.get("usage").and_then(Value::as_bool)) == Some(false)
}

pub fn opted_out() -> bool {
    let consent = publik_dir()
        .and_then(|d| std::fs::read_to_string(d.join("consent.json")).ok())
        .and_then(|s| serde_json::from_str(&s).ok());
    opted_out_with(|k| std::env::var(k).ok(), consent)
}

/// The install id shared by every publik app (`telemetry.json`).
fn telemetry_install_id(dir: Option<&std::path::Path>) -> String {
    let Some(path) = dir.map(|d| d.join("telemetry.json")) else {
        return uuid_v4();
    };
    if let Ok(text) = std::fs::read_to_string(&path) {
        return serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| {
                v.get("install_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .filter(|id| is_uuid_v4(id))
            .unwrap_or_else(uuid_v4);
    }
    let id = uuid_v4();
    let text = serde_json::to_string_pretty(&json!({"install_id": id, "created_at": utc_now()}))
        .unwrap_or_default();
    let _ = write_private(&path, &text, true);
    // Another app may have created it first.
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| {
            v.get("install_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|saved| is_uuid_v4(saved))
        .unwrap_or(id)
}

pub fn telemetry_body(event: &str, install_id: &str) -> Value {
    let mut body = json!({
        "v": 1,
        "app": APP_SLUG,
        "install_id": install_id,
        "event": event,
        "app_version": APP_VERSION.trim_start_matches(['v', 'V']),
        "os": OS,
    });
    if let Some(a) = arch() {
        body["arch"] = json!(a);
    }
    body
}

/// Sends one event; never retries, never fails the app.
pub fn send_event(event: &str) {
    let debug = std::env::var("PUBLIK_TELEMETRY_DEBUG").is_ok_and(|v| v == "1");
    if opted_out() {
        if debug {
            eprintln!("publik-telemetry: {event} -> off (opt-out)");
        }
        return;
    }
    let env = Env::from_process();
    let url = env_nonempty("PUBLIK_TELEMETRY_URL")
        .unwrap_or_else(|| format!("{}{TELEMETRY_PATH}", env.origin));
    let result = post_event(&env, &url, event);
    if debug {
        let shown = result.map(|code| code.to_string()).unwrap_or_else(|e| e);
        eprintln!("publik-telemetry: {event} -> {shown}");
    }
}

/// `POST /api/v1/telemetry` with this computer's shared install id, and the
/// key when there is one. Returns the status.
pub fn post_event(env: &Env, url: &str, event: &str) -> Result<u16, String> {
    let body = telemetry_body(event, &telemetry_install_id(env.dir.as_deref()));
    let mut args: Vec<String> = vec![
        "-m".into(),
        "5".into(),
        "-X".into(),
        "POST".into(),
        "-H".into(),
        "content-type: application/json".into(),
        "--data-raw".into(),
        body.to_string(),
    ];
    let config = match env.key() {
        Some(key) => {
            args.extend(["-K".into(), "-".into()]);
            auth_config(&key)
        }
        None => String::new(),
    };
    args.push(url.to_string());
    curl(&args, &config).map(|r| r.code)
}

pub fn telemetry_interval() -> std::time::Duration {
    let secs = env_nonempty("PUBLIK_TELEMETRY_INTERVAL_SECONDS")
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|s| (1..=86_400).contains(s))
        .unwrap_or(1800);
    std::time::Duration::from_secs(secs)
}

/// `app_opened` now, then `active_session` every 30 minutes while the app
/// runs. Once per process; only after sign-in.
pub fn start_telemetry() {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        send_event("app_opened");
        loop {
            std::thread::sleep(telemetry_interval());
            send_event("active_session");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_are_v4() {
        for _ in 0..50 {
            let id = uuid_v4();
            assert!(is_uuid_v4(&id), "{id}");
            assert_eq!(id, id.to_lowercase());
        }
        assert!(!is_uuid_v4("not-a-uuid"));
        assert!(!is_uuid_v4("123e4567-e89b-12d3-a456-426614174000"));
    }

    #[test]
    fn install_body_has_publiks_fields() {
        let body = install_body("pat_x", "id-1", "Finns-Mac");
        assert_eq!(body["app_slug"], APP_SLUG);
        assert_eq!(body["disclosure_version"], 1);
        assert_eq!(body["dialects"], json!(["chat_completions"]));
        assert_eq!(body["install_id"], "id-1");
        assert_eq!(body["device_name"], "Finns-Mac");
        assert_eq!(body["os"], OS);
    }

    #[test]
    fn telemetry_body_strips_the_v() {
        let body = telemetry_body("app_opened", "id-2");
        assert_eq!(body["v"], 1);
        assert_eq!(body["event"], "app_opened");
        assert_eq!(body["app"], APP_SLUG);
        assert!(!body["app_version"].as_str().unwrap().starts_with('v'));
    }

    #[test]
    fn opt_outs() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert!(!opted_out_with(env(&[]), None));
        assert!(opted_out_with(env(&[("DO_NOT_TRACK", "1")]), None));
        assert!(!opted_out_with(env(&[("DO_NOT_TRACK", "false")]), None));
        assert!(!opted_out_with(env(&[("DO_NOT_TRACK", "0")]), None));
        assert!(opted_out_with(env(&[("PUBLIK_TELEMETRY", "off")]), None));
        assert!(opted_out_with(env(&[]), Some(json!({"usage": false}))));
        assert!(!opted_out_with(env(&[]), Some(json!({"usage": true}))));
    }

    #[test]
    fn plans_and_claims() {
        assert!(is_claimed(&json!({"claim_state": "claimed"})));
        assert!(!is_claimed(&json!({"claim_state": "unclaimed"})));
        assert!(has_plan(&json!({"plan": {"kind": "pro"}})));
        assert!(has_plan(
            &json!({"plan": {"kind": "none"}, "balance_micros": 5})
        ));
        assert!(!has_plan(
            &json!({"plan": {"kind": "none"}, "balance_micros": 0})
        ));
        assert!(!has_plan(&json!({})));
    }

    #[test]
    fn provisioning_sends_publiks_install_and_keeps_the_id() {
        let publik = super::super::fake::FakePublik::start();
        let env = super::super::fake::env(&publik);
        let key = provision(&env).unwrap();
        assert_eq!(key, "pk_test_1");
        let cred = env.read_credential();
        assert_eq!(cred["key"], "pk_test_1");
        assert!(cred["claim_url"].as_str().unwrap().ends_with("/claim/abc"));
        let id = cred["install_id"].as_str().unwrap().to_string();
        assert!(is_uuid_v4(&id));
        // A second install of the same computer reuses its id.
        provision(&env).unwrap();
        let bodies = publik.install_bodies();
        assert_eq!(bodies[0]["install_id"], id.as_str());
        assert_eq!(bodies[1]["install_id"], id.as_str());
        assert_eq!(bodies[0]["app_token"], "pat_test");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(env.credential_path().unwrap())
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn the_self_test_line() {
        let publik = super::super::fake::FakePublik::start();
        let env = super::super::fake::env(&publik);
        assert_eq!(
            self_test_line(&env),
            format!("publik self-test: ok {APP_SLUG} {APP_VERSION}")
        );
        let mut down = env.clone();
        down.origin = "http://127.0.0.1:9".into();
        down.api_key = Some("pk_x".into());
        assert_eq!(
            self_test_line(&down),
            format!("publik self-test: failed: {UNREACHABLE}")
        );
        let mut unknown = env;
        unknown.api_key = Some("pk_unknown".into());
        assert_eq!(
            self_test_line(&unknown),
            "publik self-test: failed: Unknown key."
        );
    }

    #[test]
    fn usage_events_carry_the_shared_install_id_and_the_key() {
        let publik = super::super::fake::FakePublik::start();
        let env = super::super::fake::env(&publik);
        let url = format!("{}{TELEMETRY_PATH}", publik.origin());
        assert_eq!(post_event(&env, &url, "app_opened"), Ok(202));
        provision(&env).unwrap();
        assert_eq!(post_event(&env, &url, "active_session"), Ok(202));
        let events = publik.telemetry();
        assert_eq!(events[0].0["event"], "app_opened");
        assert_eq!(events[0].1, None);
        assert_eq!(events[1].0["event"], "active_session");
        assert_eq!(events[1].1.as_deref(), Some("pk_test_1"));
        assert_eq!(events[0].0["install_id"], events[1].0["install_id"]);
        let saved: Value = serde_json::from_str(
            &std::fs::read_to_string(env.dir.unwrap().join("telemetry.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(saved["install_id"], events[0].0["install_id"]);
        assert!(saved["created_at"].as_str().unwrap().ends_with('Z'));
    }

    #[test]
    fn the_scan_strings_are_here() {
        let src = include_str!("client.rs");
        for s in ["/api/v1/installs", "/api/v1/telemetry", "\"app_opened\""] {
            assert!(src.contains(s), "{s}");
        }
        let module = include_str!("mod.rs");
        assert!(module.contains("APP_SLUG: &str = \""));
    }
}
