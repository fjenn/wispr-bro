# SPDX-License-Identifier: GPL-3.0-or-later
"""Summarises the meter's trace lines (`fork audio: meter at <s> s: <dB>
dB, floor <dB> dB, level <0..1>`) per segment of the WAV (make-wav.py's .json): mean
and highest level and lit bars (12 bars, none below 0.05), with PASS/FAIL
against owner test 1's expectations: steady noise near zero, speech most of
the meter and moving, silence zero. `--quiet` (OT2-B, quiet speech and
whispers): the speech only has to light and move a few bars (top > 0.2,
mean > 0.1, at least 2 bars of range), still clearly over the noise.

usage: summary.py <levels.txt> <wav.json> [--quiet]
"""
import json
import math
import re
import sys

quiet = "--quiet" in sys.argv[3:]
lines = open(sys.argv[1]).read().splitlines()
segments = json.load(open(sys.argv[2]))
rows = []
for line in lines:
    # The audio's own time (the log's clock has whole seconds only).
    m = re.search(
        r"meter at ([\d.]+) s: (-?[\d.]+) dB, floor (-?[\d.]+) dB, level ([\d.]+)", line
    )
    if not m:
        continue
    rows.append((float(m[1]), float(m[2]), float(m[3]), float(m[4])))
if not rows:
    sys.exit("FAIL: no meter readings in the log")
t0 = 0.0
lit = lambda l: 0 if l < 0.05 else min(12, math.ceil(l * 12))
ok = True
for name, (a, b) in segments.items():
    seg = [r for r in rows if a + 0.5 <= r[0] - t0 < b]
    if not seg:
        print(f"{name}: no readings")
        ok = False
        continue
    levels = [r[3] for r in seg]
    bars = [lit(l) for l in levels]
    mean, top = sum(levels) / len(levels), max(levels)
    if name.startswith("noise") or name == "silence":
        good = mean < 0.05 and sum(1 for b in bars if b > 1) <= len(bars) * 0.1
    elif quiet:
        good = top > 0.2 and mean > 0.1 and max(bars) - min(bars) >= 2
    else:
        good = top > 0.7 and mean > 0.25 and max(bars) - min(bars) >= 6
    ok &= good
    print(f"{'PASS' if good else 'FAIL'} {name:12s} {len(seg):3d} readings, level mean {mean:.2f} max {top:.2f}, "
          f"bars mean {sum(bars) / len(bars):.1f} max {max(bars)}, floor {seg[-1][2]:.1f} dB")
print("PASS" if ok else "FAIL")
sys.exit(0 if ok else 1)
