// SPDX-License-Identifier: GPL-3.0-or-later
//! The product's delivery engine (`fork/insert/`, S-04), compiled from the
//! app's sources so `crate::soak` drives exactly the shipped code. The files
//! reach the clipboard layer as `super::super::clipboard` (`crate::clipboard`
//! here, an alias of `crate::clip`).

// Product API this tool does not all use.
#![allow(dead_code)]

#[path = "../../../../src/fork/insert/deliver.rs"]
pub mod deliver;
#[path = "../../../../src/fork/insert/session.rs"]
pub mod session;
#[path = "../../../../src/fork/insert/verify.rs"]
pub mod verify;
