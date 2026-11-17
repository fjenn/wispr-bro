// SPDX-License-Identifier: GPL-3.0-or-later
//! The local network as the sync engine sees it (`net.m`, Network.framework,
//! the same on the Mac and the iPhone): advertise, browse, connect, and
//! whole messages on a connection.
//!
//! This is the only place the app talks to another device, and only to one
//! on the same local link (Bonjour, peer-to-peer included): there is no
//! host name to resolve and no server. What is sent is sealed in
//! [`super::secure`] before it gets here.

use std::collections::HashMap;
use std::ffi::{c_char, CString};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::secure::{Link, Recv};

extern "C" {
    fn wb_sync_net_set_handler(handler: extern "C" fn(i32, u64, *const u8, usize));
    fn wb_sync_net_advertise(name: *const c_char);
    fn wb_sync_net_stop_advertise();
    fn wb_sync_net_browse();
    fn wb_sync_net_stop_browse();
    fn wb_sync_net_connect(conn: u64, name: *const c_char);
    fn wb_sync_net_send(conn: u64, bytes: *const u8, length: usize);
    fn wb_sync_net_close(conn: u64);
}

/// What the network reports besides messages.
#[derive(Debug)]
pub enum Event {
    /// A device that advertises appeared (its service name).
    Found(String),
    Lost(String),
    /// Another device connected to this one.
    Incoming(NativeLink),
    /// `ready`, `failed`, `denied` (local network access refused), `stopped`.
    Listener(String),
    Browser(String),
}

/// What an outgoing connection reports before it is usable.
enum Dial {
    Ready,
    Closed,
    /// Waiting for a path, or failed: `"<state> <domain> <code>"`.
    Trouble(String),
}

/// Why [`connect`] gave up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectFail {
    /// The system refuses this app the local network (the user said no to
    /// "find devices on your local network", or has not answered yet).
    Denied,
    /// Not reached: the other app is gone, or the network does not carry it.
    Unreachable,
}

/// `kDNSServiceErr_PolicyDenied`: local network access is not allowed.
const POLICY_DENIED: &str = "dns -65570";

enum Route {
    /// An outgoing connection that is not ready yet.
    Connecting(Sender<Dial>, Sender<Vec<u8>>),
    Open(Sender<Vec<u8>>),
}

struct Net {
    routes: HashMap<u64, Route>,
    events: Option<Sender<Event>>,
    next: u64,
}

fn net() -> &'static Mutex<Net> {
    static NET: OnceLock<Mutex<Net>> = OnceLock::new();
    NET.get_or_init(|| {
        // SAFETY: the handler is a plain function that lives as long as the
        // process.
        unsafe { wb_sync_net_set_handler(on_event) };
        Mutex::new(Net {
            routes: HashMap::new(),
            events: None,
            next: 1,
        })
    })
}

fn lock() -> std::sync::MutexGuard<'static, Net> {
    net().lock().unwrap_or_else(|e| e.into_inner())
}

extern "C" fn on_event(kind: i32, conn: u64, bytes: *const u8, length: usize) {
    let data = if bytes.is_null() || length == 0 {
        Vec::new()
    } else {
        // SAFETY: `net.m` passes `length` readable bytes that stay valid for
        // the call.
        unsafe { std::slice::from_raw_parts(bytes, length) }.to_vec()
    };
    let text = || String::from_utf8_lossy(&data).into_owned();
    let mut net = lock();
    let event = match kind {
        1 => Some(Event::Found(text())),
        2 => Some(Event::Lost(text())),
        3 => {
            let (tx, rx) = channel();
            net.routes.insert(conn, Route::Open(tx));
            Some(Event::Incoming(NativeLink {
                conn,
                inbox: Mutex::new(rx),
            }))
        }
        4 => {
            if let Some(Route::Connecting(ready, tx)) = net.routes.remove(&conn) {
                let _ = ready.send(Dial::Ready);
                net.routes.insert(conn, Route::Open(tx));
            }
            None
        }
        5 => {
            if let Some(Route::Open(tx)) = net.routes.get(&conn) {
                let _ = tx.send(data);
            }
            None
        }
        6 => {
            // Dropping the sender is how the link's reader learns it closed.
            if let Some(Route::Connecting(ready, _)) = net.routes.remove(&conn) {
                let _ = ready.send(Dial::Closed);
            }
            None
        }
        9 => {
            let why = text();
            match net.routes.get(&conn) {
                Some(Route::Connecting(ready, _)) => {
                    let _ = ready.send(Dial::Trouble(why));
                }
                _ => log::info!("sync: connection {conn}: {why}"),
            }
            None
        }
        7 => Some(Event::Listener(text())),
        8 => Some(Event::Browser(text())),
        _ => None,
    };
    if let (Some(event), Some(events)) = (event, net.events.as_ref()) {
        let _ = events.send(event);
    }
}

/// Everything but messages arrives here. One listener at a time.
pub fn events() -> Receiver<Event> {
    let (tx, rx) = channel();
    lock().events = Some(tx);
    rx
}

fn c_string(text: &str) -> CString {
    CString::new(text.replace('\0', "")).unwrap_or_default()
}

/// Advertises this device under `name` and accepts connections.
pub fn advertise(name: &str) {
    let _ = net();
    let name = c_string(name);
    // SAFETY: a valid NUL-terminated string, copied by the callee.
    unsafe { wb_sync_net_advertise(name.as_ptr()) }
}

pub fn stop_advertise() {
    // SAFETY: no arguments.
    unsafe { wb_sync_net_stop_advertise() }
}

/// Looks for devices that advertise ([`Event::Found`], [`Event::Lost`]).
pub fn browse() {
    let _ = net();
    // SAFETY: no arguments.
    unsafe { wb_sync_net_browse() }
}

pub fn stop_browse() {
    // SAFETY: no arguments.
    unsafe { wb_sync_net_stop_browse() }
}

/// Connects to the advertised service `name` within `wait`.
///
/// A connection the system holds back because local network access is not
/// (yet) allowed is given `denied_wait` instead: iOS and macOS ask the user
/// the first time, and Network.framework goes on by itself once they allow
/// it, so a pairing survives that question.
pub fn connect(
    name: &str,
    wait: Duration,
    denied_wait: Duration,
) -> Result<NativeLink, ConnectFail> {
    let (ready_tx, ready_rx) = channel();
    let (tx, rx) = channel();
    let conn = {
        let mut net = lock();
        let conn = net.next;
        net.next += 1;
        net.routes.insert(conn, Route::Connecting(ready_tx, tx));
        conn
    };
    let c_name = c_string(name);
    // SAFETY: a valid NUL-terminated string, copied by the callee.
    unsafe { wb_sync_net_connect(conn, c_name.as_ptr()) }
    let link = NativeLink {
        conn,
        inbox: Mutex::new(rx),
    };
    let started = Instant::now();
    let mut denied = false;
    loop {
        let limit = if denied { denied_wait.max(wait) } else { wait };
        let left = limit.saturating_sub(started.elapsed());
        match ready_rx.recv_timeout(left) {
            Ok(Dial::Ready) => {
                if denied {
                    log::info!("sync: local network access was allowed; connected");
                }
                return Ok(link);
            }
            Ok(Dial::Trouble(why)) => {
                log::info!("sync: connecting: {why}");
                if why.ends_with(POLICY_DENIED) && !denied {
                    denied = true;
                    log::info!("sync: waiting for local network access");
                }
            }
            Ok(Dial::Closed) | Err(_) => {
                log::info!(
                    "sync: could not connect after {} ms{}",
                    started.elapsed().as_millis(),
                    if denied {
                        " (local network access refused)"
                    } else {
                        ""
                    }
                );
                link.close();
                return Err(if denied {
                    ConnectFail::Denied
                } else {
                    ConnectFail::Unreachable
                });
            }
        }
    }
}

/// One connection.
#[derive(Debug)]
pub struct NativeLink {
    conn: u64,
    inbox: Mutex<Receiver<Vec<u8>>>,
}

impl Link for NativeLink {
    fn send(&self, bytes: &[u8]) -> bool {
        if !lock().routes.contains_key(&self.conn) {
            return false;
        }
        // SAFETY: `bytes` is valid for the call; the callee copies it.
        unsafe { wb_sync_net_send(self.conn, bytes.as_ptr(), bytes.len()) }
        true
    }

    fn recv(&self, timeout: Duration) -> Recv {
        let inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
        match inbox.recv_timeout(timeout) {
            Ok(message) => Recv::Message(message),
            Err(RecvTimeoutError::Timeout) => Recv::Timeout,
            Err(RecvTimeoutError::Disconnected) => Recv::Closed,
        }
    }

    fn close(&self) {
        lock().routes.remove(&self.conn);
        // SAFETY: an id; closing an unknown one does nothing.
        unsafe { wb_sync_net_close(self.conn) }
    }
}

impl Drop for NativeLink {
    fn drop(&mut self) {
        self.close();
    }
}
