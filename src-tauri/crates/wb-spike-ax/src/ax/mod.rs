// SPDX-License-Identifier: GPL-3.0-or-later
//! The fork's AX layer (R5 §4.6, W-05) as this tool uses it. S-05
//! (2026-10-31) lifted the prototype into `src-tauri/src/fork/ax/`; the
//! product files are compiled here through `#[path]`, so an RS-04 re-run
//! scores exactly the shipped verdict:
//!
//! - `element`: AX call wrappers and app helpers (SpeakoFlow port, MIT).
//! - `lease`: `AXManualAccessibility` per-PID refcount (VoiceInk port, GPL).
//! - `editable`: `is_editable` + fail-open delivery rule (OpenWhispr port, MIT).
//! - `markers`: selected text through text markers (VoiceInk port, GPL).
//!
//! Only the tool's own parts stay here:
//!
//! - `thread`: the AX thread with Ctrl+C handling and the tool's lease probes.
//! - `probe`: the timed full read RS-04 records per focus event.
//!
//! Never written: `AXEnhancedUserInterface`. Never read: secure fields.

// Product API this tool does not all use.
#![allow(dead_code)]

#[path = "../../../../src/fork/ax/editable.rs"]
pub mod editable;
#[path = "../../../../src/fork/ax/element.rs"]
pub mod element;
#[path = "../../../../src/fork/ax/lease.rs"]
pub mod lease;
#[path = "../../../../src/fork/ax/markers.rs"]
pub mod markers;
pub mod probe;
pub mod thread;
