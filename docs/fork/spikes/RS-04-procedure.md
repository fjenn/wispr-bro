# RS-04: text field test, how to run it

This test checks how well Wispr Bro can tell where your cursor is: in a text
field, where dictated text should go, or somewhere else (a list, a sidebar,
a button). It takes **about 15 minutes**. Nothing leaves your Mac.

## Start it (3 steps)

1. Open **Terminal** (press ⌘ Space, type `Terminal`, press Enter).
2. Copy this line into Terminal and press Enter:

   ```sh
   ~/Projects/wispr-bro/scripts/wb-spike-ax.sh
   ```

   The first time, it spends about a minute getting ready.

3. Make the Terminal window narrow and put it at the right edge of the
   screen, so you can read it while you click in the other apps. Then follow
   what it says.

To stop at any time, press **Control + C**. What you did up to then is kept.

## What will happen

- **Permission.** The test needs **Accessibility** for Terminal (System
  Settings → Privacy & Security → Accessibility → Terminal). It is probably
  already on from the fn key test. If not, the test tells you where to
  switch it on and offers to open that page.
- **One app at a time**, 13 parts in all: TextEdit, Notes, Mail, Messages,
  Terminal, Safari, Chrome, Gmail and Google Docs in Chrome (only if you're
  signed in: the test asks, and "n" skips them), Cursor, Obsidian, Claude and
  ChatGPT. Press Enter to start each app, or type `s` to skip it.
- **The test opens the app** and asks you to click somewhere, for example
  "Click into the search box" or "Click a note in the list of notes". It
  notices the click by itself, beeps, and shows the next step. Some steps
  ask you to type `hello test` and double-click a word to select it.
- **If it doesn't notice your click** within 20 seconds it says so: click
  back into Terminal and press Enter. Type `s` and Enter to skip a step you
  can't do, `q` and Enter to skip the rest of that app.
- **Safari and Chrome** first show the test's own page (a file on your Mac,
  no website). It moves through its fields by itself for about a minute and
  a half. **Don't click while it runs.**
- **Practice fields.** In TextEdit, the new note, the new e-mail, the new
  Cursor file, Obsidian's new note and the Claude/ChatGPT message box, the
  test briefly types `[wb]` and removes it again. That checks whether
  Wispr Bro could replace selected text directly. It never does this in
  Messages or Terminal, and nothing is ever sent.
- **Tidying up** after each app, it tells you what to close or delete: the
  test note, the draft e-mail (Delete Draft), the unsaved Cursor file (Don't
  Save), the "hello test" in a message box.
- **Results.** At the end it prints a short summary and the folder it saved
  everything in (`~/wb-bench/results/rs04/<date-time>/`). Tell Claude you're
  done; it reads the results from there.

What is saved: for each click, what kind of place it was (text field,
list, button...), whether text was selected and how many characters,
timings, the website's name for browser tabs (like `mail.google.com`,
never the full address), and whether the window had a title (its length,
not the title). Nothing you typed, no e-mail or note text, no window titles
of your own documents. Text is only kept for the test's own page and
practice file.

## For the team: what this run covers and what it doesn't

The tool is `src-tauri/crates/wb-spike-ax` (its own workspace and lockfile:
a pruned copy of `src-tauri/Cargo.lock` plus `objc2-application-services
0.3.2`). Modes: `--dry-run` (permission, plan, nothing written), `--auto`
(no owner: TextEdit driven through AX and the test page in Safari and
Chrome; one unscored look at Cursor, Obsidian, Claude and ChatGPT), `--apps a,b`, `--out DIR`, `--lease-check PID` (read, set and restore `AXManualAccessibility` on one app), `--summarize DIR` (rebuild the summary
from `events.jsonl`; the summary is also written on Ctrl+C).

`src/ax/` is the S-05 prototype of `fork/ax/` (R5 §4.6):

| File | What | Origin |
|---|---|---|
| `element.rs` | AX call wrappers, app/bundle helpers, Secure Input | SpeakoFlow `smart_spacing/macos.rs` (MIT, port) + additions |
| `thread.rs` | one AX thread; jobs with a budget; leases restored on expiry, exit and Ctrl+C | SpeakoFlow worker pattern (MIT), generalised |
| `lease.rs` | `AXManualAccessibility` per-PID refcount, restore-previous, retry back-off, 60 s hold | VoiceInk `AccessibilityWebAccess` (GPL-3.0, port) + SpeakoFlow hold |
| `editable.rs` | `is_editable` verdict + delivery (paste / fail-open paste / card / refuse) | OpenWhispr `isEditableTextElement` (MIT, port) + VoiceInk checks + R5 fail-open rule |
| `markers.rs` | selected text via `AXSelectedTextMarkerRange` / `AXStringForTextMarkerRange` | VoiceInk (GPL-3.0, port) |
| `probe.rs` | one timed full read per focus event | ours |

Per focus event the tool records (`events.jsonl`, one `event` line with the
raw probes and a `scored` row): role/subrole, enabled, `AXEditable`,
`AXProtectedContent`, `AXValue`/`AXSelectedText` settable, verdict and
delivery, selected range and character count, caret window read
(`AXStringForRange`, length only), selected text through AX then text
markers (length only outside the tool's own fields), whether Edit › Copy (found
by its ⌘C shortcut, not its title) is enabled, caret/selection bounds
(`AXBoundsForRange`) and element frame, window title length, the URL from
the web area's `AXURL` (else the window's `AXDocument`) reduced to its host,
`AXSelectedText` write test in scratch fields (write `[wb]`, verify, put the
original back), every AX call's latency and error, and NSWorkspace
`frontmostApplication` vs system-wide `AXFocusedApplication`.

Probes per event: Chromium/Electron apps get one probe with our lease
switched off, then one with the lease on at a 0.25 s messaging timeout and one at
0.1 s; other apps get the 0.25 s and 0.1 s probes. The write test runs last,
in its own probe. Each Chromium/Electron app also gets one "cold" probe on
activation, before our lease ever touches it (not scored: focus is wherever
the app put it). After the first event, "lease off" means our lease was
switched off 300 ms before the probe; Chromium may keep its tree built for
a while, so the cold probe is the clean "without lease" reading.

Ground truth: each prompt names a place that is or is not a text field.
The tool records when focus (or, for selection steps, the selected range)
changes in that app and stays put for 600 ms, or when the owner presses
Enter (then it re-activates the app and reads what is focused there). On the
test page, the page's own title (`WB-RS04 r1 03 textarea text`) is the
ground truth; a read during which the page moved on is dropped (`stale`).

Compared with the RS-04 row in R5 §7, this run is smaller:

- **Apps:** only the installed ones. Slack, VS Code, Discord, Notion, Arc,
  Brave, Edge, Firefox, Word and iTerm2 are not tested (not installed).
  Cursor stands in for VS Code (same Electron/Monaco editor); Chrome stands
  in for the other Chromium browsers.
- **Events:** RS-04 asks for ≥ 20 focus events per app. The guided part has
  2-6 per app (58 in all, to stay near 15 minutes); the test page adds 28 in
  Safari and in Chrome. Per-app percentages from 4-6 events are indicative
  only.
- **Selection chain:** only AX and text markers are measured. Edit › Copy is
  only checked for being available (pressing it, or ⌘C, would change the
  clipboard; that is RS-05's ground).
- **Not covered:** focus moving between key-up and delivery (S-05's target
  rule), Secure Input from another app (RS-14), multiple displays.
