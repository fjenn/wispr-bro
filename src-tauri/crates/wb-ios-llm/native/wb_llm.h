// SPDX-License-Identifier: GPL-3.0-or-later
//
// llama.cpp's server request code, in process. The only symbols this library
// exports.
#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// The library is built with hidden visibility; these are the exceptions.
#define WB_LLM_API __attribute__((visibility("default")))

typedef struct wb_llm wb_llm;

// Returns non-zero when the request must stop (cancelled or past its
// deadline). Polled between tokens, and once a second while waiting.
typedef int (*wb_llm_stop_fn)(void * user);
// Called once, before any data: the HTTP status llama-server would send and
// whether the body is a stream (server-sent events).
typedef void (*wb_llm_head_fn)(void * user, int status, int is_stream);
// Receives the response body: all of it for a plain response, one piece per
// call for a stream (as llama-server writes them).
typedef void (*wb_llm_data_fn)(void * user, const char * data, size_t len);

// Loads a model with llama-server's own command line (argv[0] is a name;
// `--host`/`--port` are ignored: nothing listens). Returns NULL on failure
// with a message in `err`. One engine per process at a time.
WB_LLM_API wb_llm * wb_llm_start(int argc, const char * const * argv, char * err, size_t err_len);

// Runs one request through the server's route handler for `path`
// ("/v1/chat/completions", "/props", "/health", "/tokenize",
// "/apply-template", "/slots"). Returns the HTTP status the server would
// send, or -1 for an unknown path, -2 when stopped before a response.
WB_LLM_API int wb_llm_request(wb_llm * h, const char * path, const char * body, size_t body_len,
                   wb_llm_stop_fn stop, wb_llm_head_fn head, wb_llm_data_fn data,
                   void * user);

// Size of the loaded model's tensors in bytes.
WB_LLM_API uint64_t wb_llm_model_bytes(const wb_llm * h);
// Context size in tokens.
WB_LLM_API int32_t wb_llm_n_ctx(const wb_llm * h);

// Ends the engine and frees the model. Requests still running are stopped.
WB_LLM_API void wb_llm_stop(wb_llm * h);

// 0 = errors only (default), 1 = warnings, 2 = info, 3 = debug. stderr.
WB_LLM_API void wb_llm_set_verbosity(int level);

// "b11624-af0579c78" style build string of the linked llama.cpp.
WB_LLM_API const char * wb_llm_build_info(void);

#ifdef __cplusplus
}
#endif
