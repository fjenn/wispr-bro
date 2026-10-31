// SPDX-License-Identifier: GPL-3.0-or-later
//! What a take knows about where it is going (S-09: the frontmost app and its
//! taxonomy; S-19b adds the browser host, S-20 the text near the caret).

pub mod app_rules;
pub mod apps;
pub mod categories;
pub mod frontmost;
pub mod take;

use take::{ledger, TakeContext};

/// Unix time in milliseconds.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// A take as of now: time, local day, frontmost app.
pub fn current_context() -> TakeContext {
    let now = now_ms();
    TakeContext::new(
        now,
        crate::fork::stats::local_day(now),
        frontmost::frontmost(),
    )
}

/// Record start (from `fork::on_record_start`): note the app the user is
/// dictating into. Cheap (one NSWorkspace call); runs on Handy's start path.
pub fn on_record_start() {
    let ctx = current_context();
    log::debug!(
        "context: take started in {} ({}, {})",
        ctx.app
            .as_ref()
            .and_then(|a| a.bundle_id.as_deref())
            .unwrap_or("?"),
        ctx.class.style_group.as_str(),
        ctx.class.usage_category.as_str()
    );
    ledger().record_started(ctx);
}
