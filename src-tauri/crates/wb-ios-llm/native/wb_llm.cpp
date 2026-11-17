// SPDX-License-Identifier: GPL-3.0-or-later
//
// llama-server's request path without the HTTP listener: the same steps as
// `llama_server()` in tools/server/server.cpp (single-model mode), then the
// route handlers called directly. Arguments are parsed by llama-server's own
// parser, requests are the same JSON bodies, replies the same bytes.

#include "wb_llm.h"

#include "server-context.h"
#include "server-common.h"
#include "server-stream.h"

#include "arg.h"
#include "build-info.h"
#include "common.h"
#include "llama.h"
#include "log.h"

#include <atomic>
#include <clocale>
#include <cstdio>
#include <cstring>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

struct wb_llm {
    common_params                  params;
    std::unique_ptr<server_context> ctx;
    std::unique_ptr<server_routes>  routes;
    std::thread                    loop;
    uint64_t                       model_bytes = 0;
    int32_t                        n_ctx       = 0;
};

// Metal hands out autoreleased objects. The app calls in from worker threads
// that have no autorelease pool, where such objects would live until the
// thread ends; every entry point and the task loop get their own pool.
extern "C" void * objc_autoreleasePoolPush(void);
extern "C" void   objc_autoreleasePoolPop(void * pool);

namespace {

struct autorelease_pool {
    void * pool = objc_autoreleasePoolPush();
    ~autorelease_pool() { objc_autoreleasePoolPop(pool); }
    autorelease_pool() = default;
    autorelease_pool(const autorelease_pool &) = delete;
    autorelease_pool & operator=(const autorelease_pool &) = delete;
};

std::mutex       g_mutex;      // start / stop
bool             g_running = false;
std::once_flag   g_backend_once;
std::atomic<int> g_verbosity{LOG_LEVEL_ERROR};

void set_error(char * err, size_t err_len, const std::string & message) {
    if (err != nullptr && err_len > 0) {
        std::snprintf(err, err_len, "%s", message.c_str());
    }
}

const server_http_context::handler_t * route(const wb_llm * h, const std::string & path) {
    const server_routes & r = *h->routes;
    if (path == "/v1/chat/completions" || path == "/chat/completions") return &r.post_chat_completions;
    if (path == "/props")          return &r.get_props;
    if (path == "/health" || path == "/v1/health") return &r.get_health;
    if (path == "/tokenize")       return &r.post_tokenize;
    if (path == "/apply-template") return &r.post_apply_template;
    if (path == "/slots")          return &r.get_slots;
    return nullptr;
}

// server.cpp `ex_wrapper`: a handler never throws; an exception is the
// server's own error response.
server_http_res_ptr call(const server_http_context::handler_t & handler, const server_http_req & req) {
    std::string message;
    error_type  error = ERROR_TYPE_SERVER;
    try {
        return handler(req);
    } catch (const std::invalid_argument & e) {
        error   = ERROR_TYPE_INVALID_REQUEST;
        message = e.what();
    } catch (const common_json_error & e) {
        error   = ERROR_TYPE_INVALID_REQUEST;
        message = e.what();
    } catch (const std::exception & e) {
        message = e.what();
    } catch (...) {
        message = "unknown error";
    }
    auto res    = std::make_unique<server_http_res>();
    res->status = 500;
    try {
        json error_data = format_error_response(message, error);
        res->status     = json_value(error_data, "code", 500);
        res->data       = safe_json_to_str({{"error", error_data}});
    } catch (...) {
        res->data = "Internal Server Error";
    }
    return res;
}

} // namespace

extern "C" {

void wb_llm_set_verbosity(int level) {
    int thold = LOG_LEVEL_ERROR;
    switch (level) {
        case 1:  thold = LOG_LEVEL_WARN;  break;
        case 2:  thold = LOG_LEVEL_INFO;  break;
        case 3:  thold = LOG_LEVEL_DEBUG; break;
        default: thold = LOG_LEVEL_ERROR; break;
    }
    g_verbosity.store(thold);
    common_log_set_verbosity_thold(thold);
}

const char * wb_llm_build_info(void) {
    return llama_build_info();
}

wb_llm * wb_llm_start(int argc, const char * const * argv, char * err, size_t err_len) {
    autorelease_pool pool;
    std::lock_guard<std::mutex> lock(g_mutex);
    if (g_running) {
        set_error(err, err_len, "an engine is already running");
        return nullptr;
    }
    try {
        std::setlocale(LC_NUMERIC, "C");
        common_init();
        common_log_set_verbosity_thold(g_verbosity.load());

        auto h = std::make_unique<wb_llm>();
        std::vector<std::string> args(argv, argv + argc);
        std::vector<char *>      cargv;
        for (auto & a : args) {
            cargv.push_back(a.data());
        }
        if (!common_params_parse((int) cargv.size(), cargv.data(), h->params, LLAMA_EXAMPLE_SERVER)) {
            set_error(err, err_len, "invalid arguments");
            return nullptr;
        }
        // `-lv` in the arguments wins; otherwise ours.
        common_log_set_verbosity_thold(g_verbosity.load());

        std::call_once(g_backend_once, [&] {
            llama_backend_init();
        });
        llama_numa_init(h->params.numa);

        // server.cpp, single-model mode.
        if (h->params.n_parallel < 0) {
            h->params.n_parallel = 4;
            h->params.kv_unified = true;
        }
        auto model_name = h->params.model.get_name();
        if (h->params.model_alias.empty() && !model_name.empty()) {
            h->params.model_alias.insert(model_name);
        }

        server_stream_session_manager_start();
        h->ctx    = std::make_unique<server_context>();
        h->routes = std::make_unique<server_routes>(h->params, *h->ctx);
        if (!h->ctx->load_model(h->params)) {
            h->routes.reset();
            h->ctx.reset();
            server_stream_session_manager_stop();
            set_error(err, err_len, "the model could not be loaded");
            return nullptr;
        }
        h->routes->update_meta(*h->ctx);
        {
            const auto meta = h->ctx->get_meta();
            h->model_bytes  = meta.model_size;
            h->n_ctx        = meta.slot_n_ctx;
        }
        server_context * ctx = h->ctx.get();
        h->loop = std::thread([ctx] {
            autorelease_pool pool;
            ctx->start_loop();
        });
        g_running = true;
        return h.release();
    } catch (const std::exception & e) {
        set_error(err, err_len, e.what());
    } catch (...) {
        set_error(err, err_len, "unknown error");
    }
    return nullptr;
}

int wb_llm_request(wb_llm * h, const char * path, const char * body, size_t body_len,
                   wb_llm_stop_fn stop, wb_llm_head_fn head, wb_llm_data_fn data,
                   void * user) {
    if (h == nullptr || path == nullptr) {
        return -1;
    }
    const auto * handler = route(h, path);
    if (handler == nullptr) {
        return -1;
    }
    autorelease_pool pool;
    const std::function<bool()> should_stop = [stop, user]() {
        return stop != nullptr && stop(user) != 0;
    };
    server_http_req req{
        /* params       */ {},
        /* headers      */ {},
        /* path         */ path,
        /* query_string */ "",
        /* body         */ body != nullptr ? std::string(body, body_len) : std::string(),
        /* files        */ {},
        /* should_stop  */ should_stop,
    };
    server_http_res_ptr res = call(*handler, req);
    if (res == nullptr) {
        return -2;
    }
    const int status = res->status;
    if (!res->is_stream()) {
        const bool stopped = res->data.empty() && should_stop();
        if (head != nullptr && !stopped) {
            head(user, status, 0);
        }
        if (data != nullptr && !res->data.empty()) {
            data(user, res->data.data(), res->data.size());
        }
        res->on_complete();
        return stopped ? -2 : status;
    }
    if (head != nullptr) {
        head(user, status, 1);
    }
    // server-http.cpp's chunked content provider: one piece per next().
    std::string chunk;
    bool        more = true;
    while (more) {
        chunk.clear();
        try {
            more = res->next(chunk);
        } catch (...) {
            more = false;
        }
        if (data != nullptr && !chunk.empty()) {
            data(user, chunk.data(), chunk.size());
        }
    }
    res->on_complete();
    return status;
}

uint64_t wb_llm_model_bytes(const wb_llm * h) {
    return h != nullptr ? h->model_bytes : 0;
}

int32_t wb_llm_n_ctx(const wb_llm * h) {
    return h != nullptr ? h->n_ctx : 0;
}

void wb_llm_stop(wb_llm * h) {
    if (h == nullptr) {
        return;
    }
    autorelease_pool pool;
    std::lock_guard<std::mutex> lock(g_mutex);
    // server.cpp `clean_up`, without the HTTP listener and backend free (the
    // backend stays initialised for the next start).
    server_stream_session_manager_stop();
    h->ctx->terminate();
    if (h->loop.joinable()) {
        h->loop.join();
    }
    h->routes.reset();
    h->ctx.reset();
    delete h;
    g_running = false;
}

} // extern "C"
