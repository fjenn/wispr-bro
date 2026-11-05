# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-15b: seed a portable test profile with synthetic history.

usage: python3 -I scripts/wb-s15b/seed.py <Data dir> [bulk N]

Writes made-up dictations (no personal data) into the test profile's
Handy `history.db` and the matching `fork.db` rows, as the S-09 writer
would have, so the launch reconcile keeps them. Run it with the app
closed, after one launch created both databases.

Each seeded dictation gets a real recording: macOS `say` renders the
heard text to a file (never through the speakers) and `afconvert` makes
the 16 kHz mono WAV Handy writes, so Play, Save audio as… and Retry work on
real audio.

- yesterday 18:10  "Please send the invoice to Sioban by Friday." (asked for
  the Flag check: "Sioban" is the wrong spelling)
- yesterday 23:59 and today 00:01 (the midnight boundary)
- today: an AI-cleaned take (raw "so um the meeting is at 2 actually 3"
  → "The meeting is at 3."), a plain take, a file transcription
  ("Weekly sync.m4a"), and one take whose audio expired (no WAV,
  `audio_deleted_at` set 15 days after it).
- `bulk N`: N more text-only rows over the past days (scale check).
"""
import datetime as dt
import os
import sqlite3
import subprocess
import sys
import tempfile
import time

data = sys.argv[1]
if "/Data" not in data or "s15b" not in data:
    sys.exit("refusing: not the S-15b portable test profile")
bulk = int(sys.argv[3]) if len(sys.argv) > 3 and sys.argv[2] == "bulk" else 0

rec = os.path.join(data, "recordings")
os.makedirs(rec, exist_ok=True)
hdb = sqlite3.connect(os.path.join(data, "history.db"))
fdb = sqlite3.connect(os.path.join(data, "fork.db"))
now = dt.datetime.now()
today = now.date()
yesterday = today - dt.timedelta(days=1)


def wav(text, name):
    with tempfile.TemporaryDirectory() as tmp:
        aiff = os.path.join(tmp, "t.aiff")
        subprocess.run(["/usr/bin/say", "-o", aiff, text], check=True)
        subprocess.run(["/usr/bin/afconvert", "-f", "WAVE", "-d", "LEI16@16000",
                        "-c", "1", aiff, os.path.join(rec, name)], check=True)
    # 16 kHz, 16-bit mono: bytes after the header / 32 per ms.
    return max(0, (os.path.getsize(os.path.join(rec, name)) - 44) // 32)


def add(local, final, raw=None, source_name=None, audio=True, expired=False,
        app=("com.apple.TextEdit", "TextEdit")):
    ts_ms = int(local.timestamp() * 1000)
    file_name = f"wispr-bro-{ts_ms}.wav"
    heard = raw or final
    audio_ms = wav(heard, file_name) if audio and not expired else 0
    cur = hdb.execute(
        "INSERT INTO transcription_history (file_name, timestamp, saved, title,"
        " transcription_text, post_processed_text, post_process_requested, language,"
        " source_name) VALUES (?, ?, 0, ?, ?, ?, 0, 'en', ?)",
        (file_name, ts_ms // 1000, local.strftime("%B %d, %Y - %H:%M"),
         raw or final, final if raw else None, source_name),
    )
    entry_id = cur.lastrowid
    words = len(final.split())
    source = "file" if source_name else "dictation"
    counted = 1 if source == "dictation" else 0
    deleted_at = ts_ms + 15 * 86_400_000 if expired else None
    fdb.execute(
        "INSERT INTO entry_meta (entry_id, created_at, asr_text, final_text, audio_ms,"
        " word_count, char_count, language, app_bundle_id, app_name, style_group,"
        " usage_category, cleanup_level, cleanup_engine, cleanup_outcome, paste_outcome,"
        " source, day, audio_file, counted, audio_deleted_at)"
        " VALUES (?,?,?,?,?,?,?,'en',?,?,'other','documents','light',?,?,'confirmed',"
        " ?,?,?,?,?)",
        (entry_id, ts_ms, heard, final, audio_ms, words, len(final), app[0], app[1],
         "local" if raw else "none", "applied" if raw else "skipped", source,
         local.date().isoformat(), file_name, counted, deleted_at),
    )
    fdb.execute("INSERT INTO history_fts (rowid, final_text) VALUES (?, ?)",
                (entry_id, final))
    if counted:
        day = local.date().isoformat()
        fdb.execute(
            "INSERT INTO daily_stats (day, words, dictations, speech_ms, timed_words)"
            " VALUES (?, ?, 1, ?, ?) ON CONFLICT (day) DO UPDATE SET"
            " words = words + excluded.words, dictations = dictations + 1,"
            " speech_ms = speech_ms + excluded.speech_ms,"
            " timed_words = timed_words + excluded.timed_words",
            (day, words, audio_ms, words if audio_ms else 0))
        fdb.execute(
            "INSERT INTO daily_app_stats (day, usage_category, words, dictations)"
            " VALUES (?, 'documents', ?, 1) ON CONFLICT (day, usage_category) DO UPDATE"
            " SET words = words + excluded.words, dictations = dictations + 1",
            (day, words))
    return entry_id


def at(day, h, m):
    return dt.datetime(day.year, day.month, day.day, h, m)


ids = {}
for i in range(bulk):
    day = today - dt.timedelta(days=2 + i // 40)
    add(at(day, 8 + (i % 40) // 4, (i * 7) % 60),
        f"Bulk note number {i} about the quarterly plan.", audio=False)
ids["expired"] = add(at(today - dt.timedelta(days=16), 9, 0),
                     "An old note whose audio was removed.", expired=True)
ids["flag"] = add(at(yesterday, 18, 10), "Please send the invoice to Sioban by Friday.")
ids["late"] = add(at(yesterday, 23, 59), "Last note before midnight.")
ids["early"] = add(at(today, 0, 1), "First note after midnight.")


def earlier(minutes):
    """`minutes` before now, but still today after 00:01."""
    return max(now - dt.timedelta(minutes=minutes), at(today, 0, 2))


ids["file"] = add(earlier(30), "Notes from the weekly sync.",
                  source_name="Weekly sync.m4a")
ids["plain"] = add(earlier(15), "The plain take for delete and undo.")
ids["ai"] = add(earlier(5), "The meeting is at 3.",
                raw="so um the meeting is at 2 actually 3")
hdb.commit()
fdb.commit()
print(ids)
