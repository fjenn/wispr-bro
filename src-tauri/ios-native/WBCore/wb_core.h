// SPDX-License-Identifier: GPL-3.0-or-later
// Wispr Bro for iPhone: the Rust core's C API, as called by the native Swift
// session code (WBSession). Implemented in Rust; WBCoreStub/wb_core_stub.c is
// a stand-in so the native parts build and run without the Rust library.
//
// All strings are UTF-8. Returned strings are allocated by the core and must
// be freed with wb_string_free.

#ifndef WB_CORE_H
#define WB_CORE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Called once, early at launch, from the main thread. Safe to call again
/// (later calls are ignored).
void wb_core_init(const char *app_support_dir, const char *app_group_dir);

/// {"ready":bool,"model":"...","downloading":bool,"progress":0..1,"message":"..."}
char *wb_core_status_json(void);

/// Begins a take. `take_id` is non-zero and unique per take.
void wb_take_start(uint64_t take_id);

/// Feeds 16 kHz mono Float32 samples (about 33 ms per call, from one audio
/// thread, in order). Returns the display level 0..1 for the waveform.
float wb_take_feed(uint64_t take_id, const float *samples_16k_mono, size_t count);

/// BLOCKING: transcribes and cleans the take. Called off the main thread,
/// after the last wb_take_feed for this take has returned.
/// JSON {"ok":bool,"text":"...","error":"..."}.
char *wb_take_finish(uint64_t take_id);

/// Discards a take. May be called while wb_take_finish is running on another
/// thread (its result is then ignored by the caller).
void wb_take_cancel(uint64_t take_id);

void wb_string_free(char *s);

#ifdef __cplusplus
}
#endif

#endif // WB_CORE_H
