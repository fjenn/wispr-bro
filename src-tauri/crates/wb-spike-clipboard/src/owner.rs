// SPDX-License-Identifier: GPL-3.0-or-later
//! The owner's own clipboard: saved at the start, put back at the end, and
//! on Control+C, a closed Terminal window (SIGHUP), SIGTERM or a panic.
//!
//! The saved copy also goes to `owner-clipboard.wbclip` (mode 0600) in the
//! run's results folder, deleted once the restore is verified, so even a
//! crash or `kill -9` leaves a way back: the next run offers it, or
//! `wb-spike-clipboard --restore-owner FILE`. Its contents are never logged.

use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use crate::clip::nspb;
use crate::clip::snapshot::{self, Snapshot};

pub const BACKUP_NAME: &str = "owner-clipboard.wbclip";

static INTERRUPTED: AtomicBool = AtomicBool::new(false);
static SAVED: Mutex<Option<(Snapshot, PathBuf)>> = Mutex::new(None);

pub fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct OwnerRestore {
    pub identical: bool,
    pub items: usize,
    pub types: usize,
    pub detail: String,
}

/// Saves the clipboard as it is now.
pub fn save(dir: &Path) -> Result<(usize, usize), String> {
    let (snap, _) = nspb::read();
    let path = dir.join(BACKUP_NAME);
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(&snapshot::encode(&snap))
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())?;
    let counts = (snap.items.len(), snap.type_count());
    if let Ok(mut s) = SAVED.lock() {
        *s = Some((snap, path));
    }
    Ok(counts)
}

/// Writes `snap` back and checks it byte for byte.
pub fn put_back(snap: &Snapshot) -> OwnerRestore {
    let written = nspb::write(snap);
    let (after, _) = nspb::read();
    let fid = snapshot::compare(snap, &after);
    // The owner's clipboard is restored whole: nothing is "skipped by
    // policy" here, so a skipped type is a loss too.
    let identical = written.is_ok() && fid.byte_identical() && fid.skipped() == 0;
    OwnerRestore {
        identical,
        items: snap.items.len(),
        types: snap.type_count(),
        detail: match written {
            Err(e) => e,
            Ok(_) => fid.describe(),
        },
    }
}

/// Restores the saved clipboard (once). Deletes the backup file if the
/// restore is byte-identical.
pub fn restore() -> Option<OwnerRestore> {
    let saved = SAVED.lock().ok()?.take()?;
    let r = put_back(&saved.0);
    if r.identical {
        let _ = fs::remove_file(&saved.1);
    }
    Some(r)
}

pub fn restore_file(path: &Path) -> Result<OwnerRestore, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let snap = snapshot::decode(&bytes)?;
    let r = put_back(&snap);
    if r.identical {
        let _ = fs::remove_file(path);
    }
    Ok(r)
}

/// Backups left by runs that did not finish (newest first).
pub fn leftover_backups(results_root: &Path, except: Option<&Path>) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(results_root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|d| d.path().join(BACKUP_NAME))
        .filter(|p| p.exists() && Some(p.parent().unwrap_or(p)) != except)
        .collect();
    v.sort();
    v.reverse();
    v
}

/// Control+C, SIGTERM, SIGHUP: flag the run as interrupted and wake the UI
/// so the main thread cleans up (it owns the run loop and the input
/// source). If it has not exited within 3 s, restore from here and exit.
pub fn install_signals(wake: impl Fn() + Send + 'static) {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    let Ok(mut signals) = signal_hook::iterator::Signals::new([SIGINT, SIGTERM, SIGHUP]) else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("wb-rs05-signals".into())
        .spawn(move || {
            if signals.forever().next().is_some() {
                INTERRUPTED.store(true, Ordering::SeqCst);
                wake();
                std::thread::sleep(Duration::from_secs(3));
                if let Some(r) = restore() {
                    eprintln!(
                        "\nStopped. Your clipboard was put back ({}).",
                        if r.identical {
                            "exactly"
                        } else {
                            "not exactly; see the results"
                        }
                    );
                }
                std::process::exit(130);
            }
        });
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore();
        prev(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leftovers_skip_the_current_run() {
        let root = std::env::temp_dir().join(format!("wb-rs05-owner-{}", std::process::id()));
        for d in ["20261030-100000", "20261030-110000", "20261030-120000"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        fs::write(root.join("20261030-100000").join(BACKUP_NAME), b"x").unwrap();
        fs::write(root.join("20261030-120000").join(BACKUP_NAME), b"x").unwrap();
        let cur = root.join("20261030-120000");
        let v = leftover_backups(&root, Some(&cur));
        assert_eq!(v, vec![root.join("20261030-100000").join(BACKUP_NAME)]);
        assert_eq!(leftover_backups(&root, None).len(), 2);
        fs::remove_dir_all(&root).unwrap();
    }
}
