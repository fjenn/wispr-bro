// SPDX-License-Identifier: GPL-3.0-or-later
//! The iPhone's two native sync pieces (`ios.m`): the share sheet for the
//! files this phone sends, and the file another app hands over (AirDrop,
//! Files, "Open in Wispr Bro"; `scene.m` passes file URLs here).

use std::ffi::{c_char, CStr, CString};
use std::sync::OnceLock;

use tauri::AppHandle;

extern "C" {
    fn wb_ios_share_files(paths_json: *const c_char);
}

static APP: OnceLock<AppHandle> = OnceLock::new();

/// From `fork::on_ready`: the page can be told about opened files.
pub fn on_ready(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

/// Opens the iOS share sheet (AirDrop, Files, …) for `paths`.
pub fn share_files(paths: &[String]) {
    let Some(json) = serde_json::to_string(paths)
        .ok()
        .and_then(|j| CString::new(j).ok())
    else {
        return;
    };
    // SAFETY: a valid NUL-terminated string that outlives the call; the
    // function copies it and presents on the main queue.
    unsafe { wb_ios_share_files(json.as_ptr()) }
}

/// Called by `scene.m` with the path of a file opened in the app.
///
/// # Safety
/// `path` is null or a valid NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn wb_sync_file_opened(path: *const c_char) {
    if path.is_null() {
        return;
    }
    // SAFETY: checked non-null; valid per the contract above.
    let path = unsafe { CStr::from_ptr(path) }
        .to_string_lossy()
        .into_owned();
    crate::fork::commands::sync::file_opened(APP.get(), path);
}
