// SPDX-License-Identifier: GPL-3.0-or-later
//! First-run navigation and consented macOS setup actions (S-29a); the
//! practice take (S-29b); the setup checklist's ticks (S-30).

pub mod checklist;
pub mod native;
pub mod practice;
pub mod state;

#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};

#[cfg(target_os = "macos")]
use crate::fork::settings::ForkSettingsHandle;
#[cfg(target_os = "macos")]
use crate::fork::ForkRuntime;

/// alpha.3: runs from `fork::on_ready` on macOS. A saved Move to Applications
/// step advances to Languages (the screen no longer exists), then the app
/// settles its install location: already installed is a no-op, a free
/// destination is moved to and relaunched, an existing installed copy is
/// opened, and only a failed move shows the one small native prompt.
#[cfg(target_os = "macos")]
pub fn ensure_location_at_startup<R: ForkRuntime>(app: &AppHandle<R>) {
    if let Some(handle) = app.try_state::<ForkSettingsHandle>() {
        match handle.update(app, |settings| {
            Ok(state::skip_move_to_applications(&mut settings.onboarding))
        }) {
            Ok((_, true)) => {
                log::info!("onboarding: move_to_applications resumes at languages")
            }
            Ok((_, false)) => {}
            Err(e) => log::warn!("onboarding: cannot advance past the move step: {e}"),
        }
    }
    native::ensure_location(app);
}
