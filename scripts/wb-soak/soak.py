# SPDX-License-Identifier: GPL-3.0-or-later
"""S-38 exactly-once soak driver (R5 §5.4, S-38 "Soak harness"; Mac tests
S-38.3 and A12.1).

Per take: focus the target app's field (prepared by a person or by
`--prepare`), clear it through AX, snapshot the clipboard digest, start the
take with `<app> --toggle-transcription` (the CLI goes through the same
coordinator and `deliver` path as the hotkey), play the prerecorded WAV
into the loopback device the app uses as its microphone, stop with the same
flag, then read the field back through AX and decide the outcome:

  confirmed     the expected text is in the field exactly once
  duplicate     it is there more than once
  card          the field is unchanged and the bar shows its card
  wrong_target  the field is unchanged, no card, and the text landed in
                another app (frontmost changed) or the frontmost app at the
                stop was not the target
  lost          none of the above

plus latency (stop → field changed), whether the clipboard digest after the
take equals the one before (restored), and the frontmost app at stop and at
the end. One JSON object per take goes to `<out>/takes.jsonl`; a summary
line is printed at the end. Every take holds ~/wb-bench/gui.lock.

  python3 -I scripts/wb-soak/soak.py --plan plan.json --takes 500
          --app '<…>/Wispr Bro.app/Contents/MacOS/wispr-bro' --pid <app pid>
          --device 'BlackHole 2ch' --bin <helpers dir> --out <dir>

`plan.json`: {"apps": [{"name": "TextEdit", "bundle": "com.apple.TextEdit"},
…], "takes": [{"wav": "…/take-01.wav", "expect": "…"}, …]}. Takes cycle
through the apps (§5.4 lists Mail, Notes, TextEdit, Slack, VS Code, Chrome
Gmail, Safari, Notion, Messages, Terminal) and the WAVs. Each app must have
an empty text field focused (a scratch document, a draft, a local page);
the driver never types into anything else. `--bin` holds the compiled
helpers: `axread` and `play` (this folder), `clip` (scripts/wb-s05) and
`pill` (scripts/wb-s03-pill.swift), each `swiftc -O -o <bin>/<name> …`.

Never plays through the speakers: `--device` must be a loopback driver, and
the run stops if `play` reports another device.

**Hardware run** (`--hardware`, R5 S-38: 50 takes with a real fn key on a
real keyboard): the same apps, field clearing, read-back, card and
clipboard checks, but no CLI toggle and no WAV. Per take the driver
focuses the target, prints the sentence (`expect`), and the tester holds
fn, reads it aloud and releases; the driver waits up to `--hardware-wait`
seconds for the field to change. A person's words differ from the script,
so the outcome compares words, not the exact text: `confirmed` when the
field changed and holds the take once (≥ 50 % of the script's words, not
twice their number), `duplicate` when the field holds it twice, then card /
wrong_target / lost as above. No GUI lock is taken (the tester is at the
Mac), and `--pid`, `--device`, `play` are not used.

  python3 -I scripts/wb-soak/soak.py --hardware --plan plan.json --takes 50
          --app unused --pid 0 --bin <helpers dir> --out <dir>
"""
import argparse
import fcntl
import json
import os
import subprocess
import sys
import time
import unicodedata

LOCK = os.path.expanduser("~/wb-bench/gui.lock")
FIELD_WAIT_S = 12.0
CARD_MIN_H = 30


def run_json(cmd, env=None, timeout=60):
    out = subprocess.run(
        cmd, capture_output=True, text=True, timeout=timeout, env=env
    ).stdout
    for line in reversed(out.strip().splitlines()):
        try:
            return json.loads(line)
        except json.JSONDecodeError:
            continue
    return {}


def norm(text):
    text = unicodedata.normalize("NFC", text or "")
    return " ".join(text.replace(" ", " ").split()).casefold()


class Helpers:
    def __init__(self, bin_dir, pid):
        self.bin = bin_dir
        self.pid = pid

    def ax(self, cmd):
        return run_json([os.path.join(self.bin, "axread"), cmd])

    def clip_digest(self):
        return run_json([os.path.join(self.bin, "clip"), "digest"])

    def active_bar_height(self):
        d = run_json(
            [os.path.join(self.bin, "pill"), "-e", "windows"],
            env={**os.environ, "WB_PID": str(self.pid)},
        )
        heights = [w.get("h", 0) for w in d.get("windows", [])]
        return max([h for h in heights if h >= CARD_MIN_H] or [0])

    def play(self, device, wav):
        return run_json([os.path.join(self.bin, "play"), device, wav], timeout=180)


def toggle(app_bin):
    subprocess.run([app_bin, "--toggle-transcription"], capture_output=True, timeout=30)


def activate(bundle):
    subprocess.run(["/usr/bin/open", "-b", bundle], capture_output=True, timeout=30)


def words(text):
    return [w.strip(".,!?;:\"'()") for w in norm(text).split() if w.strip(".,!?;:\"'()")]


def spoken_outcome(value, expect):
    """Hardware run: how many times a person's take of `expect` is in
    `value` (0, 1 or 2), comparing words rather than the exact text."""
    got, want = words(value), words(expect)
    if not got or not want:
        return 0
    if len(got) >= 1.6 * len(want):
        half = len(got) // 2
        first, second = set(got[:half]), set(got[half:])
        if first and len(first & second) >= 0.5 * len(first):
            return 2
    hits = sum(1 for w in set(want) if w in set(got))
    return 1 if hits >= 0.5 * len(set(want)) else 0


def hardware_take(h, a, n, target, take):
    rec = {"n": n, "app": target["name"], "mode": "hardware"}
    activate(target["bundle"])
    time.sleep(1.0)
    front = h.ax("front").get("front")
    if front != target["bundle"]:
        rec.update(outcome="skipped", reason=f"front={front}")
        return rec
    h.ax("clear")
    before = h.ax("value").get("value", "")
    clip_before = h.clip_digest()
    print(f"\n[{n}] {target['name']}: hold fn, read aloud, release:\n    {take['expect']}", flush=True)
    t0 = time.monotonic()
    value, changed, card = before, False, False
    while time.monotonic() - t0 < a.hardware_wait:
        value = h.ax("value").get("value", "")
        if norm(value) != norm(before):
            changed = True
            time.sleep(1.5)  # a duplicate paste would follow within this
            value = h.ax("value").get("value", "")
            break
        bar = h.active_bar_height()
        if CARD_MIN_H <= bar < 80:
            card = True
        time.sleep(0.2)
    rec["front_at_end"] = h.ax("front").get("front")
    rec["front_at_stop"] = rec["front_at_end"]
    clip_after = h.clip_digest()
    count = spoken_outcome(value, take["expect"]) if changed else 0
    if count == 1:
        outcome = "confirmed"
    elif count > 1:
        outcome = "duplicate"
    elif not changed and card:
        outcome = "card"
    elif not changed and rec["front_at_end"] != target["bundle"]:
        outcome = "wrong_target"
    else:
        outcome = "lost"
    rec.update(
        outcome=outcome,
        latency_ms=None,
        clipboard_restored=clip_before.get("digest") == clip_after.get("digest"),
        field_chars=len(value),
    )
    return rec


def one_take(h, a, n, target, take):
    rec = {"n": n, "app": target["name"], "wav": os.path.basename(take["wav"])}
    activate(target["bundle"])
    time.sleep(1.0)
    front = h.ax("front").get("front")
    if front != target["bundle"]:
        rec.update(outcome="skipped", reason=f"front={front}")
        return rec
    h.ax("clear")
    before = h.ax("value").get("value", "")
    clip_before = h.clip_digest()
    toggle(a.app)
    time.sleep(0.35)
    played = h.play(a.device, take["wav"])
    if played.get("device") != a.device:
        rec.update(outcome="aborted", reason=f"play: {played}")
        return rec
    time.sleep(0.25)
    rec["front_at_stop"] = h.ax("front").get("front")
    t_stop = time.monotonic()
    toggle(a.app)

    expect = norm(take["expect"])
    value, changed_at, card = before, None, False
    while time.monotonic() - t_stop < FIELD_WAIT_S:
        value = h.ax("value").get("value", "")
        if norm(value) != norm(before):
            changed_at = time.monotonic()
            time.sleep(0.6)  # a duplicate paste would follow within this
            value = h.ax("value").get("value", "")
            break
        if h.active_bar_height() >= CARD_MIN_H and h.active_bar_height() < 80:
            card = True
        time.sleep(0.2)
    rec["front_at_end"] = h.ax("front").get("front")
    clip_after = h.clip_digest()
    count = norm(value).count(expect) if expect else 0
    if count == 1:
        outcome = "confirmed"
    elif count > 1:
        outcome = "duplicate"
    elif changed_at is None and card:
        outcome = "card"
    elif changed_at is None and (
        rec["front_at_stop"] != target["bundle"]
        or rec["front_at_end"] != target["bundle"]
    ):
        outcome = "wrong_target"
    else:
        outcome = "lost"
    rec.update(
        outcome=outcome,
        latency_ms=round((changed_at - t_stop) * 1000) if changed_at else None,
        clipboard_restored=clip_before.get("digest") == clip_after.get("digest"),
        field_chars=len(value),
    )
    return rec


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--plan", required=True)
    ap.add_argument("--takes", type=int, default=500)
    ap.add_argument("--app", required=True, help="the app's executable")
    ap.add_argument("--pid", type=int, required=True)
    ap.add_argument("--device", default="BlackHole 2ch")
    ap.add_argument("--bin", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--gap", type=float, default=2.0, help="seconds between takes")
    ap.add_argument("--hardware", action="store_true", help="a person takes each sentence with fn")
    ap.add_argument("--hardware-wait", type=float, default=60.0)
    a = ap.parse_args()

    plan = json.load(open(a.plan))
    apps, takes = plan["apps"], plan["takes"]
    os.makedirs(a.out, exist_ok=True)
    h = Helpers(a.bin, a.pid)
    tally = {}
    latencies = []
    with open(os.path.join(a.out, "takes.jsonl"), "a") as sink, open(LOCK, "a") as lock:
        for n in range(1, a.takes + 1):
            target = apps[(n - 1) % len(apps)]
            take = takes[(n - 1) % len(takes)]
            if a.hardware:
                rec = hardware_take(h, a, n, target, take)
            else:
                fcntl.flock(lock, fcntl.LOCK_EX)
                try:
                    rec = one_take(h, a, n, target, take)
                finally:
                    fcntl.flock(lock, fcntl.LOCK_UN)
            sink.write(json.dumps(rec) + "\n")
            sink.flush()
            tally[rec["outcome"]] = tally.get(rec["outcome"], 0) + 1
            if rec.get("latency_ms") is not None:
                latencies.append(rec["latency_ms"])
            if rec["outcome"] == "aborted":
                break
            time.sleep(a.gap)

    latencies.sort()

    def pct(p):
        return latencies[min(len(latencies) - 1, int(p * len(latencies)))] if latencies else None

    summary = {
        "takes": sum(tally.values()),
        "outcomes": tally,
        "latency_ms": {"p50": pct(0.5), "p95": pct(0.95)},
        "pass": all(tally.get(k, 0) == 0 for k in ("lost", "duplicate", "wrong_target", "aborted")),
    }
    print(json.dumps(summary))
    return 0 if summary["pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
