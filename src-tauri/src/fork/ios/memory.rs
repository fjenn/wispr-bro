// SPDX-License-Identifier: GPL-3.0-or-later
//! Memory warnings on iPhone. The speech model is the product; the cleanup
//! model is the part that can go. On UIKit's memory warning
//! (`scene.m` → [`wb_ios_memory_warning`]) the cleanup model is unloaded at
//! once, unless a cleanup is running (it ends within its deadline and the
//! idle owner unloads it right after), and takes go without the model for
//! [`crate::fork::llm::memory::WARNING_HOLD`]: they get the rule-based text.
//! The speech model is not touched.

use crate::fork::llm::{memory, runtime};

/// Called by UIKit's memory-warning observer (any thread).
#[no_mangle]
pub extern "C" fn wb_ios_memory_warning() {
    memory::note_warning();
    let footprint = wb_ios_llm::footprint_bytes().unwrap_or(0) / (1024 * 1024);
    log::warn!("ios: memory warning at {footprint} MB; the cleanup model is unloaded");
    let Some(app) = super::core::app_handle() else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(rt) = runtime::runtime(&app) else {
            return;
        };
        let Some(sup) = rt.supervisor().cloned() else {
            return;
        };
        let stopped = sup.stop_if_idle().await;
        let after = wb_ios_llm::footprint_bytes().unwrap_or(0) / (1024 * 1024);
        log::info!("ios: memory warning handled (model unloaded: {stopped}, now {after} MB)");
    });
}
