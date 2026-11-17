// SPDX-License-Identifier: GPL-3.0-or-later
//
// Sync over the local network, the Network.framework part (net.rs), the same
// file on the Mac and the iPhone:
//
//   - the Mac advertises a Bonjour service (`_wisprbro-sync._tcp`) and
//     accepts connections;
//   - the iPhone browses for it and connects;
//   - both send and receive whole messages (a 4-byte big-endian length, then
//     the bytes).
//
// Peer-to-peer is on (`includePeerToPeer`), so two devices without a shared
// Wi-Fi network still find each other over Apple's direct link (AWDL).
// Nothing here goes beyond the local link: there is no host name, no relay,
// no server. Everything on the wire is sealed by Rust (secure.rs); this file
// carries bytes and knows no keys.
//
// Every callback into Rust runs on one serial queue.

#import <Foundation/Foundation.h>
#import <Network/Network.h>

// kinds of `wb_sync_net_event`
enum {
    WBSyncFound = 1,      // text: service name
    WBSyncLost = 2,       // text: service name
    WBSyncIncoming = 3,   // conn: a new connection to this device
    WBSyncConnected = 4,  // conn: an outgoing connection is ready
    WBSyncMessage = 5,    // conn, bytes
    WBSyncClosed = 6,     // conn
    WBSyncListener = 7,   // text: "ready" | "failed" | "denied" | "stopped"
    WBSyncBrowser = 8,    // text: "ready" | "failed" | "denied" | "stopped"
    WBSyncTrouble = 9,    // conn, text: "<state> <domain> <code>", e.g.
                          // "waiting dns -65570" (local network refused)
};

typedef void (*wb_sync_net_event)(int32_t kind, uint64_t conn, const uint8_t *bytes, size_t length);

static const char *const kServiceType = "_wisprbro-sync._tcp";
// Larger than any dictionary and stats at their limits.
static const uint32_t kMaxMessage = 32 * 1024 * 1024;
// Ids of incoming connections; Rust numbers its outgoing ones below this.
static const uint64_t kIncomingBase = 1ull << 32;

static wb_sync_net_event gHandler;
static dispatch_queue_t gQueue;
static nw_listener_t gListener;
static nw_browser_t gBrowser;
static NSMutableDictionary<NSNumber *, nw_connection_t> *gConnections;
// The endpoints the browser found, by service name.
static NSMutableDictionary<NSString *, nw_endpoint_t> *gEndpoints;
static uint64_t gNextIncoming;

static dispatch_queue_t WBQueue(void) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        gQueue = dispatch_queue_create("com.wisprbro.sync.net", DISPATCH_QUEUE_SERIAL);
        gConnections = [NSMutableDictionary dictionary];
        gEndpoints = [NSMutableDictionary dictionary];
        gNextIncoming = kIncomingBase;
    });
    return gQueue;
}

static void WBEmit(int32_t kind, uint64_t conn, const void *bytes, size_t length) {
    if (gHandler != NULL) {
        gHandler(kind, conn, bytes, length);
    }
}

static void WBEmitText(int32_t kind, const char *text) {
    WBEmit(kind, 0, text, text ? strlen(text) : 0);
}

// Plain TCP on the local link, peer-to-peer included.
static nw_parameters_t WBParameters(void) {
    nw_parameters_t parameters = nw_parameters_create_secure_tcp(
        NW_PARAMETERS_DISABLE_PROTOCOL, NW_PARAMETERS_DEFAULT_CONFIGURATION);
    nw_parameters_set_include_peer_to_peer(parameters, true);
    return parameters;
}

// "denied": the user said no to "find devices on your local network".
static const char *WBFailure(nw_error_t error) {
    if (error != NULL && nw_error_get_error_domain(error) == nw_error_domain_dns &&
        nw_error_get_error_code(error) == -65570 /* kDNSServiceErr_PolicyDenied */) {
        return "denied";
    }
    return "failed";
}

// Why a connection waits or failed, for Rust's log and its "denied" check.
static void WBTrouble(uint64_t conn, const char *state, nw_error_t error) {
    const char *domain = "none";
    int code = 0;
    if (error != NULL) {
        code = nw_error_get_error_code(error);
        switch (nw_error_get_error_domain(error)) {
            case nw_error_domain_posix: domain = "posix"; break;
            case nw_error_domain_dns: domain = "dns"; break;
            case nw_error_domain_tls: domain = "tls"; break;
            default: domain = "other"; break;
        }
    }
    char text[64];
    snprintf(text, sizeof text, "%s %s %d", state, domain, code);
    WBEmit(WBSyncTrouble, conn, text, strlen(text));
}

static void WBDrop(uint64_t conn) {
    NSNumber *key = @(conn);
    nw_connection_t connection = gConnections[key];
    if (connection == nil) {
        return;
    }
    [gConnections removeObjectForKey:key];
    nw_connection_cancel(connection);
    WBEmit(WBSyncClosed, conn, NULL, 0);
}

static void WBReceive(uint64_t conn, nw_connection_t connection);

static void WBReceiveBody(uint64_t conn, nw_connection_t connection, uint32_t length) {
    if (length == 0) {
        WBEmit(WBSyncMessage, conn, "", 0);
        WBReceive(conn, connection);
        return;
    }
    nw_connection_receive(connection, length, length,
        ^(dispatch_data_t content, __unused nw_content_context_t context, __unused bool complete,
          __unused nw_error_t error) {
            if (content == NULL || dispatch_data_get_size(content) != length) {
                WBDrop(conn);
                return;
            }
            const void *bytes = NULL;
            size_t size = 0;
            dispatch_data_t flat = dispatch_data_create_map(content, &bytes, &size);
            WBEmit(WBSyncMessage, conn, bytes, size);
            (void)flat;
            WBReceive(conn, connection);
        });
}

static void WBReceive(uint64_t conn, nw_connection_t connection) {
    nw_connection_receive(connection, 4, 4,
        ^(dispatch_data_t content, __unused nw_content_context_t context, __unused bool complete,
          __unused nw_error_t error) {
            if (content == NULL || dispatch_data_get_size(content) != 4) {
                WBDrop(conn);
                return;
            }
            uint8_t header[4] = {0};
            const void *bytes = NULL;
            size_t size = 0;
            dispatch_data_t flat = dispatch_data_create_map(content, &bytes, &size);
            memcpy(header, bytes, 4);
            (void)flat;
            uint32_t length = ((uint32_t)header[0] << 24) | ((uint32_t)header[1] << 16) |
                              ((uint32_t)header[2] << 8) | (uint32_t)header[3];
            if (length > kMaxMessage) {
                WBDrop(conn);
                return;
            }
            WBReceiveBody(conn, connection, length);
        });
}

// Starts a connection; `readyKind` is what Rust hears when it is usable.
static void WBStart(uint64_t conn, nw_connection_t connection, int32_t readyKind) {
    gConnections[@(conn)] = connection;
    nw_connection_set_queue(connection, WBQueue());
    nw_connection_set_state_changed_handler(connection, ^(nw_connection_state_t state, nw_error_t error) {
        switch (state) {
            case nw_connection_state_ready:
                WBEmit(readyKind, conn, NULL, 0);
                WBReceive(conn, connection);
                break;
            case nw_connection_state_failed:
                WBTrouble(conn, "failed", error);
                WBDrop(conn);
                break;
            case nw_connection_state_cancelled:
                WBDrop(conn);
                break;
            // `waiting`: no usable path yet. Network.framework tries again by
            // itself when the path changes, e.g. once the user allows local
            // network access; Rust decides how long to wait.
            case nw_connection_state_waiting:
                WBTrouble(conn, "waiting", error);
                break;
            default:
                break;
        }
    });
    nw_connection_start(connection);
}

void wb_sync_net_set_handler(wb_sync_net_event handler) {
    dispatch_sync(WBQueue(), ^{
        gHandler = handler;
    });
}

// Advertises this device as `name` and accepts connections.
void wb_sync_net_advertise(const char *name) {
    NSString *service = name ? [NSString stringWithUTF8String:name] : nil;
    dispatch_async(WBQueue(), ^{
        if (gListener != nil || service == nil) {
            return;
        }
        nw_listener_t listener = nw_listener_create(WBParameters());
        if (listener == nil) {
            WBEmitText(WBSyncListener, "failed");
            return;
        }
        nw_advertise_descriptor_t advertise =
            nw_advertise_descriptor_create_bonjour_service(service.UTF8String, kServiceType, NULL);
        nw_listener_set_advertise_descriptor(listener, advertise);
        nw_listener_set_queue(listener, WBQueue());
        nw_listener_set_new_connection_handler(listener, ^(nw_connection_t connection) {
            WBStart(gNextIncoming++, connection, WBSyncIncoming);
        });
        nw_listener_set_state_changed_handler(listener, ^(nw_listener_state_t state, nw_error_t error) {
            if (state == nw_listener_state_ready) {
                WBEmitText(WBSyncListener, "ready");
            } else if (state == nw_listener_state_failed || state == nw_listener_state_waiting) {
                if (error != NULL) {
                    WBEmitText(WBSyncListener, WBFailure(error));
                }
                if (state == nw_listener_state_failed && gListener == listener) {
                    nw_listener_cancel(listener);
                    gListener = nil;
                }
            }
        });
        gListener = listener;
        nw_listener_start(listener);
    });
}

void wb_sync_net_stop_advertise(void) {
    dispatch_async(WBQueue(), ^{
        if (gListener != nil) {
            nw_listener_cancel(gListener);
            gListener = nil;
            WBEmitText(WBSyncListener, "stopped");
        }
    });
}

// Looks for devices that advertise; each is reported by its service name.
void wb_sync_net_browse(void) {
    dispatch_async(WBQueue(), ^{
        if (gBrowser != nil) {
            return;
        }
        nw_browse_descriptor_t descriptor =
            nw_browse_descriptor_create_bonjour_service(kServiceType, NULL);
        nw_browser_t browser = nw_browser_create(descriptor, WBParameters());
        nw_browser_set_queue(browser, WBQueue());
        nw_browser_set_browse_results_changed_handler(browser,
            ^(nw_browse_result_t old, nw_browse_result_t new, __unused bool batchComplete) {
                nw_browse_result_change_t change = nw_browse_result_get_changes(old, new);
                if (change & nw_browse_result_change_result_added) {
                    nw_endpoint_t endpoint = nw_browse_result_copy_endpoint(new);
                    const char *found = nw_endpoint_get_bonjour_service_name(endpoint);
                    if (found != NULL) {
                        gEndpoints[[NSString stringWithUTF8String:found]] = endpoint;
                        WBEmitText(WBSyncFound, found);
                    }
                } else if (change & nw_browse_result_change_result_removed) {
                    nw_endpoint_t endpoint = nw_browse_result_copy_endpoint(old);
                    const char *lost = nw_endpoint_get_bonjour_service_name(endpoint);
                    if (lost != NULL) {
                        [gEndpoints removeObjectForKey:[NSString stringWithUTF8String:lost]];
                        WBEmitText(WBSyncLost, lost);
                    }
                }
            });
        nw_browser_set_state_changed_handler(browser, ^(nw_browser_state_t state, nw_error_t error) {
            if (state == nw_browser_state_ready) {
                WBEmitText(WBSyncBrowser, "ready");
            } else if (state == nw_browser_state_failed || state == nw_browser_state_waiting) {
                if (error != NULL) {
                    WBEmitText(WBSyncBrowser, WBFailure(error));
                }
                if (state == nw_browser_state_failed && gBrowser == browser) {
                    nw_browser_cancel(browser);
                    gBrowser = nil;
                }
            }
        });
        gBrowser = browser;
        nw_browser_start(browser);
    });
}

void wb_sync_net_stop_browse(void) {
    dispatch_async(WBQueue(), ^{
        if (gBrowser != nil) {
            nw_browser_cancel(gBrowser);
            gBrowser = nil;
            [gEndpoints removeAllObjects];
            WBEmitText(WBSyncBrowser, "stopped");
        }
    });
}

// Connects to the advertised service `name`, as connection `conn` (an id
// below 2^32 chosen by Rust). `WBSyncConnected` or `WBSyncClosed` follows.
void wb_sync_net_connect(uint64_t conn, const char *name) {
    NSString *service = name ? [NSString stringWithUTF8String:name] : nil;
    dispatch_async(WBQueue(), ^{
        if (service == nil) {
            WBEmit(WBSyncClosed, conn, NULL, 0);
            return;
        }
        // The endpoint as the browser found it (it knows the interface, the
        // peer-to-peer one included); else by name.
        nw_endpoint_t endpoint = gEndpoints[service];
        if (endpoint == nil) {
            endpoint = nw_endpoint_create_bonjour_service(service.UTF8String, kServiceType, "local.");
        }
        nw_connection_t connection = nw_connection_create(endpoint, WBParameters());
        WBStart(conn, connection, WBSyncConnected);
    });
}

// Sends one message on `conn`.
void wb_sync_net_send(uint64_t conn, const uint8_t *bytes, size_t length) {
    if (length > kMaxMessage) {
        return;
    }
    NSMutableData *frame = [NSMutableData dataWithCapacity:length + 4];
    uint8_t header[4] = {(uint8_t)(length >> 24), (uint8_t)(length >> 16), (uint8_t)(length >> 8),
                         (uint8_t)length};
    [frame appendBytes:header length:4];
    if (length > 0) {
        [frame appendBytes:bytes length:length];
    }
    dispatch_async(WBQueue(), ^{
        nw_connection_t connection = gConnections[@(conn)];
        if (connection == nil) {
            return;
        }
        dispatch_data_t data = dispatch_data_create(frame.bytes, frame.length, WBQueue(),
                                                    DISPATCH_DATA_DESTRUCTOR_DEFAULT);
        nw_connection_send(connection, data, NW_CONNECTION_DEFAULT_MESSAGE_CONTEXT, true,
            ^(nw_error_t error) {
                if (error != NULL) {
                    WBDrop(conn);
                }
            });
    });
}

// Closes `conn` after everything already handed to `wb_sync_net_send` has
// gone out: the last message of an exchange (the pairing's "done", an
// unpair notice) is followed at once by a close, and a plain cancel would
// overtake it and drop it.
void wb_sync_net_close(uint64_t conn) {
    dispatch_async(WBQueue(), ^{
        NSNumber *key = @(conn);
        nw_connection_t connection = gConnections[key];
        if (connection == nil) {
            return;
        }
        [gConnections removeObjectForKey:key];
        WBEmit(WBSyncClosed, conn, NULL, 0);
        // Sends complete in order: this empty final one runs last.
        nw_connection_send(connection, NULL, NW_CONNECTION_FINAL_MESSAGE_CONTEXT, true,
            ^(__unused nw_error_t error) {
                nw_connection_cancel(connection);
            });
        // A peer that never reads must not keep the connection alive.
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC), WBQueue(), ^{
            nw_connection_cancel(connection);
        });
    });
}
