// SPDX-License-Identifier: GPL-3.0-or-later
//! Automatic sync: pair once, then the two devices stay in step whenever
//! both apps run on the same local network.
//!
//! ```text
//!   Mac (listens)                              iPhone (looks, connects)
//!   advertise "ab12cd34 Finns MacBook Pro"  ◄──  browse
//!   first message: PairRequest → show a code     "Find my Mac" → type the code
//!                  Hello       → session    ◄──  connect to each paired Mac found
//!   session::run  ◄──── sealed states, both ways ────►  session::run
//! ```
//!
//! One manager thread per app keeps that true: the Mac advertises while
//! sync is on; the phone browses while it is paired (or searching) and
//! connects to a paired Mac as soon as it is found, again after a drop.
//! On iPhone all of this lives only while the app's process does: open, or
//! in a dictation session.
//!
//! The pieces: [`net`](super::net) carries bytes, [`secure`](super::secure)
//! pairs and seals, [`session`](super::session) exchanges and merges,
//! [`peers`](super::peers) remembers who is paired.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::net::{self, ConnectFail, Event, NativeLink};
use super::peers::Peers;
use super::secure::{self, First, Identity, Link, Refusal};
use super::session::{self, Control, End, Host};

/// Which side of the pair this device is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SyncRole {
    /// Listens and shows the pairing code.
    Mac,
    /// Looks for Macs and connects.
    Phone,
}

/// What the engine needs from the app.
pub trait App: Host {
    /// The state on screen changed (`fork_sync_auto_status` has news).
    fn changed(&self);
    /// A pairing code is up: bring the window that shows it forward.
    fn attention(&self) {}
    /// `active`, `inactive` or `background` (iPhone); `active` elsewhere.
    fn app_state(&self) -> &'static str {
        "active"
    }
}

/// A Mac found on the network.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FoundDevice {
    /// What to pass to `fork_sync_pair`.
    pub service: String,
    pub name: String,
    /// Already paired with this device.
    pub paired: bool,
}

/// A paired device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PairedDevice {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub paired_at: i64,
    pub last_sync_at: Option<i64>,
    /// A session with it is up right now.
    pub connected: bool,
    /// It is on the network (the phone sees the Mac).
    pub found: bool,
    /// Its app speaks another version of sync: one of the two needs an
    /// update before they sync again.
    pub needs_update: bool,
}

/// Where a pairing stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum SyncPairing {
    /// Phone: reaching the Mac.
    Connecting { device_name: String },
    /// Phone: the Mac shows a code; the user types it here.
    EnterCode { device_name: String },
    /// Phone: the code is being checked.
    Checking { device_name: String },
    /// Mac: show `code` to the phone's user.
    ShowCode { device_name: String, code: String },
    /// It did not work: `wrong_code`, `no_code`, `busy`, `unreachable`
    /// (never connected), `denied` (no local network access on this
    /// device), `dropped` (the connection broke midway), `update_this`
    /// (this app is the older one), `update_mac` / `update_phone` (the
    /// other one is), `version` (one of the two, not known which),
    /// `failed`.
    Failed {
        device_name: String,
        problem: String,
    },
}

/// Everything the Sync pages show about automatic sync.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AutoStatus {
    pub role: SyncRole,
    pub device_name: String,
    /// Automatic sync is on.
    pub enabled: bool,
    pub devices: Vec<PairedDevice>,
    /// Phone: Macs on the network, while searching.
    pub found: Vec<FoundDevice>,
    pub searching: bool,
    pub pairing: Option<SyncPairing>,
    /// The system refused local network access.
    pub network_denied: bool,
}

#[derive(Default)]
struct State {
    /// Service names on the network.
    found: BTreeSet<String>,
    /// Device id → the running session's control.
    sessions: HashMap<String, Arc<Control>>,
    /// Phone: device ids a connection attempt is running for.
    connecting: BTreeSet<String>,
    /// Phone: when a failed peer may be tried again.
    retry_at: HashMap<String, Instant>,
    pairing: Option<SyncPairing>,
    /// Phone: where the typed code goes.
    code: Option<Sender<Option<String>>>,
    searching_until: Option<Instant>,
    advertising: bool,
    browsing: bool,
    /// Since when the system reports local network access as refused.
    denied_since: Option<Instant>,
    /// Phone: the pairing's connection was held back for want of local
    /// network access.
    connect_denied: bool,
    /// Paired devices whose app speaks another protocol version.
    outdated: BTreeSet<String>,
    /// In-memory sync times (the file is written less often).
    synced: BTreeMap<String, i64>,
}

/// The running engine.
pub struct Engine {
    app: Arc<dyn App>,
    role: SyncRole,
    me: Identity,
    peers: Mutex<Peers>,
    state: Mutex<State>,
}

static ENGINE: OnceLock<Arc<Engine>> = OnceLock::new();

/// The engine, once [`start`] ran.
pub fn engine() -> Option<Arc<Engine>> {
    ENGINE.get().cloned()
}

/// How long "Find my Mac" keeps looking.
const SEARCH_FOR: Duration = Duration::from_secs(120);
/// How long before a Mac that refused or dropped is tried again.
const RETRY_AFTER: Duration = Duration::from_secs(5);
const CONNECT_WAIT: Duration = Duration::from_secs(10);
/// How long a pairing waits for the user to answer, or to allow in
/// Settings, "find devices on your local network".
const PERMISSION_WAIT: Duration = Duration::from_secs(60);
/// How long the network must stay refused before the page says so (the
/// first-time permission question reads as "refused" until it is answered).
const DENIED_AFTER: Duration = Duration::from_secs(12);

/// The service name a Mac advertises: its id's first 8 characters, a
/// space, its name. The id lets a phone find its Mac again after a rename.
pub fn service_name(me: &Identity) -> String {
    let id: String = me.device_id.chars().take(8).collect();
    let mut name = format!("{id} {}", me.name);
    // A Bonjour instance name holds 63 bytes.
    while name.len() > 63 {
        name.pop();
    }
    name
}

/// `(id prefix, display name)` of a service name.
pub fn parse_service(service: &str) -> Option<(&str, &str)> {
    let (id, name) = service.split_once(' ')?;
    (id.len() == 8 && id.chars().all(|c| c.is_ascii_alphanumeric())).then_some((id, name))
}

fn now_ms() -> i64 {
    crate::fork::context::now_ms()
}

/// Starts automatic sync for this app: loads the pairings from `data_dir`
/// and runs the manager. Idempotent.
pub fn start(app: Arc<dyn App>, role: SyncRole, me: Identity, data_dir: &Path) -> Arc<Engine> {
    ENGINE
        .get_or_init(|| {
            let engine = Arc::new(Engine {
                app,
                role,
                me,
                peers: Mutex::new(Peers::load(data_dir)),
                state: Mutex::new(State::default()),
            });
            let events = net::events();
            let for_events = engine.clone();
            let for_manager = engine.clone();
            let spawned = std::thread::Builder::new()
                .name("wb-sync-events".into())
                .spawn(move || for_events.on_events(events))
                .and_then(|_| {
                    std::thread::Builder::new()
                        .name("wb-sync-manager".into())
                        .spawn(move || for_manager.manage())
                });
            if let Err(e) = spawned {
                log::warn!("sync: could not start automatic sync: {e}");
            }
            engine
        })
        .clone()
}

impl Engine {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn peers(&self) -> MutexGuard<'_, Peers> {
        self.peers.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ── What the pages read and ask ─────────────────────────────────────

    pub fn status(&self) -> AutoStatus {
        let peers = self.peers();
        let state = self.state();
        let found_ids: Vec<&str> = state
            .found
            .iter()
            .filter_map(|s| parse_service(s))
            .map(|(id, _)| id)
            .collect();
        let devices = peers
            .all()
            .iter()
            .map(|p| PairedDevice {
                device_id: p.device_id.clone(),
                name: p.name.clone(),
                platform: p.platform.clone(),
                paired_at: p.paired_at,
                last_sync_at: state.synced.get(&p.device_id).copied().or(p.last_sync_at),
                connected: state.sessions.contains_key(&p.device_id),
                found: found_ids.iter().any(|id| p.device_id.starts_with(id)),
                needs_update: state.outdated.contains(&p.device_id),
            })
            .collect();
        let found = state
            .found
            .iter()
            .filter_map(|service| {
                let (id, name) = parse_service(service)?;
                Some(FoundDevice {
                    service: service.clone(),
                    name: name.to_owned(),
                    paired: peers.all().iter().any(|p| p.device_id.starts_with(id)),
                })
            })
            .collect();
        AutoStatus {
            role: self.role,
            device_name: self.me.name.clone(),
            enabled: peers.enabled(),
            devices,
            found,
            searching: state.searching_until.is_some_and(|t| t > Instant::now()),
            pairing: state.pairing.clone(),
            // While the system's own question is still on screen the
            // network also reads as refused: say so only once it stays.
            network_denied: state
                .denied_since
                .is_some_and(|since| since.elapsed() >= DENIED_AFTER),
        }
    }

    /// The Mac's switch. Off also ends the running sessions.
    pub fn set_enabled(&self, on: bool) {
        self.peers().set_enabled(on);
        if !on {
            for control in self.state().sessions.values() {
                control.stop.store(true, Ordering::Release);
            }
        }
        self.app.changed();
    }

    /// Phone: look for Macs for a while ("Find my Mac").
    pub fn search(&self, on: bool) {
        self.state().searching_until = on.then(|| Instant::now() + SEARCH_FOR);
        self.app.changed();
    }

    /// Send this device's state now on every session ("Sync now"); a peer
    /// that is waiting for a retry is tried at once.
    pub fn sync_now(&self) {
        let mut state = self.state();
        for control in state.sessions.values() {
            control.now.store(true, Ordering::Release);
        }
        state.retry_at.clear();
    }

    /// Removes a pairing here and, if it is connected, there.
    pub fn unpair(&self, device_id: &str) {
        self.peers().remove(device_id);
        let mut state = self.state();
        if let Some(control) = state.sessions.get(device_id) {
            control.unpair.store(true, Ordering::Release);
        }
        state.synced.remove(device_id);
        drop(state);
        log::info!("sync: a device was unpaired");
        self.app.changed();
    }

    /// Phone: the code the user typed (`None`: cancel the pairing).
    pub fn pair_code(&self, code: Option<String>) {
        let mut state = self.state();
        match state.code.take() {
            Some(tx) => {
                if let (Some(_), Some(SyncPairing::EnterCode { device_name })) =
                    (&code, state.pairing.clone())
                {
                    state.pairing = Some(SyncPairing::Checking { device_name });
                } else {
                    state.pairing = None;
                }
                let _ = tx.send(code);
            }
            // Dismissing a failure (the Mac's code stays up until it is used).
            None if code.is_none()
                && !matches!(state.pairing, Some(SyncPairing::ShowCode { .. })) =>
            {
                state.pairing = None;
            }
            None => {}
        }
        drop(state);
        self.app.changed();
    }

    /// Phone: start pairing with the Mac advertised as `service`.
    pub fn pair(self: &Arc<Self>, service: String) {
        let name = parse_service(&service)
            .map(|(_, name)| name.to_owned())
            .unwrap_or_default();
        {
            let mut state = self.state();
            if matches!(
                state.pairing,
                Some(
                    SyncPairing::Connecting { .. }
                        | SyncPairing::EnterCode { .. }
                        | SyncPairing::Checking { .. }
                )
            ) {
                return;
            }
            state.pairing = Some(SyncPairing::Connecting {
                device_name: name.clone(),
            });
        }
        self.app.changed();
        let engine = self.clone();
        let spawned = std::thread::Builder::new()
            .name("wb-sync-pair".into())
            .spawn(move || {
                let result = engine.pair_with(&service, &name);
                if let Err(why) = result {
                    log::info!("sync: pairing did not work ({why:?})");
                }
                let mut state = engine.state();
                let denied = state.denied_since.is_some() || state.connect_denied;
                state.code = None;
                state.pairing = match result {
                    Ok(()) => None,
                    // Cancelled on the phone.
                    Err(_) if state.pairing.is_none() => None,
                    Err(why) => Some(SyncPairing::Failed {
                        device_name: name,
                        problem: match why {
                            Refusal::WrongCode => "wrong_code",
                            Refusal::NoCode => "no_code",
                            Refusal::Busy => "busy",
                            Refusal::Unreachable if denied => "denied",
                            Refusal::Unreachable => "unreachable",
                            Refusal::Closed => "dropped",
                            Refusal::PeerOlder => "update_mac",
                            Refusal::PeerNewer => "update_this",
                            Refusal::Version => "version",
                            _ => "failed",
                        }
                        .into(),
                    }),
                };
                drop(state);
                engine.app.changed();
            });
        if spawned.is_err() {
            self.state().pairing = None;
        }
    }

    fn pair_with(&self, service: &str, name: &str) -> Result<(), Refusal> {
        let link = self.reach(service).ok_or(Refusal::Unreachable)?;
        let result = secure::pair_as_phone(&link, &self.me, |mac| {
            let (tx, rx) = channel();
            {
                let mut state = self.state();
                state.code = Some(tx);
                state.pairing = Some(SyncPairing::EnterCode {
                    device_name: if mac.name.is_empty() {
                        name.to_owned()
                    } else {
                        mac.name.clone()
                    },
                });
            }
            self.app.changed();
            rx.recv_timeout(secure::CODE_WAIT).ok().flatten()
        });
        link.close();
        let (mac, key) = result?;
        self.peers().add(&mac, &key, now_ms());
        {
            let mut state = self.state();
            state.retry_at.clear();
            // Found: "Find my Mac" is over.
            state.searching_until = None;
        }
        log::info!("sync: paired with a Mac");
        Ok(())
    }

    /// Connects to `service` for a pairing. The first connection may wait
    /// on the system's "find devices on your local network" question:
    /// [`net::connect`] waits for the answer, and a browser that reported
    /// "denied" gets more tries while the user goes to Settings.
    fn reach(&self, service: &str) -> Option<NativeLink> {
        let until = Instant::now() + PERMISSION_WAIT;
        self.state().connect_denied = false;
        loop {
            match net::connect(service, CONNECT_WAIT, PERMISSION_WAIT) {
                Ok(link) => {
                    self.state().connect_denied = false;
                    return Some(link);
                }
                Err(ConnectFail::Denied) => {
                    self.state().connect_denied = true;
                }
                Err(ConnectFail::Unreachable) => {}
            }
            let waiting_for_permission = {
                let state = self.state();
                state.denied_since.is_some() || state.connect_denied
            };
            // Cancelled meanwhile.
            let cancelled = self.state().pairing.is_none();
            if !waiting_for_permission || cancelled || Instant::now() >= until {
                return None;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    // ── The network's side ──────────────────────────────────────────────

    fn on_events(self: Arc<Self>, events: Receiver<Event>) {
        for event in events {
            match event {
                Event::Found(service) => {
                    log::info!("sync: found a device on the network");
                    self.state().found.insert(service);
                }
                Event::Lost(service) => {
                    self.state().found.remove(&service);
                }
                Event::Incoming(link) => {
                    let engine = self.clone();
                    let _ = std::thread::Builder::new()
                        .name("wb-sync-serve".into())
                        .spawn(move || engine.serve(link));
                    continue;
                }
                Event::Listener(what) | Event::Browser(what) => {
                    log::info!("sync: Bonjour {what}");
                    let mut state = self.state();
                    match what.as_str() {
                        "denied" => {
                            state.denied_since.get_or_insert_with(Instant::now);
                        }
                        "ready" => state.denied_since = None,
                        "failed" => {
                            // The manager starts it again.
                            state.advertising = false;
                            state.browsing = false;
                        }
                        _ => {}
                    }
                }
            }
            self.app.changed();
        }
    }

    /// Mac: one connection from a phone, from its first message to its end.
    fn serve(self: Arc<Self>, link: NativeLink) {
        if self.role != SyncRole::Mac || !self.peers().enabled() {
            link.close();
            return;
        }
        match secure::first(&link) {
            Ok(First::Pair(phone)) => self.serve_pairing(&link, phone),
            Ok(First::Hello(hello)) => {
                let key = self.peers().get(&hello.device_id).and_then(|p| p.key());
                let Some(key) = key else {
                    secure::refuse(&link, Refusal::Unknown);
                    link.close();
                    return;
                };
                let peer = hello.device_id.clone();
                match secure::accept(link, &self.me.device_id, &hello, &key) {
                    Ok(mut channel) => self.run_session(&mut channel, &peer),
                    Err(why) => log::info!("sync: a session was refused ({why:?})"),
                }
            }
            Err(why) => {
                log::info!("sync: a connection's first message was refused ({why:?})");
                link.close();
            }
        }
    }

    fn serve_pairing(&self, link: &NativeLink, phone: Identity) {
        let code = {
            let mut state = self.state();
            if matches!(state.pairing, Some(SyncPairing::ShowCode { .. })) {
                drop(state);
                secure::refuse(link, Refusal::Busy);
                link.close();
                return;
            }
            let code = test_code().unwrap_or_else(secure::new_code);
            state.pairing = Some(SyncPairing::ShowCode {
                device_name: phone.name.clone(),
                code: code.clone(),
            });
            code
        };
        log::info!("sync: a phone asked to pair; showing a code");
        self.app.changed();
        self.app.attention();
        let result = secure::pair_as_mac(link, &self.me, &phone, &code);
        if let Err(why) = &result {
            log::info!("sync: pairing with a phone did not work ({why:?})");
        }
        link.close();
        let mut state = self.state();
        state.pairing = match &result {
            Ok(_) | Err(Refusal::NoCode) | Err(Refusal::Closed) => None,
            Err(Refusal::WrongCode) => Some(SyncPairing::Failed {
                device_name: phone.name.clone(),
                problem: "wrong_code".into(),
            }),
            Err(why) => Some(SyncPairing::Failed {
                device_name: phone.name.clone(),
                problem: match why {
                    // On the Mac: the phone's app is older or newer.
                    Refusal::PeerOlder => "update_phone",
                    Refusal::PeerNewer => "update_this",
                    Refusal::Version => "version",
                    _ => "failed",
                }
                .into(),
            }),
        };
        drop(state);
        if let Ok(key) = result {
            self.peers().add(&phone, &key, now_ms());
            log::info!("sync: paired with a phone");
        }
        self.app.changed();
    }

    /// Phone: a session with the paired Mac `device_id`, found as `service`.
    fn dial(self: Arc<Self>, device_id: String, service: String, key: secure::PairKey) {
        let outcome = net::connect(&service, CONNECT_WAIT, CONNECT_WAIT)
            .map_err(|_| Refusal::Unreachable)
            .and_then(|link| secure::connect(link, &self.me.device_id, &device_id, &key));
        {
            let mut state = self.state();
            state.connecting.remove(&device_id);
            if outcome.as_ref().err().is_some_and(|e| e.is_version()) {
                state.outdated.insert(device_id.clone());
            } else if outcome.is_ok() {
                state.outdated.remove(&device_id);
            }
        }
        match outcome {
            Ok(mut channel) => self.run_session(&mut channel, &device_id),
            Err(Refusal::Unknown) => {
                // The Mac no longer knows this phone: the pairing is over.
                log::info!("sync: the Mac removed this pairing");
                self.peers().remove(&device_id);
            }
            Err(why) => log::info!("sync: could not reach the Mac ({why:?})"),
        }
        self.state()
            .retry_at
            .insert(device_id, Instant::now() + RETRY_AFTER);
        self.app.changed();
    }

    fn run_session<L: Link>(&self, channel: &mut secure::Channel<L>, peer: &str) {
        let control = Arc::new(Control::default());
        if let Some(old) = self
            .state()
            .sessions
            .insert(peer.to_owned(), control.clone())
        {
            // One session per pair: the newer connection replaces the older.
            old.stop.store(true, Ordering::Release);
        }
        log::info!("sync: session started");
        self.app.changed();
        let end = session::run(channel, &Tracking(self), peer, &control);
        {
            let mut state = self.state();
            if state
                .sessions
                .get(peer)
                .is_some_and(|c| Arc::ptr_eq(c, &control))
            {
                state.sessions.remove(peer);
            }
        }
        if end == End::Unpaired {
            self.peers().remove(peer);
            log::info!("sync: the other device removed the pairing");
        }
        log::info!("sync: session ended ({end:?})");
        self.app.changed();
    }

    /// Keeps advertising, browsing and the phone's connections as they
    /// should be. Once a second; everything it does is idempotent.
    fn manage(self: Arc<Self>) {
        let mut app_state = self.app.app_state();
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let enabled = self.peers().enabled();
            match self.role {
                SyncRole::Mac => {
                    let mut state = self.state();
                    if enabled && !state.advertising {
                        net::advertise(&service_name(&self.me));
                        state.advertising = true;
                    } else if !enabled && state.advertising {
                        net::stop_advertise();
                        state.advertising = false;
                    }
                }
                SyncRole::Phone => {
                    let paired: Vec<(String, Option<secure::PairKey>)> = self
                        .peers()
                        .all()
                        .iter()
                        .map(|p| (p.device_id.clone(), p.key()))
                        .collect();
                    let now_state = self.app.app_state();
                    let woke = now_state == "active" && app_state != "active";
                    app_state = now_state;
                    let mut dial = Vec::new();
                    {
                        let mut state = self.state();
                        let searching = state.searching_until.is_some_and(|t| t > Instant::now());
                        if state.searching_until.is_some() && !searching {
                            state.searching_until = None;
                        }
                        let want = searching || (enabled && !paired.is_empty());
                        if woke && state.browsing {
                            // A browser from before the app slept may be stale.
                            net::stop_browse();
                            state.browsing = false;
                            state.found.clear();
                        } else if want && !state.browsing {
                            net::browse();
                            state.browsing = true;
                        } else if !want && state.browsing {
                            net::stop_browse();
                            state.browsing = false;
                            state.found.clear();
                        }
                        if enabled {
                            for (id, key) in &paired {
                                let Some(key) = key else { continue };
                                if state.sessions.contains_key(id)
                                    || state.connecting.contains(id)
                                    || state.retry_at.get(id).is_some_and(|t| *t > Instant::now())
                                {
                                    continue;
                                }
                                let service = state
                                    .found
                                    .iter()
                                    .find(|s| {
                                        parse_service(s).is_some_and(|(p, _)| id.starts_with(p))
                                    })
                                    .cloned();
                                if let Some(service) = service {
                                    state.connecting.insert(id.clone());
                                    dial.push((id.clone(), service, *key));
                                }
                            }
                        }
                    }
                    for (id, service, key) in dial {
                        let engine = self.clone();
                        let _ = std::thread::Builder::new()
                            .name("wb-sync-dial".into())
                            .spawn(move || engine.dial(id, service, key));
                    }
                }
            }
        }
    }
}

/// The session's host, with the sync time kept for the pages.
struct Tracking<'a>(&'a Engine);

impl Host for Tracking<'_> {
    fn collect(&self) -> Option<(super::Manifest, super::Payload)> {
        self.0.app.collect()
    }

    fn merge(&self, manifest: super::Manifest, payload: super::Payload) -> bool {
        self.0.app.merge(manifest, payload)
    }

    fn synced(&self, peer_id: &str) {
        let now = now_ms();
        self.0.peers().synced(peer_id, now);
        self.0.state().synced.insert(peer_id.to_owned(), now);
        self.0.app.changed();
    }
}

/// Debug builds only: a fixed pairing code for the end-to-end check
/// (`WB_SYNC_TEST_CODE`), so a script can type it on the other device.
fn test_code() -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var("WB_SYNC_TEST_CODE")
        .ok()
        .filter(|c| c.len() == 6 && c.chars().all(|c| c.is_ascii_digit()))
}
