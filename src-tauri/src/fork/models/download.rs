// SPDX-License-Identifier: GPL-3.0-or-later
//! The speech-model download as a pure state machine (S-07a, R5 §5.4).
//!
//! The bytes move through Handy's own downloader
//! (`ModelManager::download_model`: hf-hub into the shared cache, resume
//! from its `.sync.part`, four attempts with a stall watchdog). This
//! machine is what the fork adds around it, so a slow, flaky or hostile
//! network never ends in a corrupt model or a silent stall:
//!
//! - **Disk first.** Free space on the cache volume is checked before every
//!   transfer (file size + [`DISK_MARGIN`]); too little → `NoSpace`.
//! - **Retry with backoff.** After a failed transfer the pinned URL is
//!   probed once ([`super::probe`]): a 429/503 `Retry-After` is honoured
//!   (capped at [`RETRY_AFTER_CAP`]), otherwise [`backoff`]; after
//!   [`MAX_TRANSFERS`] transfers the state is "Couldn't download · Retry".
//! - **Captive portals.** A probe answered with a web page, or redirected
//!   off the allowed hosts, means a Wi-Fi login page: "check your
//!   connection", retried every [`PORTAL_WAIT`] in case the user logs in.
//! - **Never a corrupt model.** Handy's Hugging Face path does not check
//!   hashes, so the file is sha256-checked against the catalog pin after
//!   every transfer; a mismatch deletes it and downloads once more, a second
//!   mismatch fails.
//! - **Retry and wake** restart from the top with a fresh attempt count.
//!
//! The driver (`fork::models::setup`) performs each [`Step`] and feeds the
//! result back as an [`Event`].

use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;

/// Free space kept on top of the file (the cache, the OS, recordings).
pub const DISK_MARGIN: u64 = 256 * 1024 * 1024;
/// Transfers before giving up (each is up to four hf-hub attempts).
pub const MAX_TRANSFERS: u32 = 6;
/// The longest `Retry-After` honoured.
pub const RETRY_AFTER_CAP: Duration = Duration::from_secs(600);
/// How often a captive portal is re-tried.
pub const PORTAL_WAIT: Duration = Duration::from_secs(30);

/// Wait before transfer `n + 1` after transfer `n` failed (1-based):
/// 5, 15, 30, 60, 120 s.
pub fn backoff(failed_transfers: u32) -> Duration {
    const STEPS: [u64; 5] = [5, 15, 30, 60, 120];
    let i = (failed_transfers.max(1) as usize - 1).min(STEPS.len() - 1);
    Duration::from_secs(STEPS[i])
}

/// What the probe of the pinned URL found after a failed transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probe {
    /// No connection (DNS, connect, TLS, timeout).
    Offline,
    /// A web page instead of the file, or a redirect off the allowed hosts.
    CaptivePortal,
    /// HTTP 429 or 503, with the server's `Retry-After` if it sent one.
    RateLimited { retry_after: Option<Duration> },
    /// Another HTTP error.
    ServerError,
    /// The file is reachable: the transfer itself failed (a reset, a stall).
    Reachable,
}

/// Why setup is waiting or stopped (shown to the user, no model names).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SetupProblem {
    Offline,
    /// A Wi-Fi login page (captive portal) or a proxy is in the way.
    CheckConnection,
    /// Hugging Face answered with an error or kept failing.
    Server,
    NoSpace,
    /// The file did not match its pinned checksum twice.
    Corrupt,
}

impl Probe {
    fn problem(self) -> SetupProblem {
        match self {
            Probe::Offline => SetupProblem::Offline,
            Probe::CaptivePortal => SetupProblem::CheckConnection,
            Probe::RateLimited { .. } | Probe::ServerError | Probe::Reachable => {
                SetupProblem::Server
            }
        }
    }
}

/// What the driver does next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Read the free space on the cache volume → [`Event::DiskFree`].
    CheckDisk,
    /// Run Handy's download → `TransferDone` / `TransferFailed` /
    /// `TransferCancelled`.
    Transfer,
    /// sha256 the file → [`Event::Verified`].
    Verify,
    /// Delete the file (bad checksum) → [`Event::Deleted`].
    Delete,
    /// Sleep, then → [`Event::Waited`]. `problem` is what the user is told
    /// meanwhile (`None`: just "setting up").
    Wait {
        delay: Duration,
        problem: Option<SetupProblem>,
    },
    /// The file is verified: select the model when no take runs.
    Ready,
    /// Stopped until Retry or a wake.
    Failed(SetupProblem),
    /// Cancelled (another model was picked); the driver exits.
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    DiskFree(Option<u64>),
    TransferDone,
    TransferFailed(Probe),
    TransferCancelled,
    Verified(bool),
    Deleted,
    Waited,
    /// The user pressed Retry, or the Mac woke up: start over.
    Retry,
}

/// One download of one file of `size` bytes.
#[derive(Clone, Debug)]
pub struct Machine {
    size: u64,
    transfers: u32,
    bad_checksums: u32,
    step: Step,
}

impl Machine {
    /// A new download; the first step is [`Step::CheckDisk`].
    pub fn new(size: u64) -> Machine {
        Machine {
            size,
            transfers: 0,
            bad_checksums: 0,
            step: Step::CheckDisk,
        }
    }

    pub fn step(&self) -> &Step {
        &self.step
    }

    /// Transfers started so far.
    pub fn transfers(&self) -> u32 {
        self.transfers
    }

    /// Bytes the disk needs free for this file.
    pub fn needed_bytes(&self) -> u64 {
        self.size.saturating_add(DISK_MARGIN)
    }

    fn set(&mut self, s: Step) -> Step {
        self.step = s.clone();
        s
    }

    /// Applies one event and returns the next step.
    pub fn on(&mut self, event: Event) -> Step {
        let next = match (&self.step, event) {
            (_, Event::Retry) => {
                self.transfers = 0;
                self.bad_checksums = 0;
                Step::CheckDisk
            }
            (Step::CheckDisk, Event::DiskFree(free)) => {
                if free.is_some_and(|f| f < self.needed_bytes()) {
                    Step::Failed(SetupProblem::NoSpace)
                } else {
                    self.transfers += 1;
                    Step::Transfer
                }
            }
            (Step::Transfer, Event::TransferDone) => Step::Verify,
            (Step::Transfer, Event::TransferCancelled) => Step::Stopped,
            (Step::Transfer, Event::TransferFailed(probe)) => {
                if self.transfers >= MAX_TRANSFERS {
                    Step::Failed(probe.problem())
                } else {
                    let (delay, shown) = match probe {
                        Probe::RateLimited {
                            retry_after: Some(d),
                        } => (d.clamp(Duration::from_secs(1), RETRY_AFTER_CAP), None),
                        Probe::CaptivePortal => (PORTAL_WAIT, Some(SetupProblem::CheckConnection)),
                        _ => (backoff(self.transfers), None),
                    };
                    Step::Wait {
                        delay,
                        problem: shown,
                    }
                }
            }
            (Step::Verify, Event::Verified(true)) => Step::Ready,
            (Step::Verify, Event::Verified(false)) => {
                self.bad_checksums += 1;
                Step::Delete
            }
            (Step::Delete, Event::Deleted) => {
                if self.bad_checksums >= 2 {
                    Step::Failed(SetupProblem::Corrupt)
                } else {
                    // The one extra download a bad checksum earns does not
                    // count against the network budget.
                    self.transfers = self.transfers.saturating_sub(1);
                    Step::CheckDisk
                }
            }
            (Step::Wait { .. }, Event::Waited) => Step::CheckDisk,
            // Anything else is a driver bug or a late event: stay put.
            (current, e) => {
                log::debug!("models: download event {e:?} ignored in {current:?}");
                current.clone()
            }
        };
        self.set(next)
    }
}

/// Parses a `Retry-After` value: delta seconds, or an HTTP date relative to
/// `now` (unix seconds). A date in the past is zero.
pub fn parse_retry_after(value: &str, now_unix: i64) -> Option<Duration> {
    let v = value.trim();
    if let Ok(secs) = v.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let at = chrono::DateTime::parse_from_rfc2822(v).ok()?.timestamp();
    Some(Duration::from_secs(
        at.saturating_sub(now_unix).max(0) as u64
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    fn started(size: u64) -> Machine {
        let mut m = Machine::new(size);
        assert_eq!(m.step(), &Step::CheckDisk);
        assert_eq!(m.on(Event::DiskFree(Some(10_000 * MB))), Step::Transfer);
        m
    }

    #[test]
    fn happy_path_checks_disk_transfers_and_verifies() {
        let mut m = started(135 * MB);
        assert_eq!(m.on(Event::TransferDone), Step::Verify);
        assert_eq!(m.on(Event::Verified(true)), Step::Ready);
        assert_eq!(m.transfers(), 1);
    }

    #[test]
    fn disk_full_stops_before_any_transfer_and_retry_rechecks() {
        let mut m = Machine::new(740 * MB);
        assert_eq!(
            m.on(Event::DiskFree(Some(740 * MB))),
            Step::Failed(SetupProblem::NoSpace)
        );
        assert_eq!(m.transfers(), 0);
        // The user freed space and pressed Retry.
        assert_eq!(m.on(Event::Retry), Step::CheckDisk);
        assert_eq!(
            m.on(Event::DiskFree(Some(740 * MB + DISK_MARGIN))),
            Step::Transfer
        );
        // Unknown free space never blocks.
        let mut m = Machine::new(MB);
        assert_eq!(m.on(Event::DiskFree(None)), Step::Transfer);
    }

    #[test]
    fn offline_backs_off_then_gives_up_with_retry() {
        let mut m = started(MB);
        let mut waits = Vec::new();
        loop {
            match m.on(Event::TransferFailed(Probe::Offline)) {
                Step::Wait { delay, problem } => {
                    assert_eq!(problem, None);
                    waits.push(delay.as_secs());
                    assert_eq!(m.on(Event::Waited), Step::CheckDisk);
                    assert_eq!(m.on(Event::DiskFree(None)), Step::Transfer);
                }
                Step::Failed(p) => {
                    assert_eq!(p, SetupProblem::Offline);
                    break;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(waits, [5, 15, 30, 60, 120]);
        assert_eq!(m.transfers(), MAX_TRANSFERS);
        // Retry starts a fresh budget.
        assert_eq!(m.on(Event::Retry), Step::CheckDisk);
        assert_eq!(m.on(Event::DiskFree(None)), Step::Transfer);
        assert_eq!(m.transfers(), 1);
    }

    #[test]
    fn retry_after_is_honoured_on_429_and_503() {
        let mut m = started(MB);
        assert_eq!(
            m.on(Event::TransferFailed(Probe::RateLimited {
                retry_after: Some(Duration::from_secs(42))
            })),
            Step::Wait {
                delay: Duration::from_secs(42),
                problem: None
            }
        );
        m.on(Event::Waited);
        m.on(Event::DiskFree(None));
        // Capped, and never zero.
        assert_eq!(
            m.on(Event::TransferFailed(Probe::RateLimited {
                retry_after: Some(Duration::from_secs(86_400))
            })),
            Step::Wait {
                delay: RETRY_AFTER_CAP,
                problem: None
            }
        );
        m.on(Event::Waited);
        m.on(Event::DiskFree(None));
        assert_eq!(
            m.on(Event::TransferFailed(Probe::RateLimited {
                retry_after: Some(Duration::ZERO)
            })),
            Step::Wait {
                delay: Duration::from_secs(1),
                problem: None
            }
        );
        // Without a header: the normal backoff (fourth failure: 60 s).
        m.on(Event::Waited);
        m.on(Event::DiskFree(None));
        assert_eq!(
            m.on(Event::TransferFailed(Probe::RateLimited {
                retry_after: None
            })),
            Step::Wait {
                delay: Duration::from_secs(60),
                problem: None
            }
        );
    }

    #[test]
    fn server_errors_end_in_couldnt_download() {
        let mut m = started(MB);
        for _ in 1..MAX_TRANSFERS {
            assert!(matches!(
                m.on(Event::TransferFailed(Probe::ServerError)),
                Step::Wait { .. }
            ));
            m.on(Event::Waited);
            m.on(Event::DiskFree(None));
        }
        assert_eq!(
            m.on(Event::TransferFailed(Probe::Reachable)),
            Step::Failed(SetupProblem::Server)
        );
    }

    #[test]
    fn a_captive_portal_says_check_your_connection_and_keeps_trying() {
        let mut m = started(MB);
        assert_eq!(
            m.on(Event::TransferFailed(Probe::CaptivePortal)),
            Step::Wait {
                delay: PORTAL_WAIT,
                problem: Some(SetupProblem::CheckConnection)
            }
        );
        // The user logged in to the Wi-Fi: the next transfer works.
        m.on(Event::Waited);
        assert_eq!(m.on(Event::DiskFree(None)), Step::Transfer);
        assert_eq!(m.on(Event::TransferDone), Step::Verify);
        assert_eq!(m.on(Event::Verified(true)), Step::Ready);
        // Still behind the portal after every attempt: that is the state.
        let mut m = started(MB);
        let last = loop {
            match m.on(Event::TransferFailed(Probe::CaptivePortal)) {
                Step::Wait { .. } => {
                    m.on(Event::Waited);
                    m.on(Event::DiskFree(None));
                }
                other => break other,
            }
        };
        assert_eq!(last, Step::Failed(SetupProblem::CheckConnection));
    }

    #[test]
    fn a_bad_checksum_deletes_and_downloads_once_more() {
        let mut m = started(MB);
        assert_eq!(m.on(Event::TransferDone), Step::Verify);
        assert_eq!(m.on(Event::Verified(false)), Step::Delete);
        assert_eq!(m.on(Event::Deleted), Step::CheckDisk);
        assert_eq!(m.on(Event::DiskFree(None)), Step::Transfer);
        // The extra download is free.
        assert_eq!(m.transfers(), 1);
        assert_eq!(m.on(Event::TransferDone), Step::Verify);
        assert_eq!(m.on(Event::Verified(true)), Step::Ready);

        // Twice wrong: fail (the file is deleted first), never "ready".
        let mut m = started(MB);
        m.on(Event::TransferDone);
        m.on(Event::Verified(false));
        m.on(Event::Deleted);
        m.on(Event::DiskFree(None));
        m.on(Event::TransferDone);
        assert_eq!(m.on(Event::Verified(false)), Step::Delete);
        assert_eq!(m.on(Event::Deleted), Step::Failed(SetupProblem::Corrupt));
    }

    #[test]
    fn cancel_stops_and_late_events_are_ignored() {
        let mut m = started(MB);
        assert_eq!(m.on(Event::TransferCancelled), Step::Stopped);
        assert_eq!(m.on(Event::TransferDone), Step::Stopped);
        let mut m = started(MB);
        assert_eq!(m.on(Event::Verified(true)), Step::Transfer);
        assert_eq!(m.on(Event::Waited), Step::Transfer);
    }

    #[test]
    fn wake_or_retry_during_a_wait_starts_over() {
        let mut m = started(MB);
        for _ in 0..3 {
            m.on(Event::TransferFailed(Probe::Offline));
            m.on(Event::Waited);
            m.on(Event::DiskFree(None));
        }
        assert!(matches!(
            m.on(Event::TransferFailed(Probe::Offline)),
            Step::Wait { .. }
        ));
        assert_eq!(m.on(Event::Retry), Step::CheckDisk);
        m.on(Event::DiskFree(None));
        assert_eq!(m.transfers(), 1);
    }

    #[test]
    fn backoff_schedule() {
        let s: Vec<u64> = (0..8).map(|n| backoff(n).as_secs()).collect();
        assert_eq!(s, [5, 5, 15, 30, 60, 120, 120, 120]);
    }

    #[test]
    fn retry_after_parses_seconds_and_dates() {
        assert_eq!(parse_retry_after("120", 0), Some(Duration::from_secs(120)));
        assert_eq!(parse_retry_after(" 7 ", 0), Some(Duration::from_secs(7)));
        // Wed, 21 Oct 2015 07:28:00 GMT = 1445412480.
        assert_eq!(
            parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT", 1_445_412_480 - 90),
            Some(Duration::from_secs(90))
        );
        assert_eq!(
            parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT", 1_445_412_480 + 5),
            Some(Duration::ZERO)
        );
        assert_eq!(parse_retry_after("soon", 0), None);
        assert_eq!(parse_retry_after("-3", 0), None);
    }
}
