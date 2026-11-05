#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""S-38 startup timing (R5 §5.3): launch to hotkey ready and bar visible,
<= 2 s p50 and <= 3 s p95 on Apple Silicon.

Launches the app N times and reads the line `fork::startup` logs once the
fork's hotkey tap runs and the resting bar is on screen:

    startup: ready 612 ms after launch (hotkeys 430 ms, bar 612 ms)

The times are measured by the app from the kernel's process start
(`proc_pidinfo`), so dyld, Tauri and WebKit set-up count. The app is then
quit (SIGTERM) and the next launch waits `--gap` seconds. The first launch
after a build is reported separately as "cold" (the dyld and Metal shader
caches are empty then; macOS keeps them across launches afterwards).

    python3 -I scripts/wb-perf/startup.py --app <dir with wispr-bro (or handy) + portable> \
        [--runs 10] [--timeout 20] [--gap 3]
    python3 -I scripts/wb-perf/startup.py --app <Wispr Bro.app> ...

A portable test profile (a `portable` file next to the binary, `Data/`
beside it) keeps the real ~/Library untouched. The profile must have
finished onboarding (the resting bar shows only then, DEC-38), and the
binary needs Accessibility/Input Monitoring for its tap: without them the
tap never runs and the run reports `no ready line`. It puts the bar on
screen, so on a shared Mac run it under the GUI lock:

    /usr/bin/lockf -k ~/wb-bench/gui.lock python3 -I scripts/wb-perf/startup.py ...

Nothing is spoken or played. Exit code 1 when the budget is missed.
"""

import argparse
import os
import plistlib
import re
import signal
import subprocess
import sys
import time

READY = re.compile(r"startup: ready (\d+) ms after launch \(hotkeys (\d+) ms, bar (\d+) ms\)")
BUDGET_P50_MS = 2000
BUDGET_P95_MS = 3000


def percentile(values, p):
    v = sorted(values)
    return v[min(len(v) - 1, round((len(v) - 1) * p))] if v else None


def binary_and_log(app):
    """The executable and the log file of a portable dir or a bundle."""
    if app.endswith(".app"):
        with open(os.path.join(app, "Contents", "Info.plist"), "rb") as f:
            exe = plistlib.load(f)["CFBundleExecutable"]
        binary = os.path.join(app, "Contents", "MacOS", exe)
    else:
        # `tauri build --no-bundle` writes `wispr-bro` (mainBinaryName); a
        # plain `cargo build` the crate name `handy`. The newest one wins
        # (scripts/wb-lib/app.sh does the same for the shell drivers).
        found = [os.path.join(app, n) for n in ("wispr-bro", "handy")]
        found = [f for f in found if os.access(f, os.X_OK)]
        binary = max(found, key=os.path.getmtime) if found else os.path.join(app, "handy")
    data = os.path.join(os.path.dirname(binary), "Data")
    if not os.path.exists(os.path.join(os.path.dirname(binary), "portable")):
        sys.exit("no `portable` marker next to the binary: refusing to touch the real profile")
    return binary, os.path.join(data, "logs", "wispr-bro.log")


def log_size(path):
    try:
        return os.path.getsize(path)
    except OSError:
        return 0


def one_launch(binary, log, timeout):
    start = log_size(log)
    proc = subprocess.Popen(
        [binary, "--start-hidden"],
        cwd=os.path.dirname(binary),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    found = None
    end = time.monotonic() + timeout
    while time.monotonic() < end and found is None:
        time.sleep(0.2)
        try:
            with open(log, "r", errors="replace") as f:
                f.seek(start)
                m = READY.search(f.read())
        except OSError:
            m = None
        if m:
            found = tuple(int(x) for x in m.groups())
        if proc.poll() is not None:
            break
    proc.send_signal(signal.SIGTERM)
    try:
        proc.wait(10)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()
    return found


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--app", required=True)
    ap.add_argument("--runs", type=int, default=10)
    ap.add_argument("--timeout", type=float, default=20)
    ap.add_argument("--gap", type=float, default=3)
    a = ap.parse_args()
    binary, log = binary_and_log(os.path.abspath(a.app))
    results = []
    for i in range(a.runs + 1):
        r = one_launch(binary, log, a.timeout)
        label = "cold" if i == 0 else f"run {i}"
        if r is None:
            print(f"{label}: no ready line within {a.timeout:.0f} s")
        else:
            print(f"{label}: ready {r[0]} ms (hotkeys {r[1]} ms, bar {r[2]} ms)")
            if i > 0:
                results.append(r[0])
        time.sleep(a.gap)
    if not results:
        print("FAIL: no launch reported ready")
        return 1
    p50, p95 = percentile(results, 0.5), percentile(results, 0.95)
    ok = len(results) == a.runs and p50 <= BUDGET_P50_MS and p95 <= BUDGET_P95_MS
    print(
        f"{'PASS' if ok else 'FAIL'}: launch → hotkey ready and bar visible "
        f"p50 {p50} ms, p95 {p95} ms over {len(results)}/{a.runs} warm launches "
        f"(budget {BUDGET_P50_MS}/{BUDGET_P95_MS} ms)"
    )
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
