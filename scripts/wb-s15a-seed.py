# SPDX-License-Identifier: GPL-3.0-or-later
"""Seed an S-15a test profile's Handy history.db with neutral rows.

Usage: python3 -I scripts/wb-s15a-seed.py <Data dir>  (app closed)

Rows: 30 dictations over the last 5 local days (so the streak is 5), each with
a silent 16 kHz WAV of a known length so the backfill finds speech time
(words per minute). Text is generic sample prose, no personal data.
"""
import os
import sqlite3
import sys
import time
import wave

data = sys.argv[1]
db = sqlite3.connect(os.path.join(data, "history.db"))
rec = os.path.join(data, "recordings")
os.makedirs(rec, exist_ok=True)

SENTENCES = [
    "Please send the quarterly report to the team by Friday afternoon.",
    "Remind me to water the plants and call the plumber tomorrow morning.",
    "The meeting moved to Thursday at three in the small conference room.",
    "Thanks for the update, I will review the draft and reply tonight.",
    "Add milk, eggs, bread and coffee beans to the shopping list.",
    "Can we push the design review to next week so everyone can attend?",
]

now = time.time()
day = 86400
rows = 0
for d in range(5):  # today and the four days before
    for i in range(6):
        ts = int(now - d * day - i * 600 - 60)
        text = SENTENCES[(d + i) % len(SENTENCES)]
        name = f"wispr-bro-{ts * 1000 + i}.wav"
        secs = 4 + (i % 3)  # 4-6 s of "speech"
        with wave.open(os.path.join(rec, name), "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(16000)
            w.writeframes(b"\x00\x00" * 16000 * secs)
        db.execute(
            "INSERT INTO transcription_history (file_name, timestamp, saved, title, transcription_text, post_process_requested) VALUES (?, ?, 0, ?, ?, 0)",
            (name, ts, time.strftime("%B %d, %Y - %l:%M%p", time.localtime(ts)), text),
        )
        rows += 1
db.commit()
print(f"seeded {rows} rows")
