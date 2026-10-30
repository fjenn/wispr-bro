// SPDX-License-Identifier: GPL-3.0-or-later
//! Prototype of the fork's AX layer (R5 §4.6, W-05; S-05 lifts this into
//! `src-tauri/src/fork/ax/{mod,thread,element,lease,editable,markers}.rs`).
//!
//! - `element`: AX call wrappers and app helpers (SpeakoFlow port, MIT).
//! - `thread`: the one AX thread with per-call messaging timeouts.
//! - `lease`: `AXManualAccessibility` per-PID refcount (VoiceInk port, GPL).
//! - `editable`: `is_editable` + fail-open delivery rule (OpenWhispr port, MIT).
//! - `markers`: selected text through text markers (VoiceInk port, GPL).
//! - `probe`: the timed full read RS-04 records per focus event.
//!
//! Never written: `AXEnhancedUserInterface`. Never read: secure fields.

pub mod editable;
pub mod element;
pub mod lease;
pub mod markers;
pub mod probe;
pub mod thread;
