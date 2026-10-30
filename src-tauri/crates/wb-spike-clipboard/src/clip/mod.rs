// SPDX-License-Identifier: GPL-3.0-or-later
//! Prototype of `fork/clipboard/` (R5 §4 W-07, S-04). S-04 lifts these files:
//!
//! - `snapshot.rs`: snapshot data, skip policy, byte comparison (pure);
//! - `restore.rs`: restore guard and wait rule (pure; ports hex + Handy);
//! - `markers.rs`: nspasteboard.org markers and the manager view (pure);
//! - `target.rs`: native / Chromium / Electron and the 155/156 check (hex);
//! - `keys.rs`: ⌘V, typing, modifier-release wait (hex + Handy);
//! - `nspb.rs`: objc2 `NSPasteboard` read/write and the lazy transcript;
//! - `carbon.rs`: hex's Carbon snapshot, kept to compare against.
//!
//! `crate::take` strings them together into one paste, the way
//! `fork::insert::deliver` will.

pub mod carbon;
pub mod keys;
pub mod markers;
pub mod nspb;
pub mod restore;
pub mod snapshot;
pub mod target;
