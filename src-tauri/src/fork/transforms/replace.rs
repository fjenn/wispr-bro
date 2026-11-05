// SPDX-License-Identifier: GPL-3.0-or-later
//! Focus-checked, single-paste replacement through S-04's clipboard line.

use tauri::{AppHandle, Runtime};

use super::selection::Replacement;
use crate::fork::ax::Focus;
use crate::fork::insert::deliver::{Delivery, Report};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplaceError {
    FocusMoved,
    Cancelled,
    PasteFailed,
}

/// A missing/unknown comparison cannot prove a focus change; a definite app or
/// element change aborts instead of pasting into the new target.
pub fn focus_allows(focus: Option<&Focus>) -> bool {
    focus.is_none_or(|focus| focus.same_as_captured != Some(false))
}

pub fn replace<R: Runtime>(
    app: &AppHandle<R>,
    text: String,
    target: u64,
    replacement: Replacement,
    cancelled: &(dyn Fn() -> bool + Send + Sync),
) -> Result<Report, ReplaceError> {
    let report = crate::fork::insert::deliver_transform(
        app,
        text,
        target,
        replacement == Replacement::WholeField,
        cancelled,
    )
    .map_err(|_| ReplaceError::PasteFailed)?;
    match report.delivery {
        Delivery::Pasted { .. } => Ok(report),
        Delivery::Cancelled => Err(ReplaceError::Cancelled),
        Delivery::Card { reason } => match reason {
            crate::fork::insert::deliver::CardReason::FocusMoved => Err(ReplaceError::FocusMoved),
            _ => Err(ReplaceError::PasteFailed),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fork::ax::{editable::Facts, editable::Family, editable::Verdict};

    #[test]
    fn focus_guard_aborts_only_on_a_definite_change() {
        let focus = |same| Focus {
            pid: Some(1),
            app: "editor".into(),
            family: Family::Native,
            facts: Facts::default(),
            verdict: Verdict::Unknown(crate::fork::ax::editable::Reason::NoElementWithWindow),
            same_as_captured: same,
            ms: 0.0,
        };
        assert!(focus_allows(None));
        assert!(focus_allows(Some(&focus(None))));
        assert!(focus_allows(Some(&focus(Some(true)))));
        assert!(!focus_allows(Some(&focus(Some(false)))));
    }
}
