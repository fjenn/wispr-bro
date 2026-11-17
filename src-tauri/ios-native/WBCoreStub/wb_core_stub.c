// SPDX-License-Identifier: GPL-3.0-or-later
// A stand-in for the Rust core (WBCore/wb_core.h): canned text after 400 ms
// and a level computed from the samples. Link this OR the Rust library, never
// both.

#include "../WBCore/wb_core.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static uint64_t wb_stub_take = 0;
static uint64_t wb_stub_cancelled = 0;
static size_t wb_stub_samples = 0;

void wb_core_init(const char *app_support_dir, const char *app_group_dir) {
    (void)app_support_dir;
    (void)app_group_dir;
}

char *wb_core_status_json(void) {
    return strdup("{\"ready\":true,\"model\":\"stub\",\"downloading\":false,\"progress\":1,\"message\":\"\"}");
}

void wb_take_start(uint64_t take_id) {
    wb_stub_take = take_id;
    wb_stub_samples = 0;
}

float wb_take_feed(uint64_t take_id, const float *samples, size_t count) {
    if (take_id != wb_stub_take || samples == NULL || count == 0) {
        return 0.0f;
    }
    wb_stub_samples += count;
    double sum = 0.0;
    for (size_t i = 0; i < count; i++) {
        sum += (double)samples[i] * (double)samples[i];
    }
    double rms = sqrt(sum / (double)count);
    // -60 dBFS..-12 dBFS → 0..1
    double db = 20.0 * log10(rms + 1e-9);
    double level = (db + 60.0) / 48.0;
    if (level < 0.0) level = 0.0;
    if (level > 1.0) level = 1.0;
    return (float)level;
}

char *wb_take_finish(uint64_t take_id) {
    usleep(400 * 1000);
    if (take_id == wb_stub_cancelled) {
        return strdup("{\"ok\":false,\"text\":\"\",\"error\":\"cancelled\"}");
    }
    char buffer[160];
    snprintf(buffer, sizeof buffer,
             "{\"ok\":true,\"text\":\"Hello from the stub (%.1f s).\",\"error\":\"\"}",
             (double)wb_stub_samples / 16000.0);
    return strdup(buffer);
}

void wb_take_cancel(uint64_t take_id) {
    wb_stub_cancelled = take_id;
}

void wb_string_free(char *s) {
    free(s);
}
