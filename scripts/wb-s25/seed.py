# SPDX-License-Identifier: GPL-3.0-or-later
"""Mac test S-25: seed a portable test profile with synthetic history.

usage: python3 -I scripts/wb-s25/seed.py <Data dir> locked|unlock <out.json>

Writes neutral, made-up dictations (no personal data, no audio) into the
test profile's Handy `history.db` and the matching `fork.db` rows
(`entry_meta`, `daily_stats`, `daily_app_stats`, `corrections`) exactly as
the S-09 writer would have, so the launch reconcile keeps them. Run it with
the app closed, after one launch created both databases.

- `locked`: about 1,100 words over 15 of the last 21 local days (streak 4
  ending today, longest run 8), five apps in five usage categories. Below 1,700: Voice is locked.
- `unlock`: 75 more dictations today, crossing 1,700.

It then computes the numbers the Insights page must show independently of
the app's Rust code (plain Python over the seeded rows) and writes them to
<out.json>.
"""
import datetime as dt
import json
import os
import re
import sqlite3
import sys
import time

data, phase, out = sys.argv[1:4]
if "/Data" not in data or "s25" not in data:
    sys.exit("refusing: not the S-25 portable test profile")
assert phase in ("locked", "unlock")

HERE = os.path.dirname(os.path.abspath(__file__))
STOP = os.path.join(HERE, "..", "..", "src-tauri", "src", "fork", "stats",
                    "tables", "stopwords-iso.json")
MARKERS = os.path.join(HERE, "..", "..", "src-tauri", "src", "fork", "llm",
                       "tables", "markers.json")

APPS = {
    "slack": ("com.tinyspeck.slackmacgap", "Slack", "work", "work"),
    "mail": ("com.apple.mail", "Mail", "email", "email"),
    "notes": ("com.apple.Notes", "Notes", "other", "documents"),
    "chat": ("com.openai.chat", "ChatGPT", "other", "ai_prompts"),
    "sms": ("com.apple.MobileSMS", "Messages", "personal", "personal"),
}

# (app, hour, final text, heard text or None)
SLACK = [
    "The roadmap draft is in the shared folder. Sounds good to me.",
    "Can you add the roadmap dates to the board? Sounds good to me.",
    "I moved the standup to eleven. Sounds good to me.",
    "Ship it after the review. Sounds good to me.",
    "The roadmap looks fine from my side.",
    "Please check the roadmap before lunch.",
]
MAIL = [
    "Thanks for the update, I will send the roadmap notes tomorrow.",
    "Hi Sam, attached is the invoice for October.",
    "I am going to be out on Friday, back on Monday.",
]
MAIL_HEARD = {2: "I am gonna be out on Friday, back on Monday."}
NOTES = [
    "The Kubernetes rollout plan needs one more review.",
    "Kubernetes upgrade checklist for the staging cluster.",
]
NOTES_HEARD = {
    0: "The cooper netties rollout plan needs one more review.",
    1: "cooper netties upgrade checklist for the staging cluster.",
}
CHAT = ["Summarize the roadmap risks in three short bullet points."]
SMS = ["Running a bit late, see you at the station soon."]


def wc(text):
    return len(re.findall(r"[\w']+", text))


def rows_for_day(offset, n, extra=False):
    """Dictations for one day: a mix weighted to Slack in the morning."""
    rows = []
    for i in range(n):
        k = (offset * 7 + i + (100 if extra else 0))
        pick = k % 10
        if pick < 5:
            rows.append(("slack", 10, SLACK[k % len(SLACK)], None))
        elif pick < 7:
            j = k % len(MAIL)
            rows.append(("mail", 15, MAIL[j], MAIL_HEARD.get(j)))
        elif pick < 8:
            j = k % len(NOTES)
            rows.append(("notes", 9, NOTES[j], NOTES_HEARD.get(j)))
        elif pick < 9:
            rows.append(("chat", 14, CHAT[0], None))
        else:
            rows.append(("sms", 19, SMS[0], None))
    return rows


today = dt.date.today()
if phase == "locked":
    # Active days: 0-3 (streak 4), 5-12 (run of 8), 15, 18, 20.
    plan = {0: 5, 1: 9, 2: 8, 3: 7}
    plan.update({d: 9 for d in range(5, 13)})
    plan.update({15: 6, 18: 5, 20: 4})
    batches = [(d, rows_for_day(d, n)) for d, n in sorted(plan.items())]
else:
    batches = [(0, rows_for_day(0, 75, extra=True))]

hdb = sqlite3.connect(os.path.join(data, "history.db"))
fdb = sqlite3.connect(os.path.join(data, "fork.db"))
now_ms = int(time.time() * 1000)
seq = 0
for offset, rows in batches:
    day = today - dt.timedelta(days=offset)
    for i, (app, hour, final, heard) in enumerate(rows):
        seq += 1
        minute = (i * 4 + seq) % 50
        local = dt.datetime(day.year, day.month, day.day, hour, minute, seq % 60)
        if offset == 0 and local.timestamp() * 1000 > now_ms:
            local = dt.datetime.now() - dt.timedelta(minutes=len(rows) - i)
        ts_ms = int(local.timestamp() * 1000) + seq
        words = wc(final)
        speech_ms = words * 400  # 150 words per minute
        bundle, name, group, cat = APPS[app]
        file_name = f"wispr-bro-{ts_ms}.wav"
        cur = hdb.execute(
            "INSERT INTO transcription_history (file_name, timestamp, saved, title,"
            " transcription_text, post_processed_text, post_process_requested, language)"
            " VALUES (?, ?, 0, ?, ?, NULL, 0, 'en')",
            (file_name, ts_ms // 1000, local.strftime("%B %d, %Y - %H:%M"), final),
        )
        entry_id = cur.lastrowid
        rule_hits = 1 if heard and "gonna" in heard else 0
        llm_fixes = 1 if heard and "cooper" in heard else 0
        fdb.execute(
            "INSERT INTO entry_meta (entry_id, created_at, asr_text, final_text, audio_ms,"
            " word_count, char_count, language, app_bundle_id, app_name, style_group,"
            " usage_category, cleanup_level, cleanup_engine, cleanup_outcome,"
            " fix_count_dictionary, fix_count_llm, paste_outcome, source, day, audio_file,"
            " counted, audio_deleted_at)"
            " VALUES (?,?,?,?,?,?,?,'en',?,?,?,?,'light','local','applied',?,?,"
            " 'confirmed','dictation',?,?,1,?)",
            (entry_id, ts_ms, heard or final, final, speech_ms, words, len(final),
             bundle, name, group, cat, rule_hits, llm_fixes, day.isoformat(),
             file_name, ts_ms),
        )
        fdb.execute(
            "INSERT INTO history_fts (rowid, final_text) VALUES (?, ?)",
            (entry_id, final),
        )
        if rule_hits:
            fdb.execute(
                "INSERT INTO corrections (entry_id, from_text, to_text, source, created_at)"
                " VALUES (?, 'gonna', 'going to', 'rule', ?)",
                (entry_id, ts_ms),
            )
        fdb.execute(
            "INSERT INTO daily_stats (day, words, dictations, speech_ms, timed_words,"
            " fixes_dictionary, fixes_llm) VALUES (?, ?, 1, ?, ?, ?, ?)"
            " ON CONFLICT (day) DO UPDATE SET words = words + excluded.words,"
            " dictations = dictations + 1, speech_ms = speech_ms + excluded.speech_ms,"
            " timed_words = timed_words + excluded.timed_words,"
            " fixes_dictionary = fixes_dictionary + excluded.fixes_dictionary,"
            " fixes_llm = fixes_llm + excluded.fixes_llm",
            (day.isoformat(), words, speech_ms, words, rule_hits, llm_fixes),
        )
        fdb.execute(
            "INSERT INTO daily_app_stats (day, usage_category, words, dictations)"
            " VALUES (?, ?, ?, 1) ON CONFLICT (day, usage_category) DO UPDATE SET"
            " words = words + excluded.words, dictations = dictations + 1",
            (day.isoformat(), cat, words),
        )
hdb.commit()
fdb.commit()

# ---- expected values, computed here from the tables (not by the app) ----
tot = fdb.execute(
    "SELECT SUM(words), SUM(dictations), SUM(speech_ms), SUM(timed_words),"
    " SUM(fixes_dictionary), SUM(fixes_llm) FROM daily_stats").fetchone()
words, dictations, speech_ms, timed, fd, fl = tot
wpm = timed * 60000 / speech_ms
cats = dict(fdb.execute(
    "SELECT usage_category, SUM(words) FROM daily_app_stats GROUP BY usage_category"))
cat_total = sum(cats.values())
days = dict(fdb.execute("SELECT day, words FROM daily_stats WHERE dictations > 0"))
active = sorted(dt.date.fromisoformat(d) for d in days)
cur = 0
d = today if today in active else today - dt.timedelta(days=1)
while d in active:
    cur += 1
    d -= dt.timedelta(days=1)
longest = run = 0
prev = None
for d in active:
    run = run + 1 if prev and (d - prev).days == 1 else 1
    longest = max(longest, run)
    prev = d


def band(w):
    return sum(w >= f for f in (1, 250, 500, 750))


stop = set(json.load(open(STOP))["en"])
mk = json.load(open(MARKERS))["languages"]
fill = {p for t in mk.values() for p in t["hesitations"] + t["fillers"] if " " not in p}
texts = [r[0] for r in fdb.execute(
    "SELECT final_text FROM entry_meta WHERE counted = 1 AND deleted_at IS NULL")]
counts = {}
for t in texts:
    for w in re.findall(r"[\w']+", t.lower()):
        if len(w) < 2 or w.isdigit() or w in stop or w in fill:
            continue
        counts[w] = counts.get(w, 0) + 1
top_word = max(sorted(counts), key=lambda w: counts[w])
hours = {}
apps = {}
for created, n, app in fdb.execute(
        "SELECT created_at, word_count, app_name FROM entry_meta WHERE counted = 1"):
    h = dt.datetime.fromtimestamp(created / 1000).hour
    hours[h] = hours.get(h, 0) + n
    apps[app] = apps.get(app, 0) + n
expected = {
    "phase": phase,
    "total_words": words,
    "total_dictations": dictations,
    "wpm": round(wpm),
    "times_typing_vs_40": round(wpm / 40, 1),
    "fixes_dictionary": fd,
    "fixes_ai": fl,
    "category_percent": {c: round(100 * w / cat_total) for c, w in sorted(cats.items())},
    "current_streak": cur,
    "longest_streak": longest,
    "active_days": len(active),
    "today_band": band(days.get(today.isoformat(), 0)),
    "bands": {d: band(w) for d, w in sorted(days.items())},
    "voice_unlocked": words >= 1700,
    "most_used_word": [top_word, counts[top_word]],
    "peak_hour": max(sorted(hours), key=lambda h: hours[h]),
    "peak_app": max(sorted(apps), key=lambda a: apps[a]),
    "phrase_expected": "Sounds good to me",
}
heard_rows = [r[0] for r in fdb.execute(
    "SELECT asr_text FROM entry_meta WHERE counted = 1 AND asr_text <> final_text")]
fixes = {
    "cooper netties → Kubernetes": sum("cooper netties" in h for h in heard_rows),
    "gonna → going to": sum("gonna" in h for h in heard_rows),
}
expected["corrected_counts"] = fixes
expected["corrected_expected"] = max(sorted(fixes), key=lambda k: fixes[k])
json.dump(expected, open(out, "w"), indent=2, ensure_ascii=False)
print(json.dumps({k: v for k, v in expected.items() if k != "bands"}, indent=1))
