// SPDX-License-Identifier: GPL-3.0-or-later
//
// Weak stand-ins for the C functions the Swift session code exports
// (src-tauri/ios-native/WBSession/wb_ios.h, WBSessionCAPI.swift). They let the
// Rust library link into an app that does not contain those Swift sources yet
// (the plain `tauri ios build` app). Once WBSession is in the Xcode app
// target, its strong definitions win at link time and these are dropped;
// nothing has to be switched.
//
// Keep the signatures in step with wb_ios.h.

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define WB_WEAK __attribute__((weak))

WB_WEAK char *wb_ios_session_state_json(void) {
    return strdup("{\"active\":false,\"state\":\"noSession\",\"message\":null,"
                  "\"expiresAt\":null,\"nativeSession\":false}");
}

WB_WEAK void wb_ios_string_free(char *s) { free(s); }

WB_WEAK void wb_ios_start_session(void) {}

WB_WEAK void wb_ios_end_session(void) {}

WB_WEAK int32_t wb_ios_keyboard_enabled(void) { return 0; }

WB_WEAK int32_t wb_ios_full_access_known(void) { return -1; }

WB_WEAK int32_t wb_ios_mic_permission(void) { return 0; }

WB_WEAK void wb_ios_request_mic_permission(void) {}

WB_WEAK void wb_ios_open_settings(void) {}

WB_WEAK uint64_t wb_ios_app_take_start(void) { return 0; }

WB_WEAK void wb_ios_app_take_stop(uint64_t take_id) { (void)take_id; }

WB_WEAK void wb_ios_app_take_cancel(uint64_t take_id) { (void)take_id; }

WB_WEAK char *wb_ios_app_take_json(uint64_t take_id) {
    (void)take_id;
    return strdup("{\"phase\":\"none\",\"level\":0,\"text\":null,\"error\":null}");
}
