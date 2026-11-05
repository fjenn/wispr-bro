# SPDX-License-Identifier: GPL-3.0-or-later
"""S-38 idle memory and CPU sampler (R5 §5.2; Mac tests S-38.2, A12.2).

Samples the running app and the WebKit processes it is responsible for
(`wb-s03-pill.swift procs`, the S-03 method) every `--every` seconds for
`--minutes`, then prints one JSON summary line: total RSS (median / max /
last), the app's CPU % over the window, and the processes at the end. It
posts no events and needs no GUI lock: it only reads `ps`.

  python3 -I scripts/wb-soak/idle.py --pid <app pid> [--minutes 10]
          [--every 30] [--pill <compiled wb-s03-pill>] [--out samples.jsonl]

`--pill` is `swiftc -O -o <bin> scripts/wb-s03-pill.swift` (faster than
`swift` per sample). With `--footprint`, the summary adds each process's
physical footprint from `/usr/bin/footprint` (what Activity Monitor shows;
RSS also counts shared framework pages).

§5.2 ceilings: total RSS ≤ 250 MB, idle CPU ≤ 0.5 % averaged over 10 min.
"""
import argparse
import json
import os
import re
import statistics
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
PILL = os.path.join(HERE, "..", "wb-s03-pill.swift")
RSS_CEILING_MB = 250.0
CPU_CEILING_PCT = 0.5


def procs(pid, pill):
    cmd = [pill] if pill else ["swift", PILL]
    out = subprocess.run(
        cmd + ["-e", "procs"],
        env={**os.environ, "WB_PID": str(pid)},
        capture_output=True,
        text=True,
        timeout=120,
    ).stdout
    for line in out.splitlines():
        d = json.loads(line)
        if "procs" in d:
            return d
    raise RuntimeError("no procs line: " + out[:200])


def footprint_mb(pid):
    try:
        out = subprocess.run(
            ["/usr/bin/footprint", "-p", str(pid)],
            capture_output=True,
            text=True,
            timeout=60,
        ).stdout
    except (OSError, subprocess.TimeoutExpired):
        return None
    m = re.search(r"Footprint:\s*([\d.]+)\s*([KMG]B)", out)
    if not m:
        return None
    value, unit = float(m.group(1)), m.group(2)
    return value * {"KB": 1 / 1024, "MB": 1, "GB": 1024}[unit]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pid", type=int, required=True)
    ap.add_argument("--minutes", type=float, default=10)
    ap.add_argument("--every", type=float, default=30)
    ap.add_argument("--pill", default=os.environ.get("WB_PILL_BIN"))
    ap.add_argument("--out")
    ap.add_argument("--footprint", action="store_true")
    ap.add_argument("--label", default="")
    a = ap.parse_args()

    sink = open(a.out, "a") if a.out else None
    start = time.monotonic()
    samples = []
    while True:
        d = procs(a.pid, a.pill)
        d["t_s"] = round(time.monotonic() - start, 1)
        samples.append(d)
        if sink:
            sink.write(json.dumps(d) + "\n")
            sink.flush()
        if time.monotonic() - start >= a.minutes * 60:
            break
        time.sleep(a.every)

    totals = [s["total_rss_kb"] / 1024 for s in samples]

    def app_cpu(s):
        return next((p["cpu_s"] for p in s["procs"] if p["pid"] == a.pid), 0.0)

    span = samples[-1]["t_s"] - samples[0]["t_s"]
    cpu_pct = (app_cpu(samples[-1]) - app_cpu(samples[0])) / span * 100 if span else None
    last = samples[-1]["procs"]
    summary = {
        "label": a.label,
        "samples": len(samples),
        "minutes": round(span / 60, 2),
        "total_rss_mb": {
            "median": round(statistics.median(totals), 1),
            "max": round(max(totals), 1),
            "last": round(totals[-1], 1),
        },
        "app_cpu_pct": round(cpu_pct, 3) if cpu_pct is not None else None,
        "processes": [
            {"name": p["name"], "pid": p["pid"], "rss_mb": round(p["rss_kb"] / 1024, 1)}
            for p in last
        ],
        "rss_ceiling_ok": totals[-1] <= RSS_CEILING_MB,
        "cpu_ceiling_ok": cpu_pct is not None and cpu_pct <= CPU_CEILING_PCT,
    }
    if a.footprint:
        for p in summary["processes"]:
            p["footprint_mb"] = footprint_mb(p["pid"])
    print(json.dumps(summary))
    return 0


if __name__ == "__main__":
    sys.exit(main())
