// SPDX-License-Identifier: GPL-3.0-or-later
//! The devices this one is paired with, and their keys.
//!
//! One small file in the app's data folder (`sync-peers.json`, readable by
//! the user only), not `fork.db`: "Export my data" copies the database, and
//! a pairing key must never travel in an export. On iPhone the app's
//! container is encrypted by the system until the first unlock.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use super::secure::{Identity, PairKey};

pub const FILE_NAME: &str = "sync-peers.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    /// The pair's 256-bit key, base64.
    key: String,
    /// Unix ms.
    pub paired_at: i64,
    /// When the two last exchanged their state (Unix ms).
    #[serde(default)]
    pub last_sync_at: Option<i64>,
}

impl Peer {
    pub fn key(&self) -> Option<PairKey> {
        base64::engine::general_purpose::STANDARD
            .decode(&self.key)
            .ok()?
            .try_into()
            .ok()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Stored {
    /// Automatic sync is on (the Mac's switch; on once a phone is paired).
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    peers: Vec<Peer>,
}

/// The file, loaded.
#[derive(Debug)]
pub struct Peers {
    path: PathBuf,
    stored: Stored,
}

impl Peers {
    /// Reads `<dir>/sync-peers.json`; a missing or unreadable file is "no
    /// pairings".
    pub fn load(dir: &Path) -> Self {
        let path = dir.join(FILE_NAME);
        let stored = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { path, stored }
    }

    fn save(&self) {
        let Ok(bytes) = serde_json::to_vec_pretty(&self.stored) else {
            return;
        };
        let temporary = self.path.with_extension("json.tmp");
        let written = (|| {
            #[cfg(unix)]
            {
                use std::io::Write;
                use std::os::unix::fs::OpenOptionsExt;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .open(&temporary)?;
                file.write_all(&bytes)?;
            }
            #[cfg(not(unix))]
            std::fs::write(&temporary, &bytes)?;
            std::fs::rename(&temporary, &self.path)
        })();
        if let Err(e) = written {
            log::warn!("sync: could not save the pairings: {e}");
        }
    }

    pub fn enabled(&self) -> bool {
        self.stored.enabled
    }

    pub fn set_enabled(&mut self, on: bool) {
        if self.stored.enabled != on {
            self.stored.enabled = on;
            self.save();
        }
    }

    pub fn all(&self) -> &[Peer] {
        &self.stored.peers
    }

    pub fn get(&self, device_id: &str) -> Option<&Peer> {
        self.stored.peers.iter().find(|p| p.device_id == device_id)
    }

    /// Adds (or replaces) a pairing and turns automatic sync on.
    pub fn add(&mut self, who: &Identity, key: &PairKey, now_ms: i64) {
        self.stored.peers.retain(|p| p.device_id != who.device_id);
        self.stored.peers.push(Peer {
            device_id: who.device_id.clone(),
            name: who.name.clone(),
            platform: who.platform.clone(),
            key: base64::engine::general_purpose::STANDARD.encode(key),
            paired_at: now_ms,
            last_sync_at: None,
        });
        self.stored.enabled = true;
        self.save();
    }

    /// Removes a pairing and its key. `true` if there was one.
    pub fn remove(&mut self, device_id: &str) -> bool {
        let before = self.stored.peers.len();
        self.stored.peers.retain(|p| p.device_id != device_id);
        let removed = self.stored.peers.len() != before;
        if removed {
            self.save();
        }
        removed
    }

    /// Records an exchange with `device_id`. Written to disk at most once a
    /// minute per device: the time on screen comes from memory.
    pub fn synced(&mut self, device_id: &str, now_ms: i64) {
        let Some(peer) = self
            .stored
            .peers
            .iter_mut()
            .find(|p| p.device_id == device_id)
        else {
            return;
        };
        let stale = peer.last_sync_at.is_none_or(|at| now_ms - at >= 60_000);
        peer.last_sync_at = Some(now_ms);
        if stale {
            self.save();
        }
    }
}
