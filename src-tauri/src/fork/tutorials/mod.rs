// SPDX-License-Identifier: GPL-3.0-or-later
//! Lessons (S-31): the backend half of the in-app tutorials. The lessons
//! themselves (scenes, practice, success detection) are data in
//! `src/fork/tutorials/`; the backend only routes a feature's own shortcut
//! to an open practice step ([`practice`]).

pub mod practice;
