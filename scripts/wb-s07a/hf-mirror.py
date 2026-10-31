# SPDX-License-Identifier: GPL-3.0-or-later
"""Local stand-in for Hugging Face's resolve endpoint (S-07a real-Mac checks).

Serves `/<org>/<repo>/resolve/<revision>/<file>` from files found under
--root (e.g. ~/wb-bench/asr), with the headers hf-hub reads (x-repo-commit,
etag, x-linked-size, Content-Range), throttled to --rate bytes per second
(one link shared by all connections), so a debug build started with HF_ENDPOINT=http://127.0.0.1:<port>
downloads at 10 or 50 Mbit/s without touching the network.

GET /_rate/<bytes per second> changes the link speed (0 = unthrottled).
Modes (switch at runtime with GET /_mode/<mode>[/<retry_after>]):
  normal    serve the file
  captive   answer every request with a Wi-Fi login page (200 text/html)
  429       answer 429 with Retry-After: <retry_after>
  drop      close the connection after the headers (a reset mid-transfer)
  corrupt   serve the file with byte 4096 flipped (a bad checksum)

Loopback only. Logs one line per request (path and range; no user data).
Run: python3 -I scripts/wb-s07a/hf-mirror.py --root ~/wb-bench/asr --port 18077 --rate 6250000
"""

import argparse
import hashlib
import os
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

STATE = {"mode": "normal", "retry_after": 20}
LOCK = threading.Lock()
FILES = {}


def find(root, name):
    if name in FILES:
        return FILES[name]
    for dirpath, _dirs, files in os.walk(root):
        if name in files:
            FILES[name] = os.path.join(dirpath, name)
            return FILES[name]
    return None


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):
        sys.stderr.write("%.3f %s\n" % (time.time(), fmt % args))
        sys.stderr.flush()

    def do_HEAD(self):
        self.handle_get(head=True)

    def do_GET(self):
        self.handle_get(head=False)

    def handle_get(self, head):
        path = self.path.split("?")[0]
        if path.startswith("/_rate/"):
            self.server.rate = int(path.split("/")[2])
            self.reply(200, b"ok\n", "text/plain")
            return
        if path.startswith("/_mode/"):
            parts = path.split("/")
            with LOCK:
                STATE["mode"] = parts[2]
                if len(parts) > 3 and parts[3]:
                    STATE["retry_after"] = int(parts[3])
            self.reply(200, b"ok\n", "text/plain")
            return
        with LOCK:
            mode = STATE["mode"]
            retry_after = STATE["retry_after"]
        if mode == "captive":
            self.reply(200, b"<html><body>Log in to the guest Wi-Fi</body></html>", "text/html")
            return
        if mode == "429":
            self.reply(429, b"", "text/plain", {"Retry-After": str(retry_after)})
            return
        segs = path.strip("/").split("/")
        if len(segs) < 5 or segs[2] != "resolve":
            self.reply(404, b"not found\n", "text/plain")
            return
        revision, name = segs[3], segs[-1]
        file = find(self.server.root, name)
        if not file:
            self.reply(404, b"not found\n", "text/plain")
            return
        size = os.path.getsize(file)
        start, stop = 0, size - 1
        rng = self.headers.get("Range")
        if rng and rng.startswith("bytes="):
            a, _, b = rng[6:].partition("-")
            start = int(a or 0)
            stop = min(int(b), size - 1) if b else size - 1
        length = stop - start + 1
        self.send_response(206 if rng else 200)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Length", str(length))
        self.send_header("Content-Range", "bytes %d-%d/%d" % (start, stop, size))
        self.send_header("x-repo-commit", revision)
        self.send_header("etag", '"%s"' % self.server.etag(file))
        self.send_header("x-linked-size", str(size))
        self.send_header("Accept-Ranges", "bytes")
        self.end_headers()
        if head:
            return
        if mode == "drop":
            self.close_connection = True
            return
        rate = self.server.rate
        with open(file, "rb") as f:
            f.seek(start)
            left = length
            chunk = 64 * 1024
            while left > 0:
                with LOCK:
                    if STATE["mode"] == "drop":
                        self.close_connection = True
                        return
                pos = f.tell()
                data = f.read(min(chunk, left))
                if not data:
                    break
                if mode == "corrupt" and pos <= 4096 < pos + len(data):
                    i = 4096 - pos
                    data = data[:i] + bytes([data[i] ^ 0xFF]) + data[i + 1 :]
                try:
                    self.wfile.write(data)
                except (BrokenPipeError, ConnectionResetError):
                    return
                left -= len(data)
                if rate:
                    # One link shared by every connection (hf-hub opens 4).
                    with LOCK:
                        now = time.monotonic()
                        slot = max(now, STATE.get("next_free", now))
                        STATE["next_free"] = slot + len(data) / rate
                    if slot > now:
                        time.sleep(slot - now)

    def reply(self, status, body, ctype, extra=None):
        self.send_response(status)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        for k, v in (extra or {}).items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)


class Server(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, addr, root, rate):
        super().__init__(addr, Handler)
        self.root = root
        self.rate = rate
        self._etags = {}

    def etag(self, file):
        if file not in self._etags:
            h = hashlib.sha256()
            with open(file, "rb") as f:
                for block in iter(lambda: f.read(1 << 22), b""):
                    h.update(block)
            self._etags[file] = h.hexdigest()
        return self._etags[file]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", required=True)
    ap.add_argument("--port", type=int, default=18077)
    ap.add_argument("--rate", type=int, default=0, help="bytes per second, shared by all connections (0 = unthrottled)")
    ap.add_argument("--mode", default="normal")
    a = ap.parse_args()
    STATE["mode"] = a.mode
    srv = Server(("127.0.0.1", a.port), os.path.expanduser(a.root), a.rate)
    sys.stderr.write("hf-mirror on 127.0.0.1:%d rate=%d mode=%s\n" % (a.port, a.rate, a.mode))
    sys.stderr.flush()
    srv.serve_forever()


if __name__ == "__main__":
    main()
