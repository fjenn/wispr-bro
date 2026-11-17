// SPDX-License-Identifier: GPL-3.0-or-later
//! Automatic sync on the wire: pairing, session keys, sealed messages and
//! the exchange itself, over an in-memory link that can listen in, change,
//! repeat and cut what passes; then the same over the real local network.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::super::peers::Peers;
use super::super::secure::{
    self, accept, connect, first, pair_as_mac, pair_as_phone, Channel, First, Identity, Link,
    PairKey, Recv, Refusal,
};
use super::super::session::{self, Control, End, Host};
use super::super::{Manifest, Payload, SourceKind};
use super::*;

// ── An in-memory link ───────────────────────────────────────────────────

type Tap = Arc<dyn Fn(Vec<u8>) -> Vec<Vec<u8>> + Send + Sync>;

/// One end of a two-way queue. `tap` sees every message this end sends and
/// decides what arrives instead (nothing, the same, something else, twice).
struct MemLink {
    tx: Sender<Vec<u8>>,
    rx: Mutex<Receiver<Vec<u8>>>,
    closed: Arc<AtomicBool>,
    tap: Option<Tap>,
}

fn link_pair() -> (MemLink, MemLink) {
    let (a_tx, b_rx) = channel();
    let (b_tx, a_rx) = channel();
    let closed = Arc::new(AtomicBool::new(false));
    let end = |tx, rx| MemLink {
        tx,
        rx: Mutex::new(rx),
        closed: closed.clone(),
        tap: None,
    };
    (end(a_tx, a_rx), end(b_tx, b_rx))
}

impl MemLink {
    fn tapped(mut self, tap: impl Fn(Vec<u8>) -> Vec<Vec<u8>> + Send + Sync + 'static) -> Self {
        self.tap = Some(Arc::new(tap));
        self
    }
}

impl Link for MemLink {
    fn send(&self, bytes: &[u8]) -> bool {
        if self.closed.load(Ordering::Acquire) {
            return false;
        }
        let out = match &self.tap {
            Some(tap) => tap(bytes.to_vec()),
            None => vec![bytes.to_vec()],
        };
        out.into_iter().all(|m| self.tx.send(m).is_ok())
    }

    fn recv(&self, timeout: Duration) -> Recv {
        let deadline = Instant::now() + timeout;
        let rx = self.rx.lock().unwrap();
        loop {
            match rx.recv_timeout(Duration::from_millis(20)) {
                Ok(m) => return Recv::Message(m),
                Err(RecvTimeoutError::Disconnected) => return Recv::Closed,
                Err(RecvTimeoutError::Timeout) => {
                    if self.closed.load(Ordering::Acquire) {
                        return Recv::Closed;
                    }
                    if Instant::now() >= deadline {
                        return Recv::Timeout;
                    }
                }
            }
        }
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }
}

// ── Devices as session hosts ────────────────────────────────────────────

impl Device {
    fn id(&self) -> String {
        self.with(|c| store::self_id(c).unwrap())
    }

    fn identity(&self, platform: &str) -> Identity {
        Identity {
            device_id: self.id(),
            name: self.name.to_owned(),
            platform: platform.to_owned(),
        }
    }
}

/// A device in a session; counts what it merged.
struct Side {
    device: Device,
    merges: Mutex<u32>,
}

fn side(name: &'static str) -> Arc<Side> {
    Arc::new(Side {
        device: device(name),
        merges: Mutex::new(0),
    })
}

impl Host for Side {
    fn collect(&self) -> Option<(Manifest, Payload)> {
        Some(
            self.device
                .with(|c| store::collect(c, self.device.name, "1.0.0-test", now_ms()).unwrap()),
        )
    }

    fn merge(&self, manifest: Manifest, payload: Payload) -> bool {
        self.device.merge(&Incoming {
            kind: SourceKind::Bundle,
            manifest,
            payload,
        });
        *self.merges.lock().unwrap() += 1;
        true
    }
}

fn pair_key() -> PairKey {
    [7u8; 32]
}

/// A session between `phone` and `mac` over `links`, each side on its own
/// thread. Returns the controls and the threads' ends.
struct Running {
    controls: [Arc<Control>; 2],
    threads: Vec<std::thread::JoinHandle<Option<End>>>,
}

fn run_session<P: Link + 'static, M: Link + 'static>(
    phone: &Arc<Side>,
    mac: &Arc<Side>,
    links: (P, M),
    keys: (PairKey, PairKey),
) -> Running {
    let (phone_id, mac_id) = (phone.device.id(), mac.device.id());
    let controls = [Arc::new(Control::default()), Arc::new(Control::default())];
    let (phone_link, mac_link) = links;
    let mac_thread = {
        let (mac, control, mac_id) = (mac.clone(), controls[1].clone(), mac_id.clone());
        std::thread::spawn(move || {
            let First::Hello(hello) = first(&mac_link).ok()? else {
                return None;
            };
            let peer = hello.device_id.clone();
            let mut channel = accept(mac_link, &mac_id, &hello, &keys.1).ok()?;
            Some(session::run(&mut channel, &*mac, &peer, &control))
        })
    };
    let phone_thread = {
        let (phone, control) = (phone.clone(), controls[0].clone());
        std::thread::spawn(move || {
            let mut channel = connect(phone_link, &phone_id, &mac_id, &keys.0).ok()?;
            Some(session::run(&mut channel, &*phone, &mac_id, &control))
        })
    };
    Running {
        controls,
        threads: vec![phone_thread, mac_thread],
    }
}

impl Running {
    fn stop(self) -> Vec<Option<End>> {
        for control in &self.controls {
            control.stop.store(true, Ordering::Release);
        }
        self.threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect()
    }
}

/// Waits until both devices hold the same state (and it satisfies `also`).
fn converge(a: &Side, b: &Side, also: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if a.device.state() == b.device.state() && also() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the devices did not converge:\n{:#?}\n{:#?}",
            a.device.state(),
            b.device.state()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

// ── Pairing ─────────────────────────────────────────────────────────────

/// Runs a pairing; the Mac shows `shown`, the phone's user types `typed`.
fn pair_over(
    links: (MemLink, MemLink),
    shown: &str,
    typed: Option<&str>,
) -> (
    Result<(Identity, PairKey), Refusal>,
    Result<PairKey, Refusal>,
) {
    let phone = Identity {
        device_id: "phone-0123456789abcdef".into(),
        name: "iPhone".into(),
        platform: "ios".into(),
    };
    let mac = Identity {
        device_id: "mac-0123456789abcdef".into(),
        name: "Finns MacBook Pro".into(),
        platform: "macos".into(),
    };
    let (phone_link, mac_link) = links;
    let shown = shown.to_owned();
    let mac_thread = std::thread::spawn(move || {
        let Ok(First::Pair(who)) = first(&mac_link) else {
            return Err(Refusal::Protocol);
        };
        assert_eq!(who.name, "iPhone");
        let result = pair_as_mac(&mac_link, &mac, &who, &shown);
        mac_link.close();
        result
    });
    let typed = typed.map(str::to_owned);
    let on_phone = pair_as_phone(&phone_link, &phone, |mac| {
        assert_eq!(mac.name, "Finns MacBook Pro");
        typed
    });
    phone_link.close();
    (on_phone, mac_thread.join().unwrap())
}

#[test]
fn the_right_code_pairs_and_both_hold_the_same_key() {
    let (phone, mac) = pair_over(link_pair(), "482913", Some("482913"));
    let (who, phone_key) = phone.unwrap();
    assert_eq!(who.device_id, "mac-0123456789abcdef");
    assert_eq!(phone_key, mac.unwrap());
    assert_ne!(phone_key, [0u8; 32]);
    // Another pairing with the same code gives another key.
    let (again, _) = pair_over(link_pair(), "482913", Some("482913"));
    assert_ne!(again.unwrap().1, phone_key);
}

#[test]
fn a_wrong_code_pairs_nobody() {
    let (phone, mac) = pair_over(link_pair(), "482913", Some("482914"));
    assert_eq!(phone.unwrap_err(), Refusal::WrongCode);
    assert_eq!(mac.unwrap_err(), Refusal::WrongCode);
    let (phone, mac) = pair_over(link_pair(), "482913", None);
    assert_eq!(phone.unwrap_err(), Refusal::NoCode);
    assert_eq!(mac.unwrap_err(), Refusal::NoCode);
}

#[test]
fn the_code_and_the_key_never_cross_the_wire() {
    let seen = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let (phone_link, mac_link) = link_pair();
    let (a, b) = (seen.clone(), seen.clone());
    let links = (
        phone_link.tapped(move |m| {
            a.lock().unwrap().push(m.clone());
            vec![m]
        }),
        mac_link.tapped(move |m| {
            b.lock().unwrap().push(m.clone());
            vec![m]
        }),
    );
    let (phone, _) = pair_over(links, "482913", Some("482913"));
    let key = phone.unwrap().1;
    let wire: Vec<u8> = seen.lock().unwrap().concat();
    let contains = |needle: &[u8]| wire.windows(needle.len()).any(|w| w == needle);
    assert!(!contains(b"482913"));
    assert!(!contains(&key));
    assert!(
        contains(b"Finns MacBook Pro"),
        "names are sent in the clear"
    );
}

#[test]
fn a_changed_pairing_message_fails_the_pairing() {
    for target in [0x03u8, 0x04] {
        let (phone_link, mac_link) = link_pair();
        let flip = move |mut m: Vec<u8>| {
            if m[0] == target {
                m[5] ^= 0x40;
            }
            vec![m]
        };
        let links = (phone_link.tapped(flip), mac_link.tapped(flip));
        let (phone, mac) = pair_over(links, "482913", Some("482913"));
        assert!(phone.is_err(), "message {target:#x}");
        assert!(mac.is_err(), "message {target:#x}");
    }
}

#[test]
fn another_protocol_version_is_refused_with_its_own_reason() {
    for (theirs, expected) in [
        (secure::VERSION + 1, Refusal::PeerNewer),
        (secure::VERSION - 1, Refusal::PeerOlder),
    ] {
        for opening in [0x01u8, 0x10] {
            let (phone_link, mac_link) = link_pair();
            let mut message = vec![opening];
            // A later version may change everything but `v`.
            message.extend_from_slice(
                serde_json::json!({ "v": theirs, "something": "new" })
                    .to_string()
                    .as_bytes(),
            );
            assert!(phone_link.send(&message));
            assert_eq!(first(&mac_link).err(), Some(expected));
            // The phone is told why and which version the Mac speaks, so it
            // can say which app to update.
            let mut said = vec![0x0f, 6];
            said.extend_from_slice(&secure::VERSION.to_be_bytes());
            assert_eq!(phone_link.recv(Duration::from_secs(2)), Recv::Message(said));
        }
    }
}

/// The phone's side hears the Mac's refusal and knows which app is behind.
#[test]
fn a_phone_learns_which_app_needs_the_update() {
    for (mac_version, expected) in [
        (secure::VERSION + 1, Refusal::PeerNewer),
        (secure::VERSION - 1, Refusal::PeerOlder),
    ] {
        let (phone_link, mac_link) = link_pair();
        let mac_thread = std::thread::spawn(move || {
            let _ = mac_link.recv(Duration::from_secs(2));
            let mut fail = vec![0x0f, 6];
            fail.extend_from_slice(&mac_version.to_be_bytes());
            mac_link.send(&fail);
        });
        let me = Identity {
            device_id: "phone-0123456789abcdef".into(),
            name: "iPhone".into(),
            platform: "ios".into(),
        };
        let pairing = pair_as_phone(&phone_link, &me, |_| Some("000000".into()));
        assert_eq!(pairing.err(), Some(expected));
        mac_thread.join().unwrap();

        // A session start says the same.
        let (phone_link, mac_link) = link_pair();
        let mac_thread = std::thread::spawn(move || {
            let _ = mac_link.recv(Duration::from_secs(2));
            let mut fail = vec![0x0f, 6];
            fail.extend_from_slice(&mac_version.to_be_bytes());
            mac_link.send(&fail);
        });
        let session = connect(phone_link, "phone-id-0001", "mac-id-0001", &[7u8; 32]);
        assert_eq!(session.err(), Some(expected));
        mac_thread.join().unwrap();
    }
    // A first-version Mac says only "another version".
    let (phone_link, mac_link) = link_pair();
    let mac_thread = std::thread::spawn(move || {
        let _ = mac_link.recv(Duration::from_secs(2));
        mac_link.send(&[0x0f, 6]);
    });
    let session = connect(phone_link, "phone-id-0001", "mac-id-0001", &[7u8; 32]);
    assert_eq!(session.err(), Some(Refusal::Version));
    mac_thread.join().unwrap();
}

// ── Sessions ────────────────────────────────────────────────────────────

fn open_channels(
    links: (MemLink, MemLink),
    phone_key: PairKey,
    mac_key: PairKey,
) -> (
    Result<Channel<MemLink>, Refusal>,
    Result<Channel<MemLink>, Refusal>,
) {
    let (phone_link, mac_link) = links;
    let mac_thread = std::thread::spawn(move || {
        let First::Hello(hello) = first(&mac_link)? else {
            return Err(Refusal::Protocol);
        };
        assert_eq!(hello.device_id, "phone-id-0001");
        accept(mac_link, "mac-id-0001", &hello, &mac_key)
    });
    let phone = connect(phone_link, "phone-id-0001", "mac-id-0001", &phone_key);
    (phone, mac_thread.join().unwrap())
}

#[test]
fn a_session_needs_the_pairs_key_on_both_sides() {
    let (phone, mac) = open_channels(link_pair(), pair_key(), pair_key());
    let (mut phone, mut mac) = (phone.unwrap(), mac.unwrap());
    assert!(phone.send(b"hello mac"));
    assert_eq!(
        mac.recv(Duration::from_secs(2)),
        Recv::Message(b"hello mac".to_vec())
    );
    assert!(mac.send(b"hello phone"));
    assert_eq!(
        phone.recv(Duration::from_secs(2)),
        Recv::Message(b"hello phone".to_vec())
    );

    // A stranger who knows the ids but not the key gets nowhere, either way.
    let (phone, mac) = open_channels(link_pair(), [9u8; 32], pair_key());
    assert_eq!(phone.err(), Some(Refusal::BadKey));
    assert!(mac.is_err());
}

#[test]
fn sealed_messages_cannot_be_read_changed_repeated_or_reordered() {
    // Read: the plaintext is not on the wire.
    let seen = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let (phone_link, mac_link) = link_pair();
    let log = seen.clone();
    let phone_link = phone_link.tapped(move |m| {
        log.lock().unwrap().push(m.clone());
        vec![m]
    });
    let (phone, mac) = open_channels((phone_link, mac_link), pair_key(), pair_key());
    let (mut phone, mut mac) = (phone.unwrap(), mac.unwrap());
    assert!(phone.send(b"the word is Kubernetes"));
    assert!(matches!(mac.recv(Duration::from_secs(2)), Recv::Message(_)));
    let wire: Vec<u8> = seen.lock().unwrap().concat();
    assert!(!wire.windows(10).any(|w| w == b"Kubernetes"));

    // Changed: one flipped bit closes the session.
    let (phone_link, mac_link) = link_pair();
    let phone_link = phone_link.tapped(|mut m| {
        if m[0] == 0x20 {
            let last = m.len() - 1;
            m[last] ^= 1;
        }
        vec![m]
    });
    let (phone, mac) = open_channels((phone_link, mac_link), pair_key(), pair_key());
    let (mut phone, mut mac) = (phone.unwrap(), mac.unwrap());
    assert!(phone.send(b"one"));
    assert_eq!(mac.recv(Duration::from_secs(2)), Recv::Closed);

    // Repeated: the same sealed message a second time closes it.
    let (phone_link, mac_link) = link_pair();
    let phone_link = phone_link.tapped(|m| {
        if m[0] == 0x20 {
            vec![m.clone(), m]
        } else {
            vec![m]
        }
    });
    let (phone, mac) = open_channels((phone_link, mac_link), pair_key(), pair_key());
    let (mut phone, mut mac) = (phone.unwrap(), mac.unwrap());
    assert!(phone.send(b"one"));
    assert_eq!(
        mac.recv(Duration::from_secs(2)),
        Recv::Message(b"one".to_vec())
    );
    assert_eq!(mac.recv(Duration::from_secs(2)), Recv::Closed);

    // Reordered: the second message first closes it.
    let held = Arc::new(Mutex::new(None::<Vec<u8>>));
    let (phone_link, mac_link) = link_pair();
    let phone_link = phone_link.tapped(move |m| {
        if m[0] != 0x20 {
            return vec![m];
        }
        match held.lock().unwrap().replace(m.clone()) {
            None => vec![],
            Some(earlier) => vec![m, earlier],
        }
    });
    let (phone, mac) = open_channels((phone_link, mac_link), pair_key(), pair_key());
    let (mut phone, mut mac) = (phone.unwrap(), mac.unwrap());
    assert!(phone.send(b"one"));
    assert!(phone.send(b"two"));
    assert_eq!(mac.recv(Duration::from_secs(2)), Recv::Closed);
}

#[test]
fn a_recorded_session_cannot_be_replayed() {
    // Record everything the phone sends in one session …
    let recorded = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let (phone_link, mac_link) = link_pair();
    let log = recorded.clone();
    let phone_link = phone_link.tapped(move |m| {
        log.lock().unwrap().push(m.clone());
        vec![m]
    });
    let (phone, mac) = open_channels((phone_link, mac_link), pair_key(), pair_key());
    let (mut phone, mut mac) = (phone.unwrap(), mac.unwrap());
    assert!(phone.send(b"delete everything"));
    assert!(matches!(mac.recv(Duration::from_secs(2)), Recv::Message(_)));

    // … and play it to the Mac again, as an attacker without the key.
    let (attacker, mac_link) = link_pair();
    let mac_thread = std::thread::spawn(move || {
        let First::Hello(hello) = first(&mac_link)? else {
            return Err(Refusal::Protocol);
        };
        let mut channel = accept(mac_link, "mac-id-0001", &hello, &pair_key())?;
        Ok(channel.recv(Duration::from_secs(2)))
    });
    for message in recorded.lock().unwrap().iter() {
        attacker.send(message);
    }
    let outcome = mac_thread.join().unwrap();
    assert!(
        matches!(outcome, Err(Refusal::BadKey)),
        "the Mac's fresh nonce makes the old proof worthless: {outcome:?}"
    );
}

// ── The exchange ────────────────────────────────────────────────────────

#[test]
fn two_paired_devices_stay_in_step_while_both_change() {
    let (phone, mac) = (side("iPhone"), side("Finns MacBook Pro"));
    mac.device.word("Siobhán", past(1));
    mac.device.entry("gonna", Some("going to"), past(2));
    mac.device.snippet("sig", "Best wishes", past(3));
    mac.device.take("2026-11-08", 300, 120_000);
    phone.device.word("Kubernetes", past(4));
    phone.device.take("2026-11-09", 40, 30_000);

    let running = run_session(&phone, &mac, link_pair(), (pair_key(), pair_key()));
    converge(&phone, &mac, || phone.device.total_words() == 340);
    assert_eq!(mac.device.total_words(), 340);
    assert_eq!(
        phone.device.words(),
        ["Kubernetes", "Siobhán", "gonna → going to"]
    );

    // Changes on both sides while connected, with no one asking for a sync.
    phone.device.word("Zeta", later(1));
    phone.device.delete_entry("Siobhán");
    phone.device.snippet("sig", "Cheers", later(2));
    phone.device.take("2026-11-10", 60, 30_000);
    mac.device.entry("gonna", Some("gonna be"), later(3));
    mac.device.snippet("addr", "1 Example Street", later(4));
    mac.device.setting("greeting_name", json!("Finn"), later(5));
    mac.device.take("2026-11-10", 100, 60_000);
    converge(&phone, &mac, || {
        phone.device.total_words() == 500 && mac.device.total_words() == 500
    });
    assert_eq!(
        mac.device.words(),
        ["Kubernetes", "Zeta", "gonna → gonna be"]
    );
    assert_eq!(
        mac.device.snippet_bodies(),
        [
            ("addr".to_owned(), "1 Example Street".to_owned()),
            ("sig".to_owned(), "Cheers".to_owned())
        ]
    );

    // Once in step the two fall silent: no merges without a change.
    std::thread::sleep(session::POLL * 2);
    let quiet = (*phone.merges.lock().unwrap(), *mac.merges.lock().unwrap());
    std::thread::sleep(session::POLL * 2);
    assert_eq!(
        quiet,
        (*phone.merges.lock().unwrap(), *mac.merges.lock().unwrap())
    );
    // Whichever end stops first closes the link under the other.
    for end in running.stop() {
        assert!(matches!(end, Some(End::Stopped | End::Closed)), "{end:?}");
    }
}

#[test]
fn a_session_cut_in_the_middle_is_made_up_by_the_next() {
    let (phone, mac) = (side("iPhone"), side("Mac"));
    for i in 0..40 {
        mac.device.word(&format!("Word{i}"), past(i));
    }
    phone.device.word("Kubernetes", past(50));
    // The link dies with the first state the Mac sends: the phone's state
    // arrives, the Mac's never does.
    let (phone_link, mac_link) = link_pair();
    let dead = mac_link.closed.clone();
    let mac_link = mac_link.tapped(move |m| {
        if m[0] == 0x20 {
            dead.store(true, Ordering::Release);
            return vec![];
        }
        vec![m]
    });
    let running = run_session(
        &phone,
        &mac,
        (phone_link, mac_link),
        (pair_key(), pair_key()),
    );
    let ends: Vec<_> = running
        .threads
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect();
    assert_eq!(ends, [Some(End::Closed), Some(End::Closed)]);
    assert_ne!(phone.device.state(), mac.device.state());

    // The next session starts from both full states.
    let running = run_session(&phone, &mac, link_pair(), (pair_key(), pair_key()));
    converge(&phone, &mac, || phone.device.words().len() == 41);
    running.stop();
}

#[test]
fn sync_now_sends_again_and_unpair_reaches_the_other_side() {
    let (phone, mac) = (side("iPhone"), side("Mac"));
    mac.device.word("Alpha", past(1));
    let running = run_session(&phone, &mac, link_pair(), (pair_key(), pair_key()));
    converge(&phone, &mac, || phone.device.words() == ["Alpha"]);
    let before = *mac.merges.lock().unwrap();
    running.controls[0].now.store(true, Ordering::Release);
    let deadline = Instant::now() + Duration::from_secs(5);
    while *mac.merges.lock().unwrap() == before {
        assert!(Instant::now() < deadline, "Sync now sent nothing");
        std::thread::sleep(Duration::from_millis(50));
    }
    running.controls[0].unpair.store(true, Ordering::Release);
    let ends: Vec<_> = running
        .threads
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect();
    assert_eq!(ends, [Some(End::Stopped), Some(End::Unpaired)]);
}

#[test]
fn a_state_from_someone_else_than_the_paired_device_ends_the_session() {
    // The phone's channel is right, but it sends another device's state.
    struct Liar(Arc<Side>, Arc<Side>);
    impl Host for Liar {
        fn collect(&self) -> Option<(Manifest, Payload)> {
            self.1.collect()
        }
        fn merge(&self, m: Manifest, p: Payload) -> bool {
            self.0.merge(m, p)
        }
    }
    let (phone, mac, other) = (side("iPhone"), side("Mac"), side("Other"));
    other.device.word("Planted", past(1));
    let (phone_link, mac_link) = link_pair();
    let (phone_id, mac_id) = (phone.device.id(), mac.device.id());
    let mac_side = mac.clone();
    let mac_thread = std::thread::spawn(move || {
        let Ok(First::Hello(hello)) = first(&mac_link) else {
            panic!()
        };
        let peer = hello.device_id.clone();
        let mut channel = accept(mac_link, &mac_side.device.id(), &hello, &pair_key()).unwrap();
        session::run(&mut channel, &*mac_side, &peer, &Control::default())
    });
    let mut channel = connect(phone_link, &phone_id, &mac_id, &pair_key()).unwrap();
    let liar = Liar(phone.clone(), other);
    let _ = session::run(&mut channel, &liar, &mac_id, &Control::default());
    assert_eq!(mac_thread.join().unwrap(), End::Closed);
    assert!(mac.device.words().is_empty());
}

// ── Pairings on disk ────────────────────────────────────────────────────

#[test]
fn pairings_are_kept_outside_the_database_and_private() {
    let dir = tempfile::tempdir().unwrap();
    let who = Identity {
        device_id: "mac-0123456789abcdef".into(),
        name: "Finns MacBook Pro".into(),
        platform: "macos".into(),
    };
    let mut peers = Peers::load(dir.path());
    assert!(!peers.enabled());
    assert!(peers.all().is_empty());
    peers.add(&who, &pair_key(), 1_000);
    peers.synced(&who.device_id, 2_000);
    let again = Peers::load(dir.path());
    assert!(again.enabled(), "pairing turns sync on");
    assert_eq!(again.get(&who.device_id).unwrap().key(), Some(pair_key()));
    assert_eq!(again.all()[0].last_sync_at, Some(2_000));
    let file = dir.path().join(super::super::peers::FILE_NAME);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    // "Export my data" takes fork.db and the two settings files, never this.
    let export = include_str!("../export.rs");
    assert!(!export.contains("sync-peers"));
    let mut peers = again;
    assert!(peers.remove(&who.device_id));
    assert!(Peers::load(dir.path()).all().is_empty());
    // A broken file is "no pairings", not a crash.
    std::fs::write(&file, "{").unwrap();
    assert!(Peers::load(dir.path()).all().is_empty());
}

#[test]
fn a_macs_service_name_carries_its_id_and_its_name() {
    use super::super::auto::{parse_service, service_name};
    let me = Identity {
        device_id: "ab12cd34ef567890ab12cd34ef567890".into(),
        name: "Finns MacBook Pro".into(),
        platform: "macos".into(),
    };
    let service = service_name(&me);
    assert_eq!(service, "ab12cd34 Finns MacBook Pro");
    assert_eq!(
        parse_service(&service),
        Some(("ab12cd34", "Finns MacBook Pro"))
    );
    assert_eq!(parse_service("Someone's printer"), None);
    let long = Identity {
        name: "É".repeat(60),
        ..me
    };
    assert!(service_name(&long).len() <= 63);
}

// ── The real local network ──────────────────────────────────────────────

/// Advertise, find, connect, pair and sync through Network.framework on
/// this Mac: the transport the apps use, with both ends in one process.
#[test]
fn pairing_and_a_session_over_the_real_local_network() {
    use super::super::net::{self, Event};

    let (phone, mac) = (side("iPhone"), side("Test Mac"));
    mac.device.word("Siobhán", past(1));
    phone.device.word("Kubernetes", past(2));
    let service = format!("{} Test Mac", &mac.device.id()[..8]);

    let events = net::events();
    net::advertise(&service);
    net::browse();
    let mut incoming = Vec::new();
    let mut found = false;
    let deadline = Instant::now() + Duration::from_secs(20);
    while !found {
        match events.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::Found(name)) if name == service => found = true,
            Ok(Event::Incoming(link)) => incoming.push(link),
            Ok(Event::Listener(state)) | Ok(Event::Browser(state)) => {
                assert_ne!(state, "denied", "local network access refused");
            }
            _ => {}
        }
        assert!(Instant::now() < deadline, "the service was not found");
    }
    let next_incoming = |incoming: &mut Vec<net::NativeLink>| loop {
        if let Some(link) = incoming.pop() {
            return link;
        }
        match events.recv_timeout(Duration::from_secs(10)) {
            Ok(Event::Incoming(link)) => return link,
            Ok(_) => {}
            Err(e) => panic!("no incoming connection: {e}"),
        }
    };

    // Pair.
    let phone_link =
        net::connect(&service, Duration::from_secs(10), Duration::from_secs(10)).expect("connect");
    let mac_link = next_incoming(&mut incoming);
    let (mac_who, phone_who) = (mac.device.identity("macos"), phone.device.identity("ios"));
    let mac_thread = std::thread::spawn(move || {
        let Ok(First::Pair(who)) = first(&mac_link) else {
            panic!("not a pairing request")
        };
        // As the engine does: the last message, then close at once. The
        // close must not overtake it (the owner's first real pairing
        // failed on the phone exactly so, with the Mac already paired).
        let key = pair_as_mac(&mac_link, &mac_who, &who, "271828");
        mac_link.close();
        key
    });
    let (who, phone_key) =
        pair_as_phone(&phone_link, &phone_who, |_| Some("271828".into())).unwrap();
    let mac_key = mac_thread.join().unwrap().unwrap();
    assert_eq!(who.device_id, mac.device.id());
    assert_eq!(phone_key, mac_key);
    drop(phone_link);

    // A session on a new connection, with the key from the pairing.
    let phone_link = net::connect(&service, Duration::from_secs(10), Duration::from_secs(10))
        .expect("reconnect");
    let mac_link = next_incoming(&mut incoming);
    let running = run_session(&phone, &mac, (phone_link, mac_link), (phone_key, mac_key));
    converge(&phone, &mac, || phone.device.words().len() == 2);
    mac.device.word("Epsilon", later(1));
    converge(&phone, &mac, || phone.device.words().len() == 3);
    running.stop();
    net::stop_browse();
    net::stop_advertise();
}
