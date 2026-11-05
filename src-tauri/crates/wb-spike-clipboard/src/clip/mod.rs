// SPDX-License-Identifier: GPL-3.0-or-later
//! The product's `fork/clipboard/` (R5 §4 W-07), compiled into this tool
//! through `#[path]` so RS-05 and the S-04 soak test exactly the shipped
//! code. S-04 lifted these files out of this crate:
//!
//! - `snapshot.rs`: snapshot data, skip policy, byte comparison (pure);
//! - `restore.rs`: restore guard and wait rule (pure; ports hex + Handy);
//! - `markers.rs`: nspasteboard.org markers and the manager view (pure);
//! - `target.rs`: native / Chromium / Electron and the 155/156 check (hex);
//! - `keys.rs`: ⌘V, typing, modifier-release wait (hex + Handy);
//! - `nspb.rs`: objc2 `NSPasteboard` read/write and the lazy transcript.
//!
//! Only `carbon.rs` (hex's Carbon snapshot, kept to compare against; RS-05
//! showed it adds types on restore) stays here.
//!
//! `crate::take` strings them together the way the RS-05 matrix needs;
//! `crate::soak` drives the product's `fork::insert::engine` instead.

// Product API this tool does not all use.
#![allow(dead_code)]

pub mod carbon;
#[path = "../../../../src/fork/clipboard/keys.rs"]
pub mod keys;
#[path = "../../../../src/fork/clipboard/markers.rs"]
pub mod markers;
#[path = "../../../../src/fork/clipboard/nspb.rs"]
pub mod nspb;
#[path = "../../../../src/fork/clipboard/restore.rs"]
pub mod restore;
#[path = "../../../../src/fork/clipboard/snapshot.rs"]
pub mod snapshot;
#[path = "../../../../src/fork/clipboard/target.rs"]
pub mod target;
