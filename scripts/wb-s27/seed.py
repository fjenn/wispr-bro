# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-27: seed a test profile for export, diagnostics and delete all.

usage: python3 -I scripts/wb-s27/seed.py <Data dir> <out.json>

Run with the app closed, after one launch created both databases. Writes,
with neutral made-up content only:

- three History rows into Handy's `history.db` (made today, so retention
  keeps them), each with a short silent WAV in `recordings/`;
- a fake downloaded model in `huggingface/hub/` and `models/`, so "keep
  models" can be checked without a real download;
- a log line carrying a private marker phrase (`Transcript: <marker>`) and a
  crash file, so the diagnostics checks can prove the marker is redacted.

<out.json> lists what verify.py expects. The <Data dir> must be a scratch
test profile: its path has to contain `s27`.
"""
import datetime as dt
import json
import os
import sqlite3
import struct
import sys
import time

data, out = sys.argv[1:3]
if "s27" not in data:
    sys.exit("refusing: not the S-27 test profile")
history = os.path.join(data, "history.db")
if not os.path.isfile(history) or not os.path.isfile(os.path.join(data, "fork.db")):
    sys.exit("launch the app once first (no history.db / fork.db yet)")

MARKER = "zebra lantern quartz"  # never appears in exports of logs
TEXTS = [
    "Please move the planning call to Thursday afternoon.",
    "The draft is ready for review, thanks.",
    "Buy oat milk, coffee filters and batteries.",
]


def silent_wav(path, ms=500, rate=16000):
    frames = rate * ms // 1000
    body = b"\x00\x00" * frames
    header = b"RIFF" + struct.pack("<I", 36 + len(body)) + b"WAVEfmt "
    header += struct.pack("<IHHIIHH", 16, 1, 1, rate, rate * 2, 2, 16)
    header += b"data" + struct.pack("<I", len(body))
    with open(path, "wb") as f:
        f.write(header + body)


os.makedirs(os.path.join(data, "recordings"), exist_ok=True)
now = int(time.time())
rows = []
con = sqlite3.connect(history)
for i, text in enumerate(TEXTS):
    ts = now - 60 * (len(TEXTS) - i)
    name = f"wispr-bro-s27-{i + 1}.wav"
    silent_wav(os.path.join(data, "recordings", name))
    title = dt.datetime.fromtimestamp(ts).strftime("%B %d, %Y - %H:%M")
    cur = con.execute(
        "INSERT INTO transcription_history (file_name, timestamp, saved, title,"
        " transcription_text, post_processed_text, post_process_requested, language)"
        " VALUES (?, ?, 0, ?, ?, ?, 0, 'en')",
        (name, ts, title, text, text),
    )
    rows.append({"id": cur.lastrowid, "file_name": name, "text": text})
con.commit()
con.close()

for rel in ("huggingface/hub/models--s27--fake/blobs/fake.gguf", "models/s27-fake.bin"):
    path = os.path.join(data, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(b"s27 fake model")

logs = os.path.join(data, "logs")
os.makedirs(logs, exist_ok=True)
with open(os.path.join(logs, "wispr-bro.log"), "a") as f:
    f.write(f"[s27 seed] Transcript: {MARKER}\n")
stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
with open(os.path.join(logs, f"crash-{stamp}-s27.txt"), "w") as f:
    f.write(f"s27 seeded crash\nPrompt: {MARKER}\nhome {os.path.expanduser('~')}/Documents\n")

with open(out, "w") as f:
    json.dump({"rows": rows, "marker": MARKER,
               "kept_models": ["huggingface/hub/models--s27--fake/blobs/fake.gguf",
                               "models/s27-fake.bin"]}, f, indent=1)
print(f"seeded {len(rows)} rows, 2 fake models, a log and a crash file -> {out}")
