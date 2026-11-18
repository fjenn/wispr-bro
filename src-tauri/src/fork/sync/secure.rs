// SPDX-License-Identifier: GPL-3.0-or-later
//! Everything that keeps a sync connection private, over any [`Link`] that
//! carries whole messages (the local network in the app, a pair of queues in
//! the tests). The transport knows no keys.
//!
//! **Pairing, once.** The Mac shows a 6-digit code; the phone's user types
//! it. Both run SPAKE2 on it (`spake2`, RustCrypto): the code never crosses
//! the wire, someone listening learns nothing, and someone answering in the
//! Mac's place gets one guess per code. Each side proves it derived the
//! same key, and that key (HKDF) becomes the pair's long-term 256-bit key.
//!
//! ```text
//!   phone                                         Mac
//!   PairRequest {id, name}        ─────────►      makes a code, shows it
//!                                 ◄─────────      PairReady {id, name}
//!   user types the code
//!   PairA (SPAKE2 message)        ─────────►
//!                                 ◄─────────      PairB (SPAKE2 message, proof)
//!   checks the proof; PairConfirm ─────────►      checks the proof
//!                                 ◄─────────      PairDone
//! ```
//!
//! **Every later session.** Both send a fresh random nonce and prove they
//! hold the pair's key; the session's two keys come from the pair's key and
//! both nonces (HKDF), so a recorded session cannot be replayed. Messages
//! are then sealed with ChaCha20-Poly1305 under a per-direction counter: a
//! changed, repeated, dropped or reordered message fails to open and ends
//! the session.
//!
//! ```text
//!   Hello {id, nonce}             ─────────►      looks up the phone's key
//!                                 ◄─────────      HelloReply {id, nonce, proof}
//!   HelloProof                    ─────────►
//!   Sealed …                      ◄────────►      Sealed …
//! ```

use std::time::Duration;

use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305};
use ring::hkdf::{KeyType, Salt, HKDF_SHA256};
use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use spake2::{Ed25519Group, Identity as SpakeIdentity, Password, Spake2};

/// Protocol version; a peer with another one is refused.
pub const VERSION: u32 = 1;
/// Protocol minor version: what a state may carry beyond version 1's, told
/// in each state's manifest ([`super::Manifest::minor`]). A peer with an
/// older minor syncs as before and skips what it cannot read (serde
/// ignores unknown fields). 1: Scratchpad notes.
pub const MINOR: u32 = 1;
/// The pair's long-term key.
pub type PairKey = [u8; 32];

/// What a link gave back.
#[derive(Debug, PartialEq, Eq)]
pub enum Recv {
    Message(Vec<u8>),
    Timeout,
    Closed,
}

/// A connection that carries whole messages, in order.
pub trait Link: Send {
    /// `false` when the link is closed.
    fn send(&self, bytes: &[u8]) -> bool;
    fn recv(&self, timeout: Duration) -> Recv;
    fn close(&self);
}

/// A device as it names itself to the other.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub device_id: String,
    pub name: String,
    /// `macos` or `ios`.
    pub platform: String,
}

/// Why a pairing or a session did not come about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The other device was never reached.
    Unreachable,
    /// The link closed or stayed silent.
    Closed,
    /// The other side sent something this version does not understand.
    Protocol,
    /// The other side runs another version of the sync protocol and did
    /// not say which: one of the two apps needs an update.
    Version,
    /// The other side runs an older version of the sync protocol: update
    /// the app on the other device.
    PeerOlder,
    /// The other side runs a newer version of the sync protocol: update the
    /// app on this device.
    PeerNewer,
    /// The code typed on the phone is not the code the Mac showed.
    WrongCode,
    /// Nobody typed a code in time, or pairing was cancelled.
    NoCode,
    /// The Mac is in the middle of another pairing.
    Busy,
    /// The Mac does not know this phone (unpaired there).
    Unknown,
    /// The other side does not hold the pair's key.
    BadKey,
}

impl Refusal {
    /// One of the two apps needs an update.
    pub fn is_version(self) -> bool {
        matches!(
            self,
            Refusal::Version | Refusal::PeerOlder | Refusal::PeerNewer
        )
    }
}

/// The refusal for a peer that speaks protocol version `theirs`.
fn version_refusal(theirs: u32) -> Refusal {
    match theirs.cmp(&VERSION) {
        std::cmp::Ordering::Less => Refusal::PeerOlder,
        std::cmp::Ordering::Greater => Refusal::PeerNewer,
        std::cmp::Ordering::Equal => Refusal::Protocol,
    }
}

mod frame {
    pub const PAIR_REQUEST: u8 = 0x01;
    pub const PAIR_READY: u8 = 0x02;
    pub const PAIR_A: u8 = 0x03;
    pub const PAIR_B: u8 = 0x04;
    pub const PAIR_CONFIRM: u8 = 0x05;
    pub const PAIR_DONE: u8 = 0x06;
    pub const FAIL: u8 = 0x0f;
    pub const HELLO: u8 = 0x10;
    pub const HELLO_REPLY: u8 = 0x11;
    pub const HELLO_PROOF: u8 = 0x12;
    pub const SEALED: u8 = 0x20;
}

const FAIL_WRONG_CODE: u8 = 1;
const FAIL_BUSY: u8 = 2;
const FAIL_UNKNOWN: u8 = 3;
const FAIL_PROTOCOL: u8 = 4;
const FAIL_NO_CODE: u8 = 5;
const FAIL_VERSION: u8 = 6;

/// How long one step of a handshake may take.
const STEP: Duration = Duration::from_secs(15);
/// How long the user has to read and type the code.
pub const CODE_WAIT: Duration = Duration::from_secs(120);

fn random<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    // The system generator failing is not a state to continue from.
    SystemRandom::new()
        .fill(&mut out)
        .expect("system random numbers");
    out
}

/// A fresh 6-digit pairing code.
pub fn new_code() -> String {
    let n = u32::from_le_bytes(random::<4>()) % 1_000_000;
    format!("{n:06}")
}

struct Len(usize);

impl KeyType for Len {
    fn len(&self) -> usize {
        self.0
    }
}

fn derive(salt: &[u8], key: &[u8], label: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    Salt::new(HKDF_SHA256, salt)
        .extract(key)
        .expand(&[label], Len(32))
        .and_then(|okm| okm.fill(&mut out))
        .expect("HKDF of 32 bytes");
    out
}

fn proof(key: &[u8], label: &[u8]) -> Vec<u8> {
    hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, key), label)
        .as_ref()
        .to_vec()
}

fn proof_ok(key: &[u8], label: &[u8], given: &[u8]) -> bool {
    hmac::verify(&hmac::Key::new(hmac::HMAC_SHA256, key), label, given).is_ok()
}

fn send(link: &dyn Link, kind: u8, body: &[u8]) -> Result<(), Refusal> {
    let mut message = Vec::with_capacity(body.len() + 1);
    message.push(kind);
    message.extend_from_slice(body);
    if link.send(&message) {
        Ok(())
    } else {
        Err(Refusal::Closed)
    }
}

fn fail(link: &dyn Link, code: u8) {
    if code == FAIL_VERSION {
        // The version refusal carries this side's version, so the other
        // side can tell its user which app to update.
        let mut message = vec![frame::FAIL, code];
        message.extend_from_slice(&VERSION.to_be_bytes());
        let _ = link.send(&message);
    } else {
        let _ = link.send(&[frame::FAIL, code]);
    }
}

/// A `FAIL` message's body (after the frame byte).
fn refusal_of(body: &[u8]) -> Refusal {
    match body.first() {
        Some(&FAIL_WRONG_CODE) => Refusal::WrongCode,
        Some(&FAIL_BUSY) => Refusal::Busy,
        Some(&FAIL_UNKNOWN) => Refusal::Unknown,
        Some(&FAIL_NO_CODE) => Refusal::NoCode,
        Some(&FAIL_VERSION) => match body.get(1..5).and_then(|b| b.try_into().ok()) {
            Some(theirs) => version_refusal(u32::from_be_bytes(theirs)),
            // A first-version app says no more than "another version".
            None => Refusal::Version,
        },
        _ => Refusal::Protocol,
    }
}

/// The next message, which must be of `kind`; its body.
fn expect(link: &dyn Link, kind: u8, wait: Duration) -> Result<Vec<u8>, Refusal> {
    match link.recv(wait) {
        Recv::Message(m) if m.first() == Some(&kind) => Ok(m[1..].to_vec()),
        Recv::Message(m) if m.first() == Some(&frame::FAIL) => Err(refusal_of(&m[1..])),
        Recv::Message(_) => Err(Refusal::Protocol),
        Recv::Timeout | Recv::Closed => Err(Refusal::Closed),
    }
}

#[derive(Serialize, Deserialize)]
struct Greeting {
    v: u32,
    #[serde(flatten)]
    who: Identity,
}

fn greeting(me: &Identity) -> Vec<u8> {
    serde_json::to_vec(&Greeting {
        v: VERSION,
        who: me.clone(),
    })
    .unwrap_or_default()
}

/// Only the version of a first message, read before anything else in it,
/// so a later version may change the rest and still be told apart.
#[derive(Deserialize)]
struct Versioned {
    v: u32,
}

fn check_version(body: &[u8]) -> Result<(), Refusal> {
    let Versioned { v } = serde_json::from_slice(body).map_err(|_| Refusal::Protocol)?;
    if v == VERSION {
        Ok(())
    } else {
        Err(version_refusal(v))
    }
}

fn read_greeting(body: &[u8]) -> Result<Identity, Refusal> {
    check_version(body)?;
    let g: Greeting = serde_json::from_slice(body).map_err(|_| Refusal::Protocol)?;
    let id_ok = (8..=64).contains(&g.who.device_id.len())
        && g.who
            .device_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if g.v != VERSION {
        return Err(version_refusal(g.v));
    }
    if !id_ok {
        return Err(Refusal::Protocol);
    }
    Ok(Identity {
        name: g
            .who
            .name
            .chars()
            .filter(|c| !c.is_control())
            .take(64)
            .collect(),
        platform: g.who.platform.chars().take(16).collect(),
        ..g.who
    })
}

fn spake(code: &str, phone: &str, mac: &str, phone_side: bool) -> (Spake2<Ed25519Group>, Vec<u8>) {
    let password = Password::new(code.as_bytes());
    let a = SpakeIdentity::new(phone.as_bytes());
    let b = SpakeIdentity::new(mac.as_bytes());
    if phone_side {
        Spake2::<Ed25519Group>::start_a(&password, &a, &b)
    } else {
        Spake2::<Ed25519Group>::start_b(&password, &a, &b)
    }
}

const MAC_CONFIRM: &[u8] = b"wb-sync pairing: mac";
const PHONE_CONFIRM: &[u8] = b"wb-sync pairing: phone";
const PAIR_KEY: &[u8] = b"wb-sync pairing: key";

/// The phone's side of a pairing. `code` is called once the Mac shows its
/// code and returns what the user typed (`None`: cancelled).
pub fn pair_as_phone(
    link: &dyn Link,
    me: &Identity,
    code: impl FnOnce(&Identity) -> Option<String>,
) -> Result<(Identity, PairKey), Refusal> {
    send(link, frame::PAIR_REQUEST, &greeting(me))?;
    let mac = read_greeting(&expect(link, frame::PAIR_READY, STEP)?)?;
    let Some(code) = code(&mac) else {
        fail(link, FAIL_NO_CODE);
        return Err(Refusal::NoCode);
    };
    let (state, message) = spake(code.trim(), &me.device_id, &mac.device_id, true);
    send(link, frame::PAIR_A, &message)?;
    let reply = expect(link, frame::PAIR_B, STEP)?;
    if reply.len() != 33 + 32 {
        return Err(Refusal::Protocol);
    }
    let (theirs, confirm) = reply.split_at(33);
    let shared = state.finish(theirs).map_err(|_| Refusal::Protocol)?;
    if !proof_ok(&shared, MAC_CONFIRM, confirm) {
        return Err(Refusal::WrongCode);
    }
    send(link, frame::PAIR_CONFIRM, &proof(&shared, PHONE_CONFIRM))?;
    expect(link, frame::PAIR_DONE, STEP)?;
    Ok((mac, derive(&[], &shared, PAIR_KEY)))
}

/// The Mac's side of a pairing with `phone` (from [`first`]), whose user
/// is shown `code`.
pub fn pair_as_mac(
    link: &dyn Link,
    me: &Identity,
    phone: &Identity,
    code: &str,
) -> Result<PairKey, Refusal> {
    send(link, frame::PAIR_READY, &greeting(me))?;
    let message = match expect(link, frame::PAIR_A, CODE_WAIT) {
        Ok(m) => m,
        Err(Refusal::Closed) => return Err(Refusal::NoCode),
        Err(e) => return Err(e),
    };
    let (state, mine) = spake(code, &phone.device_id, &me.device_id, false);
    let shared = match state.finish(&message) {
        Ok(shared) => shared,
        Err(_) => {
            fail(link, FAIL_PROTOCOL);
            return Err(Refusal::Protocol);
        }
    };
    let mut reply = mine;
    reply.extend_from_slice(&proof(&shared, MAC_CONFIRM));
    send(link, frame::PAIR_B, &reply)?;
    // A phone with the wrong code cannot make this proof; it hangs up.
    let confirm = expect(link, frame::PAIR_CONFIRM, STEP).map_err(|e| match e {
        Refusal::Closed => Refusal::WrongCode,
        e => e,
    })?;
    if !proof_ok(&shared, PHONE_CONFIRM, &confirm) {
        fail(link, FAIL_WRONG_CODE);
        return Err(Refusal::WrongCode);
    }
    send(link, frame::PAIR_DONE, &[])?;
    Ok(derive(&[], &shared, PAIR_KEY))
}

/// What a connecting phone opened with.
pub enum First {
    /// It wants to pair.
    Pair(Identity),
    /// It is paired and starts a session.
    Hello(Hello),
}

#[derive(Serialize, Deserialize)]
pub struct Hello {
    v: u32,
    pub device_id: String,
    nonce: [u8; 32],
}

/// The Mac's read of a new connection's first message.
pub fn first(link: &dyn Link) -> Result<First, Refusal> {
    match link.recv(STEP) {
        Recv::Message(m) if m.first() == Some(&frame::PAIR_REQUEST) => {
            let who = read_greeting(&m[1..]);
            if who.as_ref().is_err_and(|e| e.is_version()) {
                fail(link, FAIL_VERSION);
            }
            who.map(First::Pair)
        }
        Recv::Message(m) if m.first() == Some(&frame::HELLO) => {
            if let Err(why) = check_version(&m[1..]) {
                if why.is_version() {
                    fail(link, FAIL_VERSION);
                }
                return Err(why);
            }
            let hello: Hello = serde_json::from_slice(&m[1..]).map_err(|_| Refusal::Protocol)?;
            Ok(First::Hello(hello))
        }
        Recv::Message(_) => Err(Refusal::Protocol),
        Recv::Timeout | Recv::Closed => Err(Refusal::Closed),
    }
}

/// Tells a phone why it is turned away (`Busy`, `Unknown`).
pub fn refuse(link: &dyn Link, why: Refusal) {
    fail(
        link,
        match why {
            Refusal::Busy => FAIL_BUSY,
            Refusal::Unknown => FAIL_UNKNOWN,
            Refusal::WrongCode => FAIL_WRONG_CODE,
            Refusal::NoCode => FAIL_NO_CODE,
            Refusal::Version | Refusal::PeerOlder | Refusal::PeerNewer => FAIL_VERSION,
            _ => FAIL_PROTOCOL,
        },
    );
}

#[derive(Serialize, Deserialize)]
struct HelloReply {
    device_id: String,
    nonce: [u8; 32],
    proof: Vec<u8>,
}

const MAC_PROOF: &[u8] = b"wb-sync session: mac";
const PHONE_PROOF: &[u8] = b"wb-sync session: phone";
const TO_MAC: &[u8] = b"wb-sync session: to mac";
const TO_PHONE: &[u8] = b"wb-sync session: to phone";

fn session_salt(phone: &[u8; 32], mac: &[u8; 32], phone_id: &str, mac_id: &str) -> Vec<u8> {
    let mut salt = Vec::with_capacity(64 + phone_id.len() + mac_id.len() + 1);
    salt.extend_from_slice(phone);
    salt.extend_from_slice(mac);
    salt.extend_from_slice(phone_id.as_bytes());
    salt.push(0);
    salt.extend_from_slice(mac_id.as_bytes());
    salt
}

/// The phone's side of a session start with the Mac `mac_id`.
pub fn connect<L: Link>(
    link: L,
    my_id: &str,
    mac_id: &str,
    key: &PairKey,
) -> Result<Channel<L>, Refusal> {
    let nonce = random::<32>();
    let hello = Hello {
        v: VERSION,
        device_id: my_id.to_owned(),
        nonce,
    };
    send(
        &link,
        frame::HELLO,
        &serde_json::to_vec(&hello).unwrap_or_default(),
    )?;
    let reply: HelloReply = serde_json::from_slice(&expect(&link, frame::HELLO_REPLY, STEP)?)
        .map_err(|_| Refusal::Protocol)?;
    if reply.device_id != mac_id {
        return Err(Refusal::BadKey);
    }
    let salt = session_salt(&nonce, &reply.nonce, my_id, mac_id);
    let session = derive(&salt, key, b"wb-sync session");
    if !proof_ok(&session, MAC_PROOF, &reply.proof) {
        return Err(Refusal::BadKey);
    }
    send(&link, frame::HELLO_PROOF, &proof(&session, PHONE_PROOF))?;
    Ok(Channel::new(
        link,
        &derive(&[], &session, TO_MAC),
        &derive(&[], &session, TO_PHONE),
    ))
}

/// The Mac's side of a session start, after [`first`] gave `hello` and the
/// phone's key was found.
pub fn accept<L: Link>(
    link: L,
    my_id: &str,
    hello: &Hello,
    key: &PairKey,
) -> Result<Channel<L>, Refusal> {
    let nonce = random::<32>();
    let salt = session_salt(&hello.nonce, &nonce, &hello.device_id, my_id);
    let session = derive(&salt, key, b"wb-sync session");
    let reply = HelloReply {
        device_id: my_id.to_owned(),
        nonce,
        proof: proof(&session, MAC_PROOF),
    };
    send(
        &link,
        frame::HELLO_REPLY,
        &serde_json::to_vec(&reply).unwrap_or_default(),
    )?;
    let given = expect(&link, frame::HELLO_PROOF, STEP)?;
    if !proof_ok(&session, PHONE_PROOF, &given) {
        fail(&link, FAIL_PROTOCOL);
        return Err(Refusal::BadKey);
    }
    Ok(Channel::new(
        link,
        &derive(&[], &session, TO_PHONE),
        &derive(&[], &session, TO_MAC),
    ))
}

/// A session: messages sealed in both directions.
pub struct Channel<L: Link> {
    link: L,
    seal: LessSafeKey,
    open: LessSafeKey,
    sent: u64,
    received: u64,
}

fn aead(key: &[u8; 32]) -> LessSafeKey {
    LessSafeKey::new(UnboundKey::new(&CHACHA20_POLY1305, key).expect("a 32-byte key"))
}

fn counter(n: u64) -> Nonce {
    let mut bytes = [0u8; 12];
    bytes[4..].copy_from_slice(&n.to_be_bytes());
    Nonce::assume_unique_for_key(bytes)
}

impl<L: Link> Channel<L> {
    fn new(link: L, seal: &[u8; 32], open: &[u8; 32]) -> Self {
        Self {
            link,
            seal: aead(seal),
            open: aead(open),
            sent: 0,
            received: 0,
        }
    }

    /// Seals and sends `plain`. `false`: the link is closed.
    pub fn send(&mut self, plain: &[u8]) -> bool {
        let mut message = Vec::with_capacity(plain.len() + 17);
        message.extend_from_slice(plain);
        if self
            .seal
            .seal_in_place_append_tag(counter(self.sent), Aad::empty(), &mut message)
            .is_err()
        {
            return false;
        }
        self.sent += 1;
        message.insert(0, frame::SEALED);
        self.link.send(&message)
    }

    /// The next message, opened. Anything that does not open with the next
    /// counter (changed, repeated, out of order) closes the session.
    pub fn recv(&mut self, timeout: Duration) -> Recv {
        match self.link.recv(timeout) {
            Recv::Message(mut m) if m.first() == Some(&frame::SEALED) => {
                m.remove(0);
                match self
                    .open
                    .open_in_place(counter(self.received), Aad::empty(), &mut m)
                {
                    Ok(plain) => {
                        self.received += 1;
                        Recv::Message(plain.to_vec())
                    }
                    Err(_) => {
                        self.link.close();
                        Recv::Closed
                    }
                }
            }
            Recv::Message(_) => {
                self.link.close();
                Recv::Closed
            }
            other => other,
        }
    }

    pub fn close(&self) {
        self.link.close();
    }
}
