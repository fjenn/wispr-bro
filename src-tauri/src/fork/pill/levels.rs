// SPDX-License-Identifier: GPL-3.0-or-later
//! Mic levels for the waveform (R5 §4.9).
//!
//! Handy's recorder reports 16 visualiser buckets (low to high frequency)
//! from every audio callback, far faster than a screen refreshes. The pill
//! takes the first [`BARS`] buckets (where speech is, the same choice as
//! Handy's own overlay), clamps them, and sends at most one frame every
//! [`FRAME_MS`] to the `fork-pill` web view only (`emit_to`), and only
//! while that view is showing a take. Each frame drives WebKit work, so
//! fewer frames are less memory churn (Handy issue #1279, wry #1489).

use std::sync::atomic::{AtomicU64, Ordering};

use super::geometry::BARS;

/// Minimum spacing between two frames (~30 fps).
pub const FRAME_MS: u64 = 33;

/// The waveform bars for one frame of levels.
pub fn bars(levels: &[f32]) -> Vec<f32> {
    (0..BARS)
        .map(|i| {
            let v = levels.get(i).copied().unwrap_or(0.0);
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
        .collect()
}

/// A lock-free frame limiter shared by every audio callback.
#[derive(Debug, Default)]
pub struct Throttle {
    last_ms: AtomicU64,
}

impl Throttle {
    pub const fn new() -> Self {
        Self {
            last_ms: AtomicU64::new(0),
        }
    }

    /// `true` when a frame may go out at `now_ms` (and records it).
    pub fn admit(&self, now_ms: u64) -> bool {
        let last = self.last_ms.load(Ordering::Relaxed);
        if last != 0 && now_ms.saturating_sub(last) < FRAME_MS {
            return false;
        }
        self.last_ms
            .compare_exchange(last, now_ms.max(1), Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_take_the_speech_buckets() {
        let levels: Vec<f32> = (0..16).map(|i| i as f32 / 10.0).collect();
        let b = bars(&levels);
        assert_eq!(b.len(), BARS);
        assert_eq!(b[0], 0.0);
        assert_eq!(b[5], 0.5);
        // Clamped to 0..1.
        assert_eq!(b[BARS - 1], 0.8);
        assert_eq!(bars(&[2.0, -1.0, f32::NAN])[..3], [1.0, 0.0, 0.0]);
        // Short input pads with silence.
        assert_eq!(bars(&[]), vec![0.0; BARS]);
    }

    #[test]
    fn throttle_admits_about_30_fps() {
        let t = Throttle::new();
        let admitted = (0..1000u64).filter(|ms| t.admit(10_000 + ms)).count();
        // One per 33 ms over one second.
        assert!((30..=31).contains(&admitted), "{admitted}");
        // A clock at 0 still admits its first frame.
        let t = Throttle::new();
        assert!(t.admit(0));
        assert!(!t.admit(10));
        assert!(t.admit(40));
    }
}
