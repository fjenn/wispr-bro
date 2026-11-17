// SPDX-License-Identifier: GPL-3.0-or-later
// C-callable helpers exported by the native session code (WBSessionCAPI.swift)
// for Rust / JS. Safe to call from any thread.

#ifndef WB_IOS_H
#define WB_IOS_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// {"active":bool,"state":"noSession|idle|listening|transcribing|error",
///  "message":string|null,"takeId":string|null,"expiresAt":number|null,
///  "timeoutSeconds":number,"autoReturn":bool,"keyboardEnabled":bool,
///  "fullAccess":-1|0|1,"mic":"granted|denied|undetermined",
///  "returnHintVisible":bool}
/// malloc'd; free with wb_ios_string_free (or free()).
char *wb_ios_session_state_json(void);
void wb_ios_string_free(char *s);

/// Starts a session from inside the app (asks for the microphone if needed).
void wb_ios_start_session(void);
/// Ends the session. A take that is listening is finished and delivered first.
void wb_ios_end_session(void);

/// 1 when the Wispr Bro keyboard is among the enabled keyboards.
int32_t wb_ios_keyboard_enabled(void);
/// Last value the keyboard reported: 1 on, 0 off, -1 not reported yet.
int32_t wb_ios_full_access_known(void);
/// 0 undetermined, 1 granted, 2 denied.
int32_t wb_ios_mic_permission(void);
/// Shows the system prompt when undetermined; poll wb_ios_mic_permission.
void wb_ios_request_mic_permission(void);
/// Opens this app's page in Settings.
void wb_ios_open_settings(void);

/// Integers (bools as 0/1) in the App Group defaults suite group.com.wisprbro.ios.
int64_t wb_ios_defaults_get_int(const char *key, int64_t fallback);
void wb_ios_defaults_set_int(const char *key, int64_t value);

#ifdef __cplusplus
}
#endif

#endif // WB_IOS_H
