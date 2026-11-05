// SPDX-License-Identifier: GPL-3.0-or-later
//! What a take knows about where it is going (S-09: the frontmost app and its
//! taxonomy; S-19b adds the browser host, S-20 the text near the caret).
//!
//! S-20 (context awareness): [`caret`] reads the text around the caret at
//! record start, [`hints`] turns it into spelling hints at once (the text is
//! dropped), [`awareness`] holds the take's hints in memory until the take
//! ends.

pub mod app_rules;
pub mod apps;
pub mod awareness;
pub mod caret;
pub mod categories;
pub mod file_names;
pub mod frontmost;
pub mod hints;
pub mod ide;
#[cfg(test)]
mod names_eval;
pub mod recognizer_prompt;
pub mod take;
pub mod url;
pub mod workspace;

use take::{ledger, TakeContext};

/// Unix time in milliseconds.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// A take as of now: time, local day, frontmost app and reduced browser host.
pub fn current_context() -> TakeContext {
    let now = now_ms();
    let app = frontmost::frontmost();
    let host = app
        .as_ref()
        .and_then(|a| url::current_host(a.bundle_id.as_deref()));
    let mut ctx = TakeContext::new(now, crate::fork::stats::local_day(now), app);
    if let Some(host) = host {
        ctx = ctx.with_url(&host);
    }
    ctx
}

/// Record start (from `fork::on_record_start`): note the app and, for a
/// supported browser, the reduced host the user is dictating into.
pub fn on_record_start() {
    let mut ctx = current_context();
    ctx.source = crate::fork::transforms::command::source();
    // S-29b: a take on the open practice screen is a practice take (DEC-11).
    let practice = crate::fork::onboarding::practice::self_in_front;
    if ctx.source == take::TakeSource::Dictation
        && crate::fork::onboarding::practice::on_record_start(practice(), std::time::Instant::now())
    {
        ctx.source = take::TakeSource::Practice;
    } else if ctx.source != take::TakeSource::Dictation {
        crate::fork::onboarding::practice::clear_take();
    }
    log::debug!(
        "context: take started in {} ({}, {}, host {})",
        ctx.app
            .as_ref()
            .and_then(|a| a.bundle_id.as_deref())
            .unwrap_or("?"),
        ctx.class.style_group.as_str(),
        ctx.class.usage_category.as_str(),
        ctx.url_host.as_deref().unwrap_or("?")
    );
    ledger().record_started(ctx);
}
