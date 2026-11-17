// SPDX-License-Identifier: GPL-3.0-or-later
//! One sync session between two paired devices, the same on both sides:
//! each sends its state ([`store::collect`]) when it differs from what it
//! last sent, and merges what the other sends ([`merge::merge`]).
//!
//! ```text
//!   every POLL: my state changed since I last sent it?  ── State ──►  merge, Ack
//!   a State arrives: merge it (idempotent), Ack           ◄── Ack ───
//! ```
//!
//! "Changed" is a hash of the payload, read from the database every
//! [`POLL`], so nothing in the app has to announce a change: a new word, an
//! edited snippet, a setting or a take's stats are all seen within a few
//! seconds. A merge that changes this device's own state makes it send once
//! more; the other side merges that with no change and the exchange stops.
//!
//! A session that breaks at any point loses nothing: the next one starts
//! with both full states, and merging one twice changes nothing.
//!
//! [`store::collect`]: super::store::collect
//! [`merge::merge`]: super::merge::merge

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::secure::{Channel, Link, Recv};
use super::{Manifest, Payload};

/// How often a device looks whether its state changed.
pub const POLL: Duration = Duration::from_secs(2);
/// A silent link gets a ping after this long …
const PING_AFTER: Duration = Duration::from_secs(15);
/// … and is given up after this long without any message.
const SILENT_AFTER: Duration = Duration::from_secs(50);
/// How long an unpairing side waits for the other to hang up.
const UNPAIR_WAIT: Duration = Duration::from_secs(3);
/// How long one wait for a message lasts (and so how fast `stop` is seen).
const TICK: Duration = Duration::from_millis(250);

/// What the session needs from the device it runs on.
pub trait Host: Send + Sync {
    /// This device's state as a sync file carries it.
    fn collect(&self) -> Option<(Manifest, Payload)>;
    /// Merges the other device's state. `false`: it could not be merged.
    fn merge(&self, manifest: Manifest, payload: Payload) -> bool;
    /// A state went out and was acknowledged, or one came in and was merged.
    fn synced(&self, _peer_id: &str) {}
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)] // one message at a time; a state is the usual one
enum Message {
    State {
        manifest: Manifest,
        payload: Payload,
    },
    Ack,
    Ping,
    /// The sender removed this pairing; the receiver does the same.
    Unpair,
}

/// What the owner of a session can ask of it while it runs.
#[derive(Default)]
pub struct Control {
    /// End the session.
    pub stop: AtomicBool,
    /// Tell the other side the pairing is over, then end.
    pub unpair: AtomicBool,
    /// Send the state now, changed or not ("Sync now").
    pub now: AtomicBool,
}

/// How a session ended.
#[derive(Debug, PartialEq, Eq)]
pub enum End {
    /// The link closed, went silent or carried something unreadable.
    Closed,
    /// `Control::stop` or `Control::unpair`.
    Stopped,
    /// The other side removed the pairing.
    Unpaired,
}

fn digest(payload: &Payload) -> Vec<u8> {
    let bytes = serde_json::to_vec(payload).unwrap_or_default();
    ring::digest::digest(&ring::digest::SHA256, &bytes)
        .as_ref()
        .to_vec()
}

fn send<L: Link>(channel: &mut Channel<L>, message: &Message) -> bool {
    serde_json::to_vec(message).is_ok_and(|bytes| channel.send(&bytes))
}

/// Runs the session with `peer_id` until it ends.
pub fn run<L: Link>(
    channel: &mut Channel<L>,
    host: &dyn Host,
    peer_id: &str,
    control: &Control,
) -> End {
    let mut sent: Option<Vec<u8>> = None;
    let mut last_sent = Instant::now();
    let mut last_heard = Instant::now();
    let mut next_look = Instant::now();
    loop {
        if control.unpair.load(Ordering::Acquire) {
            if send(channel, &Message::Unpair) {
                // Closing at once could overtake the message: wait for the
                // other side to hang up on reading it.
                let until = Instant::now() + UNPAIR_WAIT;
                while Instant::now() < until && channel.recv(TICK) != Recv::Closed {}
            }
            channel.close();
            return End::Stopped;
        }
        if control.stop.load(Ordering::Acquire) {
            channel.close();
            return End::Stopped;
        }
        let now = control.now.swap(false, Ordering::AcqRel);
        if now || Instant::now() >= next_look {
            next_look = Instant::now() + POLL;
            if let Some((manifest, payload)) = host.collect() {
                let hash = digest(&payload);
                if now || sent.as_ref() != Some(&hash) {
                    if !send(channel, &Message::State { manifest, payload }) {
                        return End::Closed;
                    }
                    sent = Some(hash);
                    last_sent = Instant::now();
                }
            }
        }
        if last_sent.elapsed() >= PING_AFTER {
            if !send(channel, &Message::Ping) {
                return End::Closed;
            }
            last_sent = Instant::now();
        }
        match channel.recv(TICK) {
            Recv::Timeout => {
                if last_heard.elapsed() >= SILENT_AFTER {
                    channel.close();
                    return End::Closed;
                }
            }
            Recv::Closed => return End::Closed,
            Recv::Message(bytes) => {
                last_heard = Instant::now();
                match serde_json::from_slice::<Message>(&bytes) {
                    Ok(Message::State { manifest, payload }) => {
                        if manifest.device_id != peer_id || !host.merge(manifest, payload) {
                            channel.close();
                            return End::Closed;
                        }
                        host.synced(peer_id);
                        if !send(channel, &Message::Ack) {
                            return End::Closed;
                        }
                        last_sent = Instant::now();
                        // What was merged may have changed what this side sends.
                        next_look = Instant::now();
                    }
                    Ok(Message::Ack) => host.synced(peer_id),
                    Ok(Message::Ping) => {}
                    Ok(Message::Unpair) => {
                        channel.close();
                        return End::Unpaired;
                    }
                    Err(_) => {
                        channel.close();
                        return End::Closed;
                    }
                }
            }
        }
    }
}
