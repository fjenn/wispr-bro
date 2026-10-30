// SPDX-License-Identifier: GPL-3.0-or-later
//! Test helpers and tests for the fork plumbing (`cfg(test)` only).
//!
//! The tests build a real Tauri app on `tauri::test::MockRuntime` with the
//! fork plugin, a stand-in for Handy's `.setup()` closure and a stand-in Handy
//! invoke handler, all wired exactly as `lib.rs` wires the real ones. Handy's
//! own handler and managers are typed to `tauri::Wry` and cannot run inside a
//! unit test (a Wry event loop needs the macOS main thread), so the parts that
//! touch Handy's real code are covered by source-level checks of `lib.rs` and
//! the generated `src/bindings.ts`, plus `cargo build` itself (the real
//! `compose_invoke(invoke_handler)` call must type-check against Handy's
//! handler).
//!
//! # Test fakes for later slices (F-10)
//!
//! Small traits plus in-memory fakes for the five things fork logic must not
//! touch for real in a unit test: time ([`Clock`] / [`FakeClock`]), the
//! Accessibility API ([`AxProvider`] / [`FakeAx`]), the pasteboard
//! ([`Pasteboard`] / [`FakePasteboard`]), Handy's transcription coordinator
//! ([`Coordinator`] / [`FakeCoordinator`]) and the network ([`Network`] /
//! [`FakeNetwork`]). R5 §11.1 asks every fork module's unit tests to use them.
//!
//! **Where the traits live (decision).** R5 §4.1 allows `testutil.rs` only
//! under `cfg(test)`, and no production code implements these traits yet. So
//! for now each trait is defined here, test-only, as the agreed contract. The
//! slice that first needs a trait in production code moves it, unchanged in
//! shape, into the module that owns it and replaces the definition here with
//! `pub use` so every fake and test keeps compiling:
//!
//! | Trait          | Owner module when it ships          | Slice        |
//! | -------------- | ----------------------------------- | ------------ |
//! | `Clock`        | `fork::shortcut` (shared by `stats`, `db` retention) | S-01 |
//! | `AxProvider`   | `fork::ax`                          | S-05         |
//! | `Pasteboard`   | `fork::clipboard` (moved in S-04)   | S-04         |
//! | `Coordinator`  | `fork::shortcut` (wraps `TranscriptionCoordinator::external_command`) | S-01 |
//! | `Network`      | `fork::net`                         | F-08 / S-10  |
//!
//! The fakes are deliberately simple models, not ports of macOS behaviour.
//! Real-Mac behaviour is checked by `docs/fork/MAC-TEST-SCRIPT.md`.

#![allow(dead_code)] // The fakes are an API for later slices; not every method is used yet.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use super::commands::{ForkModelStatus, HandyModels};

/// Locks a fake's state; a poisoned lock (a panicking test) still yields data.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------------------
// Clock
// ---------------------------------------------------------------------------

/// Time source. Monotonic time drives timers (hotkey grammar `T = 500 ms`,
/// clipboard restore delay, idle shutdown); wall time drives local days and
/// retention.
pub trait Clock: Send + Sync {
    /// Monotonic time since an arbitrary origin. Never goes backwards.
    fn now(&self) -> Duration;
    /// Wall-clock time as Unix milliseconds. May jump (NTP, manual change).
    fn unix_ms(&self) -> i64;
}

impl<T: Clock + ?Sized> Clock for Arc<T> {
    fn now(&self) -> Duration {
        (**self).now()
    }
    fn unix_ms(&self) -> i64 {
        (**self).unix_ms()
    }
}

/// Manually driven clock. Starts at monotonic zero.
#[derive(Debug)]
pub struct FakeClock {
    state: Mutex<(Duration, i64)>,
}

impl FakeClock {
    /// 2026-01-01T00:00:00Z, a fixed default wall time for tests.
    pub const DEFAULT_UNIX_MS: i64 = 1_767_225_600_000;

    pub fn new() -> Self {
        Self::at_unix_ms(Self::DEFAULT_UNIX_MS)
    }

    pub fn at_unix_ms(unix_ms: i64) -> Self {
        Self {
            state: Mutex::new((Duration::ZERO, unix_ms)),
        }
    }

    /// Moves both monotonic and wall time forward.
    pub fn advance(&self, by: Duration) {
        let mut s = lock(&self.state);
        s.0 += by;
        s.1 += by.as_millis() as i64;
    }

    pub fn advance_ms(&self, ms: u64) {
        self.advance(Duration::from_millis(ms));
    }

    /// Sets wall time only (a clock change); monotonic time is unchanged.
    pub fn set_unix_ms(&self, unix_ms: i64) {
        lock(&self.state).1 = unix_ms;
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Duration {
        lock(&self.state).0
    }
    fn unix_ms(&self) -> i64 {
        lock(&self.state).1
    }
}

// ---------------------------------------------------------------------------
// Accessibility (AX)
// ---------------------------------------------------------------------------

/// An AX answer. R5 §4.6: every caller tolerates `Unknown` (AX timed out,
/// empty Electron tree, permission missing), and an `Unknown` focus with a
/// focused window means "paste anyway" (fail-open).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ax<T> {
    Known(T),
    Unknown,
}

impl<T> Ax<T> {
    pub fn known(self) -> Option<T> {
        match self {
            Ax::Known(v) => Some(v),
            Ax::Unknown => None,
        }
    }
}

/// Snapshot of a focused UI element, the fields `fork::ax` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxElement {
    /// Stable id for "is this still the same element" (target rule, §4.6).
    pub id: u64,
    pub pid: i32,
    pub bundle_id: String,
    /// `AXRole`, e.g. `AXTextField`, `AXTextArea`, `AXComboBox`, `AXWebArea`.
    pub role: String,
    pub subrole: Option<String>,
    pub enabled: bool,
    /// `AXSecureTextField` or a password field.
    pub secure: bool,
    /// `AXValue` is settable.
    pub value_settable: bool,
    pub value: String,
    /// Selected range in UTF-16 units `(location, length)`.
    pub selection: Option<(usize, usize)>,
}

impl AxElement {
    /// A plain editable text area in the given app.
    pub fn text_area(id: u64, bundle_id: &str) -> Self {
        Self {
            id,
            pid: 1000 + id as i32,
            bundle_id: bundle_id.into(),
            role: "AXTextArea".into(),
            subrole: None,
            enabled: true,
            secure: false,
            value_settable: true,
            value: String::new(),
            selection: Some((0, 0)),
        }
    }

    /// A password field.
    pub fn secure_field(id: u64, bundle_id: &str) -> Self {
        Self {
            role: "AXTextField".into(),
            subrole: Some("AXSecureTextField".into()),
            secure: true,
            ..Self::text_area(id, bundle_id)
        }
    }
}

/// Read side of the AX layer (`fork::ax`, S-05).
pub trait AxProvider: Send + Sync {
    fn focused_element(&self) -> Ax<AxElement>;
    fn selected_text(&self) -> Ax<String>;
    fn window_title(&self) -> Ax<String>;
    fn document_url(&self) -> Ax<String>;
    /// Whether any process holds Secure Event Input.
    fn secure_input_active(&self) -> bool;
}

impl<T: AxProvider + ?Sized> AxProvider for Arc<T> {
    fn focused_element(&self) -> Ax<AxElement> {
        (**self).focused_element()
    }
    fn selected_text(&self) -> Ax<String> {
        (**self).selected_text()
    }
    fn window_title(&self) -> Ax<String> {
        (**self).window_title()
    }
    fn document_url(&self) -> Ax<String> {
        (**self).document_url()
    }
    fn secure_input_active(&self) -> bool {
        (**self).secure_input_active()
    }
}

#[derive(Debug)]
struct FakeAxState {
    focus: Ax<AxElement>,
    /// Answers returned by the next `focused_element` calls, before `focus`.
    scripted: VecDeque<Ax<AxElement>>,
    window_title: Ax<String>,
    document_url: Ax<String>,
    secure_input: bool,
    calls: Vec<&'static str>,
}

/// Scriptable AX provider. Starts with `Unknown` focus.
#[derive(Debug)]
pub struct FakeAx {
    state: Mutex<FakeAxState>,
}

impl Default for FakeAx {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeAx {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(FakeAxState {
                focus: Ax::Unknown,
                scripted: VecDeque::new(),
                window_title: Ax::Unknown,
                document_url: Ax::Unknown,
                secure_input: false,
                calls: Vec::new(),
            }),
        }
    }

    /// Sets the focused element (`None` = nothing focused, e.g. the desktop).
    pub fn focus(&self, element: Option<AxElement>) {
        lock(&self.state).focus = match element {
            Some(e) => Ax::Known(e),
            None => Ax::Unknown,
        };
    }

    /// Makes the next `focused_element` calls return these answers in order
    /// (e.g. focus moves between key-up and delivery), then the current focus.
    pub fn script_focus(&self, answers: impl IntoIterator<Item = Ax<AxElement>>) {
        lock(&self.state).scripted.extend(answers);
    }

    pub fn set_window_title(&self, title: Option<&str>) {
        lock(&self.state).window_title = title.map_or(Ax::Unknown, |t| Ax::Known(t.into()));
    }

    pub fn set_document_url(&self, url: Option<&str>) {
        lock(&self.state).document_url = url.map_or(Ax::Unknown, |u| Ax::Known(u.into()));
    }

    pub fn set_secure_input(&self, on: bool) {
        lock(&self.state).secure_input = on;
    }

    /// Simulates text landing in the focused element at its selection (what a
    /// successful paste does), so paste-verification code sees `AXValue` change.
    /// Returns `false` when nothing editable is focused.
    pub fn insert_into_focused(&self, text: &str) -> bool {
        let mut s = lock(&self.state);
        let Ax::Known(el) = &mut s.focus else {
            return false;
        };
        if el.secure || !el.value_settable || !el.enabled {
            return false;
        }
        let units: Vec<u16> = el.value.encode_utf16().collect();
        let (loc, len) = el.selection.unwrap_or((units.len(), 0));
        let loc = loc.min(units.len());
        let end = (loc + len).min(units.len());
        let mut out: Vec<u16> = units[..loc].to_vec();
        out.extend(text.encode_utf16());
        let caret = out.len();
        out.extend_from_slice(&units[end..]);
        el.value = String::from_utf16_lossy(&out);
        el.selection = Some((caret, 0));
        true
    }

    /// Names of the trait methods called so far, in order.
    pub fn calls(&self) -> Vec<&'static str> {
        lock(&self.state).calls.clone()
    }
}

impl AxProvider for FakeAx {
    fn focused_element(&self) -> Ax<AxElement> {
        let mut s = lock(&self.state);
        s.calls.push("focused_element");
        match s.scripted.pop_front() {
            Some(answer) => answer,
            None => s.focus.clone(),
        }
    }

    fn selected_text(&self) -> Ax<String> {
        let mut s = lock(&self.state);
        s.calls.push("selected_text");
        match &s.focus {
            Ax::Known(el) if !el.secure => {
                let units: Vec<u16> = el.value.encode_utf16().collect();
                match el.selection {
                    Some((loc, len)) if len > 0 && loc + len <= units.len() => {
                        Ax::Known(String::from_utf16_lossy(&units[loc..loc + len]))
                    }
                    Some(_) => Ax::Known(String::new()),
                    None => Ax::Unknown,
                }
            }
            _ => Ax::Unknown,
        }
    }

    fn window_title(&self) -> Ax<String> {
        let mut s = lock(&self.state);
        s.calls.push("window_title");
        s.window_title.clone()
    }

    fn document_url(&self) -> Ax<String> {
        let mut s = lock(&self.state);
        s.calls.push("document_url");
        s.document_url.clone()
    }

    fn secure_input_active(&self) -> bool {
        let mut s = lock(&self.state);
        s.calls.push("secure_input_active");
        s.secure_input
    }
}

// ---------------------------------------------------------------------------
// Pasteboard
// ---------------------------------------------------------------------------

// S-04 moved the trait and the item type into `fork::clipboard` (see the
// table at the top); the names stay available here for the fakes and tests.
pub use super::clipboard::markers::{
    AUTO_GENERATED as UTI_AUTO_GENERATED, CONCEALED as UTI_CONCEALED, TEXT as UTI_PLAIN_TEXT,
    TRANSIENT as UTI_TRANSIENT,
};
pub use super::clipboard::{Pasteboard, PasteboardItem};

/// Who wrote to a [`FakePasteboard`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteboardWriter {
    /// Through the [`Pasteboard`] trait (the code under test).
    App,
    /// Through [`FakePasteboard::user_copy`] (another app, the user).
    User,
}

#[derive(Debug, Default)]
struct FakePasteboardState {
    change_count: i64,
    items: Vec<PasteboardItem>,
    log: Vec<(PasteboardWriter, Vec<PasteboardItem>)>,
}

/// In-memory pasteboard with a write log.
#[derive(Debug, Default)]
pub struct FakePasteboard {
    state: Mutex<FakePasteboardState>,
}

impl FakePasteboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts with `items` already on the pasteboard (change count 1, not logged).
    pub fn with_items(items: Vec<PasteboardItem>) -> Self {
        let pb = Self::default();
        {
            let mut s = lock(&pb.state);
            s.items = items;
            s.change_count = 1;
        }
        pb
    }

    /// Another app (or the user) copies something.
    pub fn user_copy(&self, items: Vec<PasteboardItem>) -> i64 {
        self.write_as(PasteboardWriter::User, items)
    }

    fn write_as(&self, who: PasteboardWriter, items: Vec<PasteboardItem>) -> i64 {
        let mut s = lock(&self.state);
        s.change_count += 1;
        s.items = items.clone();
        s.log.push((who, items));
        s.change_count
    }

    /// Every write so far, in order.
    pub fn log(&self) -> Vec<(PasteboardWriter, Vec<PasteboardItem>)> {
        lock(&self.state).log.clone()
    }

    /// App writes a clipboard manager would record: no item carries the
    /// transient or concealed marker (R5 §4.7, RS-05).
    pub fn writes_visible_to_clipboard_managers(&self) -> Vec<Vec<PasteboardItem>> {
        lock(&self.state)
            .log
            .iter()
            .filter(|(who, _)| *who == PasteboardWriter::App)
            .filter(|(_, items)| {
                !items
                    .iter()
                    .any(|i| i.has(UTI_TRANSIENT) || i.has(UTI_CONCEALED))
            })
            .map(|(_, items)| items.clone())
            .collect()
    }
}

impl Pasteboard for FakePasteboard {
    fn change_count(&self) -> i64 {
        lock(&self.state).change_count
    }
    fn read(&self) -> Vec<PasteboardItem> {
        lock(&self.state).items.clone()
    }
    fn write(&self, items: Vec<PasteboardItem>) -> i64 {
        self.write_as(PasteboardWriter::App, items)
    }
}

// ---------------------------------------------------------------------------
// Transcription coordinator
// ---------------------------------------------------------------------------

/// Commands the fork sends through `TranscriptionCoordinator::external_command`
/// (R5 §4.2, S-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoordinatorCommand {
    Start,
    Stop,
    Lock,
    Cancel,
}

/// Origin tag carried with each command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandOrigin {
    Hotkey,
    Pill,
    Cli,
    Recovery,
    Test,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinatorState {
    Idle,
    Recording { locked: bool },
    Processing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinatorError {
    /// The command makes no sense in the current state.
    InvalidTransition {
        from: CoordinatorState,
        cmd: CoordinatorCommand,
    },
    /// Scripted failure (see [`FakeCoordinator::fail_next`]).
    Failed(String),
}

/// Outcome recorded for each command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinatorOutcome {
    Applied(CoordinatorState),
    /// A start while processing is queued and runs when processing ends (A-206).
    Queued,
    Rejected(CoordinatorError),
}

/// The fork's view of Handy's coordinator.
pub trait Coordinator: Send + Sync {
    fn external_command(
        &self,
        cmd: CoordinatorCommand,
        origin: CommandOrigin,
    ) -> Result<(), CoordinatorError>;
    fn state(&self) -> CoordinatorState;
}

impl<T: Coordinator + ?Sized> Coordinator for Arc<T> {
    fn external_command(
        &self,
        cmd: CoordinatorCommand,
        origin: CommandOrigin,
    ) -> Result<(), CoordinatorError> {
        (**self).external_command(cmd, origin)
    }
    fn state(&self) -> CoordinatorState {
        (**self).state()
    }
}

#[derive(Debug)]
struct FakeCoordinatorState {
    state: CoordinatorState,
    queued_start: bool,
    fail_next: Option<String>,
    log: Vec<(CoordinatorCommand, CommandOrigin, CoordinatorOutcome)>,
}

/// State-machine model of the coordinator: Idle → Recording → Processing →
/// Idle, Lock while recording, Cancel from any busy state, one Start queued
/// while processing. A simplified model; S-01 checks it against Handy's real
/// coordinator.
#[derive(Debug)]
pub struct FakeCoordinator {
    state: Mutex<FakeCoordinatorState>,
}

impl Default for FakeCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeCoordinator {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(FakeCoordinatorState {
                state: CoordinatorState::Idle,
                queued_start: false,
                fail_next: None,
                log: Vec::new(),
            }),
        }
    }

    /// Makes the next command fail with `message` (state unchanged).
    pub fn fail_next(&self, message: &str) {
        lock(&self.state).fail_next = Some(message.into());
    }

    /// Ends processing (transcription delivered). A queued start begins now.
    pub fn finish_processing(&self) {
        let mut s = lock(&self.state);
        if s.state == CoordinatorState::Processing {
            s.state = if std::mem::take(&mut s.queued_start) {
                CoordinatorState::Recording { locked: false }
            } else {
                CoordinatorState::Idle
            };
        }
    }

    pub fn log(&self) -> Vec<(CoordinatorCommand, CommandOrigin, CoordinatorOutcome)> {
        lock(&self.state).log.clone()
    }

    /// Commands only, in order.
    pub fn commands(&self) -> Vec<CoordinatorCommand> {
        lock(&self.state).log.iter().map(|(c, _, _)| *c).collect()
    }
}

impl Coordinator for FakeCoordinator {
    fn external_command(
        &self,
        cmd: CoordinatorCommand,
        origin: CommandOrigin,
    ) -> Result<(), CoordinatorError> {
        use CoordinatorCommand as C;
        use CoordinatorState as S;
        let mut s = lock(&self.state);
        if let Some(message) = s.fail_next.take() {
            let err = CoordinatorError::Failed(message);
            s.log
                .push((cmd, origin, CoordinatorOutcome::Rejected(err.clone())));
            return Err(err);
        }
        let from = s.state;
        let next = match (from, cmd) {
            (S::Idle, C::Start) => Some(S::Recording { locked: false }),
            (S::Recording { .. }, C::Lock) => Some(S::Recording { locked: true }),
            (S::Recording { .. }, C::Stop) => Some(S::Processing),
            (S::Recording { .. } | S::Processing, C::Cancel) => {
                s.queued_start = false;
                Some(S::Idle)
            }
            (S::Processing, C::Start) if !s.queued_start => {
                s.queued_start = true;
                s.log.push((cmd, origin, CoordinatorOutcome::Queued));
                return Ok(());
            }
            _ => None,
        };
        match next {
            Some(state) => {
                s.state = state;
                s.log
                    .push((cmd, origin, CoordinatorOutcome::Applied(state)));
                Ok(())
            }
            None => {
                let err = CoordinatorError::InvalidTransition { from, cmd };
                s.log
                    .push((cmd, origin, CoordinatorOutcome::Rejected(err.clone())));
                Err(err)
            }
        }
    }

    fn state(&self) -> CoordinatorState {
        lock(&self.state).state
    }
}

// ---------------------------------------------------------------------------
// Network
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn get(url: &str) -> Self {
        Self {
            method: "GET".into(),
            url: url.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn post_json(url: &str, body: &str) -> Self {
        Self {
            method: "POST".into(),
            url: url.into(),
            headers: vec![("content-type".into(), "application/json".into())],
            body: body.as_bytes().to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn ok(body: &str) -> Self {
        Self::status(200, body)
    }

    pub fn status(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.as_bytes().to_vec(),
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetError {
    /// Networking is off.
    Offline,
    /// The host is not on the privacy allow-list (R5 §1).
    Blocked {
        host: String,
    },
    Timeout,
    /// No scripted response for this URL.
    NoRoute(String),
}

/// Blocking HTTP transport. S-10's async reqwest client gets an adapter or an
/// async twin with the same request/response types.
pub trait Network: Send + Sync {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse, NetError>;
}

impl<T: Network + ?Sized> Network for Arc<T> {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse, NetError> {
        (**self).send(req)
    }
}

/// Host part of a URL (lower-cased, no userinfo or port), or `None`.
pub fn url_host(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host_port = authority.rsplit('@').next()?;
    let host = if let Some(stripped) = host_port.strip_prefix('[') {
        stripped.split(']').next()?
    } else {
        host_port.split(':').next()?
    };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Hosts of R5 §1's privacy promise, plus loopback for the local LLM sidecar
/// (DEC-49). Test-only copy: F-08's `fork::net` owns the real list and the CI
/// guards (§4.12) enforce it; this one lets fakes fail loudly on a stray host.
pub fn host_is_allowed(host: &str) -> bool {
    const EXACT: &[&str] = &[
        "github.com",
        "objects.githubusercontent.com",
        "release-assets.githubusercontent.com",
        "huggingface.co",
        "127.0.0.1",
        "localhost",
        "::1",
    ];
    const SUFFIX: &[&str] = &[".hf.co", ".huggingface.co"];
    EXACT.contains(&host) || SUFFIX.iter().any(|s| host.ends_with(s))
}

#[derive(Debug, Default)]
struct FakeNetworkState {
    offline: bool,
    /// URL prefix → responses in order; the last one repeats.
    routes: Vec<(String, VecDeque<Result<HttpResponse, NetError>>)>,
    requests: Vec<HttpRequest>,
    blocked: Vec<String>,
}

/// Scripted network. Hosts outside [`host_is_allowed`] fail with
/// [`NetError::Blocked`] and are recorded, so a test can assert the code under
/// test never tried one.
#[derive(Debug, Default)]
pub struct FakeNetwork {
    state: Mutex<FakeNetworkState>,
}

impl FakeNetwork {
    pub fn new() -> Self {
        Self::default()
    }

    /// Responds to URLs starting with `prefix` with `responses` in order; the
    /// last response repeats. The longest matching prefix wins.
    pub fn route(
        &self,
        prefix: &str,
        responses: impl IntoIterator<Item = Result<HttpResponse, NetError>>,
    ) {
        lock(&self.state)
            .routes
            .push((prefix.into(), responses.into_iter().collect()));
    }

    pub fn set_offline(&self, offline: bool) {
        lock(&self.state).offline = offline;
    }

    /// Every request that reached the fake (including blocked ones).
    pub fn requests(&self) -> Vec<HttpRequest> {
        lock(&self.state).requests.clone()
    }

    /// Hosts that were refused by the allow-list.
    pub fn blocked_hosts(&self) -> Vec<String> {
        lock(&self.state).blocked.clone()
    }
}

impl Network for FakeNetwork {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse, NetError> {
        let mut s = lock(&self.state);
        s.requests.push(req.clone());
        let host = url_host(&req.url).unwrap_or_default();
        if !host_is_allowed(&host) {
            s.blocked.push(host.clone());
            return Err(NetError::Blocked { host });
        }
        if s.offline {
            return Err(NetError::Offline);
        }
        let best = s
            .routes
            .iter_mut()
            .filter(|(prefix, _)| req.url.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len());
        match best {
            Some((_, queue)) if queue.len() > 1 => queue.pop_front().expect("non-empty"),
            Some((_, queue)) => queue
                .front()
                .cloned()
                .unwrap_or_else(|| Err(NetError::NoRoute(req.url.clone()))),
            None => Err(NetError::NoRoute(req.url.clone())),
        }
    }
}

/// All five fakes behind `Arc`s, for tests that wire a whole flow.
#[derive(Clone, Default)]
pub struct Fakes {
    pub clock: Arc<FakeClock>,
    pub ax: Arc<FakeAx>,
    pub pasteboard: Arc<FakePasteboard>,
    pub coordinator: Arc<FakeCoordinator>,
    pub network: Arc<FakeNetwork>,
}

impl Fakes {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Header map helper for [`HttpRequest`]/[`HttpResponse`] header lists.
pub fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// Stand-in for Handy's `ModelManager` (same `Arc<…>` state shape).
#[derive(Debug, Clone, Default)]
pub struct FakeModelManager {
    pub status: Option<ForkModelStatus>,
}

impl HandyModels for FakeModelManager {
    fn model_status(&self) -> ForkModelStatus {
        self.status.clone().unwrap_or(ForkModelStatus {
            known_models: 0,
            downloaded_model_ids: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use serde_json::{json, Value};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime};
    use tauri::webview::InvokeRequest;
    use tauri::{App, Manager, RunEvent, WebviewWindow, WebviewWindowBuilder};
    use tauri_specta::Event;

    use super::FakeModelManager;
    use crate::fork::commands::ForkModelStatus;
    use crate::fork::events::ForkPingEvent;
    use crate::fork::{self, ForkState};
    use crate::CliArgs;

    /// Stand-in for a Handy command (Handy's real ones are `Wry`-only).
    #[tauri::command]
    fn get_current_model() -> String {
        "handy-model".into()
    }

    fn sample_status() -> ForkModelStatus {
        ForkModelStatus {
            known_models: 3,
            downloaded_model_ids: vec!["parakeet-v3".into()],
        }
    }

    /// Builds an app wired like `lib.rs`: fork plugin in the plugin chain,
    /// `CliArgs` managed, a `.setup()` that manages the model manager (like
    /// `initialize_core_logic`, skipped in headless mode) and the composed
    /// invoke handler.
    fn build_app(cli_args: CliArgs) -> App<MockRuntime> {
        let headless = fork::is_headless(&cli_args);
        mock_builder()
            .plugin(fork::init())
            .manage(cli_args)
            .setup(move |app| {
                if !headless {
                    app.manage(Arc::new(FakeModelManager {
                        status: Some(sample_status()),
                    }));
                }
                Ok(())
            })
            .invoke_handler(fork::compose_invoke(tauri::generate_handler![
                get_current_model
            ]))
            .build(mock_context(noop_assets()))
            .expect("failed to build mock app")
    }

    fn main_window(app: &App<MockRuntime>) -> WebviewWindow<MockRuntime> {
        WebviewWindowBuilder::new(app, "main", Default::default())
            .build()
            .expect("failed to build mock window")
    }

    fn invoke(webview: &WebviewWindow<MockRuntime>, cmd: &str) -> Result<Value, Value> {
        get_ipc_response(
            webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: InvokeBody::default(),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<Value>().unwrap())
    }

    /// Runs the mock event loop; `on_ready` receives the window right after
    /// `RunEvent::Ready` (Handy's setup and the plugin's `on_event` have run).
    /// The window is then destroyed, which ends the mock loop.
    fn run_until_ready<T: Send + 'static>(
        app: App<MockRuntime>,
        on_ready: impl Fn(&WebviewWindow<MockRuntime>) -> T + 'static,
    ) -> T {
        let out: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
        let sink = out.clone();
        app.run(move |handle, event| {
            if let RunEvent::Ready = event {
                let window = handle.get_webview_window("main").expect("main window");
                *sink.lock().unwrap() = Some(on_ready(&window));
                window.destroy().unwrap();
            }
        });
        let result = out.lock().unwrap().take();
        result.expect("RunEvent::Ready was never delivered")
    }

    #[test]
    fn model_status_is_not_ready_before_ready_and_handy_state_after() {
        let app = build_app(CliArgs::default());
        let window = main_window(&app);

        // Plugin setup has run (inside build()); Handy's setup has not.
        assert!(app.try_state::<ForkState>().is_some());
        assert!(app.try_state::<Arc<FakeModelManager>>().is_none());
        assert_eq!(
            invoke(&window, "fork_model_status"),
            Err(json!({ "kind": "NotReady" }))
        );

        let after = run_until_ready(app, |w| invoke(w, "fork_model_status"));
        assert_eq!(after, Ok(serde_json::to_value(sample_status()).unwrap()));
    }

    #[test]
    fn handy_command_still_resolves_through_composed_handler() {
        let app = build_app(CliArgs::default());
        let window = main_window(&app);
        assert_eq!(
            invoke(&window, "get_current_model"),
            Ok(json!("handy-model"))
        );
        // An unknown non-fork command is still Handy's to reject.
        assert!(invoke(&window, "no_such_command").is_err());
        // An unknown fork command is rejected by the fork handler.
        assert!(invoke(&window, "fork_no_such_command").is_err());
    }

    #[test]
    fn fork_event_round_trips_through_mount_events() {
        assert_eq!(ForkPingEvent::NAME, "fork-ping-event");

        let app = build_app(CliArgs::default());
        let window = main_window(&app);

        // Mount a Handy event into the same app, the way Handy's own builder
        // does in its setup closure, to prove both builders share one registry.
        tauri_specta::Builder::<MockRuntime>::new()
            .events(tauri_specta::collect_events![
                crate::transcription_coordinator::RecordingStateEvent
            ])
            .mount_events(&app);
        // `listen` panics if the event is not in the registry.
        crate::transcription_coordinator::RecordingStateEvent::listen(&app, |_| {});

        let (tx, rx) = mpsc::channel();
        ForkPingEvent::listen(&app, move |event| {
            let _ = tx.send(event.payload);
        });

        let reply = invoke(&window, "fork_ping").expect("fork_ping failed");
        assert_eq!(reply["message"], json!("pong"));
        assert_eq!(reply["seq"], json!(1));
        assert_eq!(reply["ready"], json!(false));

        let received = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("ForkPingEvent was not received");
        assert_eq!(received, ForkPingEvent { seq: 1 });
    }

    #[test]
    fn headless_cli_mode_skips_on_ready_and_opens_no_windows() {
        for args in [
            CliArgs {
                list_models: true,
                ..Default::default()
            },
            CliArgs {
                list_devices: true,
                ..Default::default()
            },
            CliArgs {
                transcribe_file: Some("x.wav".into()),
                ..Default::default()
            },
        ] {
            assert!(fork::is_headless(&args));
            let app = build_app(args);
            let window = main_window(&app);
            let (status, ready, windows) = run_until_ready(app, |w| {
                let handle = w.app_handle();
                (
                    invoke(w, "fork_model_status"),
                    handle.state::<ForkState>().is_ready(),
                    handle.webview_windows().keys().cloned().collect::<Vec<_>>(),
                )
            });
            drop(window);
            assert_eq!(status, Err(json!({ "kind": "NotReady" })));
            assert!(!ready);
            assert_eq!(windows, vec!["main".to_string()]);
        }
        assert!(!fork::is_headless(&CliArgs::default()));
    }

    fn repo_file(rel: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
    }

    /// Command names in Handy's `collect_commands![…]` list in `lib.rs`.
    fn handy_command_names() -> Vec<String> {
        let lib = repo_file("src/lib.rs");
        let start = lib
            .find("collect_commands![")
            .expect("collect_commands! not found in lib.rs")
            + "collect_commands![".len();
        let end = start + lib[start..].find(']').expect("unterminated list");
        lib[start..end]
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|path| path.rsplit("::").next().unwrap().to_string())
            .collect()
    }

    #[test]
    fn no_handy_command_name_starts_with_fork_prefix() {
        let names = handy_command_names();
        assert!(names.len() > 50, "parsed only {} commands", names.len());
        let clashes: Vec<_> = names.iter().filter(|n| fork::is_fork_command(n)).collect();
        assert!(clashes.is_empty(), "Handy commands use fork_: {clashes:?}");

        // The generated Handy bindings agree.
        let bindings = repo_file("../src/bindings.ts");
        assert!(bindings.contains("TAURI_INVOKE(\"get_app_settings\""));
        assert!(!bindings.contains("TAURI_INVOKE(\"fork_"));
        assert!(!bindings.contains("\"fork-"));
    }

    #[test]
    fn compose_invoke_is_the_only_invoke_handler() {
        let lib = repo_file("src/lib.rs");
        // `.invoke_handler(` calls on the Tauri builder; `specta_builder.invoke_handler()`
        // (which only creates Handy's handler) is not one.
        let calls =
            lib.matches(".invoke_handler(").count() - lib.matches(".invoke_handler()").count();
        assert_eq!(calls, 1);
        assert!(lib.contains(".invoke_handler(fork::compose_invoke(invoke_handler))"));
        assert!(lib.contains(".plugin(fork::init())"));
    }

    #[test]
    fn build_rs_still_ends_in_plain_tauri_build() {
        let build = repo_file("build.rs");
        assert!(build.contains("    tauri_build::build()\n}"));
        assert!(!build.contains("AppManifest"));
    }

    #[test]
    fn fork_bindings_are_current_and_handy_bindings_untouched() {
        let handy_before = repo_file("../src/bindings.ts");

        let fork_builder = fork::specta_builder::<MockRuntime>();
        let generated = fork_builder
            .export_str(fork::bindings_language())
            .expect("export fork bindings");
        // Export to a scratch file too, exactly like `compose_invoke` does.
        let dir = tempfile::tempdir().unwrap();
        fork_builder
            .export(fork::bindings_language(), dir.path().join("bindings.ts"))
            .expect("export fork bindings to file");

        assert_ne!(fork::FORK_BINDINGS_PATH, "../src/bindings.ts");
        assert_eq!(
            repo_file("../src/bindings.ts"),
            handy_before,
            "fork codegen changed Handy's src/bindings.ts"
        );
        assert!(generated.contains("TAURI_INVOKE(\"fork_ping\""));
        assert!(generated.contains("TAURI_INVOKE(\"fork_model_status\""));
        assert!(generated.contains("\"fork-ping-event\""));
        assert!(!generated.contains("get_app_settings"));

        if std::env::var_os("WB_WRITE_FORK_BINDINGS").is_some() {
            let path =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(fork::FORK_BINDINGS_PATH);
            std::fs::write(path, &generated).unwrap();
        }
        assert_eq!(
            repo_file("../src/fork/bindings.ts"),
            generated,
            "src/fork/bindings.ts is stale: run a debug build (`bun run wb:dev`) or this test with WB_WRITE_FORK_BINDINGS=1"
        );
    }
}

/// Self-tests for the F-10 fakes, plus one small flow per fake that shows how
/// a later slice is expected to use it.
#[cfg(test)]
mod fake_tests {
    use super::*;

    #[test]
    fn fake_clock_advances_monotonic_and_wall_time_and_wall_can_jump() {
        let clock = FakeClock::new();
        assert_eq!(clock.now(), Duration::ZERO);
        assert_eq!(clock.unix_ms(), FakeClock::DEFAULT_UNIX_MS);
        clock.advance_ms(500);
        assert_eq!(clock.now(), Duration::from_millis(500));
        assert_eq!(clock.unix_ms(), FakeClock::DEFAULT_UNIX_MS + 500);
        // A wall-clock change never moves monotonic time.
        clock.set_unix_ms(0);
        assert_eq!(clock.unix_ms(), 0);
        assert_eq!(clock.now(), Duration::from_millis(500));
        // Shared through Arc<dyn Clock>.
        let shared: Arc<dyn Clock> = Arc::new(FakeClock::at_unix_ms(42));
        assert_eq!(shared.unix_ms(), 42);
    }

    /// Example of the S-01 pattern: a decision that depends on elapsed time.
    fn is_hold(clock: &dyn Clock, pressed_at: Duration) -> bool {
        clock.now().saturating_sub(pressed_at) >= Duration::from_millis(500)
    }

    #[test]
    fn fake_clock_drives_a_timing_rule_without_sleeping() {
        let clock = FakeClock::new();
        let down = clock.now();
        clock.advance_ms(499);
        assert!(!is_hold(&clock, down));
        clock.advance_ms(1);
        assert!(is_hold(&clock, down));
    }

    #[test]
    fn fake_ax_fail_open_focus_script_and_insertion() {
        let ax = FakeAx::new();
        assert_eq!(ax.focused_element(), Ax::Unknown);

        let mut field = AxElement::text_area(1, "com.apple.TextEdit");
        field.value = "Hello world".into();
        field.selection = Some((6, 5));
        ax.focus(Some(field));
        assert_eq!(ax.selected_text(), Ax::Known("world".into()));
        assert!(ax.insert_into_focused("there"));
        let el = ax.focused_element().known().unwrap();
        assert_eq!(el.value, "Hello there");
        assert_eq!(el.selection, Some((11, 0)));

        // Focus moves between key-up and delivery (S-05 target rule).
        ax.script_focus([Ax::Known(AxElement::text_area(2, "com.apple.Notes"))]);
        assert_eq!(ax.focused_element().known().unwrap().id, 2);
        assert_eq!(ax.focused_element().known().unwrap().id, 1);

        // Secure fields refuse insertion and hide their selection.
        ax.focus(Some(AxElement::secure_field(3, "com.apple.Safari")));
        assert!(!ax.insert_into_focused("secret"));
        assert_eq!(ax.selected_text(), Ax::Unknown);

        ax.set_secure_input(true);
        ax.set_window_title(Some("Inbox"));
        ax.set_document_url(Some("https://mail.example.com/"));
        assert!(ax.secure_input_active());
        assert_eq!(ax.window_title(), Ax::Known("Inbox".into()));
        assert_eq!(
            ax.document_url(),
            Ax::Known("https://mail.example.com/".into())
        );
        assert_eq!(ax.calls().first(), Some(&"focused_element"));
    }

    #[test]
    fn fake_ax_insertion_counts_utf16_units() {
        let ax = FakeAx::new();
        let mut field = AxElement::text_area(1, "com.tinyspeck.slackmacgap");
        field.value = "a😀b".into(); // the emoji is two UTF-16 units
        field.selection = Some((3, 0));
        ax.focus(Some(field));
        assert!(ax.insert_into_focused("X"));
        assert_eq!(ax.focused_element().known().unwrap().value, "a😀Xb");
    }

    #[test]
    fn fake_pasteboard_change_count_log_and_manager_visibility() {
        let rtf = PasteboardItem::default()
            .with("public.rtf", b"{\\rtf1 hi}")
            .with(UTI_PLAIN_TEXT, b"hi");
        let pb = FakePasteboard::with_items(vec![rtf.clone()]);
        assert_eq!(pb.change_count(), 1);
        assert_eq!(pb.read(), vec![rtf]);

        let marked = PasteboardItem::text("dictated")
            .with(UTI_TRANSIENT, b"")
            .with(UTI_AUTO_GENERATED, b"");
        assert_eq!(pb.write(vec![marked]), 2);
        assert_eq!(pb.write(vec![PasteboardItem::text("unmarked")]), 3);
        assert_eq!(pb.user_copy(vec![PasteboardItem::text("user")]), 4);

        let visible = pb.writes_visible_to_clipboard_managers();
        assert_eq!(visible, vec![vec![PasteboardItem::text("unmarked")]]);
        let writers: Vec<_> = pb.log().into_iter().map(|(w, _)| w).collect();
        assert_eq!(
            writers,
            [
                PasteboardWriter::App,
                PasteboardWriter::App,
                PasteboardWriter::User
            ]
        );
    }

    /// Example of the S-04 pattern: snapshot, write marked text, wait, restore
    /// only if nobody else wrote in between.
    fn paste_and_restore(pb: &dyn Pasteboard, clock: &FakeClock, text: &str, user_copies: bool) {
        let snapshot = pb.read();
        let ours = pb.write(vec![PasteboardItem::text(text).with(UTI_TRANSIENT, b"")]);
        if user_copies {
            pb.write(vec![PasteboardItem::text("copied meanwhile")]);
        }
        clock.advance_ms(1500);
        if pb.change_count() == ours {
            pb.write(snapshot);
        }
    }

    #[test]
    fn fake_pasteboard_supports_the_restore_guard_flow() {
        let clock = FakeClock::new();
        let image = PasteboardItem::default().with("public.png", &[0x89, b'P', b'N', b'G']);
        let pb = FakePasteboard::with_items(vec![image.clone()]);
        paste_and_restore(&pb, &clock, "hello", false);
        assert_eq!(pb.read(), vec![image.clone()], "restored byte-identical");

        let pb = FakePasteboard::with_items(vec![image]);
        paste_and_restore(&pb, &clock, "hello", true);
        assert_eq!(
            pb.read(),
            vec![PasteboardItem::text("copied meanwhile")],
            "no restore over a newer copy"
        );
    }

    #[test]
    fn fake_coordinator_transitions_queue_and_failures() {
        use CommandOrigin::*;
        use CoordinatorCommand::*;
        let c = FakeCoordinator::new();
        assert_eq!(c.state(), CoordinatorState::Idle);
        assert!(c.external_command(Stop, Hotkey).is_err());
        c.external_command(Start, Hotkey).unwrap();
        c.external_command(Lock, Hotkey).unwrap();
        assert_eq!(c.state(), CoordinatorState::Recording { locked: true });
        c.external_command(Stop, Pill).unwrap();
        assert_eq!(c.state(), CoordinatorState::Processing);

        // One start is queued while busy (A-206); a second is rejected.
        c.external_command(Start, Hotkey).unwrap();
        assert!(c.external_command(Start, Hotkey).is_err());
        c.finish_processing();
        assert_eq!(c.state(), CoordinatorState::Recording { locked: false });

        c.external_command(Cancel, Recovery).unwrap();
        assert_eq!(c.state(), CoordinatorState::Idle);
        c.fail_next("boom");
        assert_eq!(
            c.external_command(Start, Cli),
            Err(CoordinatorError::Failed("boom".into()))
        );
        assert_eq!(c.state(), CoordinatorState::Idle);

        assert_eq!(
            c.commands(),
            [Stop, Start, Lock, Stop, Start, Start, Cancel, Start]
        );
        let outcomes: Vec<_> = c.log().into_iter().map(|(_, _, o)| o).collect();
        assert_eq!(outcomes[4], CoordinatorOutcome::Queued);
    }

    #[test]
    fn url_host_parses_common_shapes() {
        assert_eq!(
            url_host("https://github.com/o/r/releases/latest/download/latest.json").as_deref(),
            Some("github.com")
        );
        assert_eq!(
            url_host("https://user@CAS-Bridge.xethub.hf.co:443/x").as_deref(),
            Some("cas-bridge.xethub.hf.co")
        );
        assert_eq!(
            url_host("http://127.0.0.1:8080/v1/chat/completions").as_deref(),
            Some("127.0.0.1")
        );
        assert_eq!(url_host("http://[::1]:9/").as_deref(), Some("::1"));
        assert_eq!(url_host("not a url"), None);
    }

    #[test]
    fn fake_network_routes_offline_and_privacy_allow_list() {
        let net = FakeNetwork::new();
        net.route(
            "https://huggingface.co/",
            [
                Ok(HttpResponse::status(429, "").with_header("Retry-After", "2")),
                Ok(HttpResponse::ok("weights")),
            ],
        );
        net.route("https://huggingface.co/special/", [Err(NetError::Timeout)]);

        let first = net
            .send(HttpRequest::get("https://huggingface.co/m"))
            .unwrap();
        assert_eq!(first.status, 429);
        assert_eq!(header(&first.headers, "retry-after"), Some("2"));
        // The last response repeats.
        for _ in 0..2 {
            let r = net
                .send(HttpRequest::get("https://huggingface.co/m"))
                .unwrap();
            assert_eq!(r.body, b"weights");
        }
        // Longest prefix wins.
        assert_eq!(
            net.send(HttpRequest::get("https://huggingface.co/special/x")),
            Err(NetError::Timeout)
        );
        assert!(matches!(
            net.send(HttpRequest::get("https://github.com/x")),
            Err(NetError::NoRoute(_))
        ));

        // Hosts outside R5 §1 are refused and recorded, even offline.
        net.set_offline(true);
        assert_eq!(
            net.send(HttpRequest::post_json(
                "https://telemetry.example.com/e",
                "{}"
            )),
            Err(NetError::Blocked {
                host: "telemetry.example.com".into()
            })
        );
        assert_eq!(
            net.send(HttpRequest::get("https://huggingface.co/m")),
            Err(NetError::Offline)
        );
        assert_eq!(net.blocked_hosts(), ["telemetry.example.com"]);
        assert_eq!(net.requests().len(), 7);
    }

    #[test]
    fn allow_list_matches_r5_privacy_promise() {
        for ok in [
            "github.com",
            "objects.githubusercontent.com",
            "release-assets.githubusercontent.com",
            "huggingface.co",
            "cdn-lfs.hf.co",
            "cas-bridge.xethub.hf.co",
            "cdn-lfs-us-1.huggingface.co",
            "127.0.0.1",
            "localhost",
        ] {
            assert!(host_is_allowed(ok), "{ok}");
        }
        for bad in [
            "api.openai.com",
            "evilhuggingface.co",
            "githubusercontent.com",
            "fonts.googleapis.com",
            "hf.co.evil.com",
        ] {
            assert!(!host_is_allowed(bad), "{bad}");
        }
    }

    #[test]
    fn fakes_bundle_shares_state_through_trait_objects() {
        let fakes = Fakes::new();
        let clock: Arc<dyn Clock> = fakes.clock.clone();
        let coordinator: Arc<dyn Coordinator> = fakes.coordinator.clone();
        let pasteboard: Arc<dyn Pasteboard> = fakes.pasteboard.clone();
        let ax: Arc<dyn AxProvider> = fakes.ax.clone();
        let network: Arc<dyn Network> = fakes.network.clone();

        fakes.clock.advance_ms(10);
        assert_eq!(clock.now(), Duration::from_millis(10));
        coordinator
            .external_command(CoordinatorCommand::Start, CommandOrigin::Test)
            .unwrap();
        assert_eq!(fakes.coordinator.commands(), [CoordinatorCommand::Start]);
        pasteboard.write(vec![PasteboardItem::text("x")]);
        assert_eq!(fakes.pasteboard.change_count(), 1);
        assert_eq!(ax.focused_element(), Ax::Unknown);
        assert!(network
            .send(HttpRequest::get("https://github.com/"))
            .is_err());
        assert_eq!(fakes.network.requests().len(), 1);
    }
}
