// SPDX-License-Identifier: GPL-3.0-or-later
//! S-01 tests: the grammar (scripted RS-01 scenarios, ported upstream
//! vectors, unit tests, take limits), the raw-event decisions, the blocking
//! set, the bindings, the labels, the action registry and the engine with a
//! fake coordinator.

mod actions;
mod bindings;
mod blocking;
mod decision;
mod engine;
mod labels;
mod limits;
mod ported;
mod processor;
pub mod scenarios;
