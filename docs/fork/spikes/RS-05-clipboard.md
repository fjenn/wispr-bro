# RS-05: clipboard fidelity and paste hygiene

**Status:** unattended part done 2026-10-30; **owner session still to run**
(`~/Projects/wispr-bro/scripts/wb-spike-clipboard.sh`, procedure in
`RS-05-procedure.md`). Sections marked _owner run_ are filled in from its
`summary.txt` / `summary.json`.

**Machine:** MacBook Pro Mac17,6 (M5 Max), macOS 26.6.2 (25G83), US layout.
Tool: `wb-spike-clipboard` 0.1.0. Raw results stay on the Mac (never
committed): `~/wb-bench/results/rs05/`.

## Short version (unattended part)

- **The objc2 snapshot restores every type byte for byte.** 10 of 10
  automated round trips of the six RS-05 sources (four of them real copies
  from TextEdit, Safari and Finder), and 25 of 25 restores after real pastes
  into TextEdit, Terminal, Safari and Chrome, came back identical, type
  order included. A 12 MB PNG snapshots in 2.3-2.9 ms and restores in
  1.1-1.5 ms.
- **hex's Carbon path does not.** It restores the data, but every text item
  comes back with two extra types (`public.utf16-plain-text`,
  `com.apple.traditional-mac-plain-text`): 9 of 11 Carbon round trips
  differ (images were identical). This confirms R4 R-10: S-04 uses the objc2
  path, not hex's Carbon calls; hex's guard and timing logic are kept.
- **Read receipts work and are fast.** With the text as a lazy promise, the
  target read it 4-18 ms after ⌘V in all four apps over three runs (one
  outlier: 1,039 ms for the first paste into a freshly opened Chrome tab).
  With "read, then 200 ms
  quiet", the restore happened 204-222 ms after ⌘V; the 1.5 s / 7 s
  deadlines were never needed. The paste was readable in the field 26-86 ms
  after ⌘V (two first-paste outliers: 1,065 ms in Chrome, 1,176 ms for the
  Terminal file to show it).
- **Chrome 156.0.8078.79 is in the 155/156 range, but did not drop ⌘V here:**
  0 of 18 pastes dropped over three runs (local test page). The typing path
  (hex) also landed, 3 of 3. ChatGPT on this Mac (bundle `com.openai.codex`,
  26.1021.31345) also embeds Chromium 156; Cursor 148, Obsidian 150 and Claude
  152 are Electron outside the range.
- **Guard checks pass on the real pasteboard:** a copy made during the paste
  window is kept (no restore); back-to-back takes restore the original; RTFD
  and PDF are left out and the rest restored; listing types (a manager's
  first look) does not fire the lazy text; a reader process triggers the
  receipt and the restore.
- **Paste with Japanese (Romaji) on landed,** plain and while "nihon" was half
  typed (the IME committed "にほn", then the text arrived). The tool turned
  the input method on and off again; the saved input-source list was
  unchanged afterwards. (Enabling Kotoeri also switches on its siblings,
  Kana, Ainu and the 50-on palette; the tool turns off everything that was
  not on before.)
- **Not yet known:** the owner's apps (Notes, Mail, Messages, Cursor,
  Obsidian, Claude, ChatGPT), held ⌃⌘ and the owner's IME run. Clipboard
  managers stay **untested** (none installed).

## Results against every RS-05 criterion

| RS-05 criterion (R5 §7)                                     | Unattended                                                               | Owner run                                                                                          |
| ----------------------------------------------------------- | ------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------- |
| Byte-identical restore for all listed types                 | **PASS** in 4 apps (25/25), round trips 10/10                            | **PASS** 68/68 in 11 apps                                                                          |
| No clipboard manager records the dictated text              | **UNTESTED** (none installed); markers present on the text item in 25/25 | UNTESTED                                                                                           |
| Held ⌃⌘ never turns ⌘V into ⌃⌘V after the modifier wait     | not run (needs a person holding keys)                                    | **not measured**: the tool saw no modifiers held when it pasted (timing); moved to S-06 acceptance |
| Paste lands with a CJK IME active (or reports `unverified`) | **PASS** (Japanese Romaji, plain and composing)                          | **PASS** (same)                                                                                    |
| Chrome version check for 155/156                            | Chrome 156.0.8078.79: in range; ⌘V dropped 0/18, typing 3/3              | -                                                                                                  |
| (§5.4) Paste lands; never the old clipboard                 | **PASS** 19/19 (⌘V outside the race) + 6/6 race probes, 0 old-clipboard  | **PASS** 56/56 in 11 apps, 0 old clipboard; Chromium 155/156 ⌘V dropped 0/12                       |

## Sources

What each source put on the clipboard (self-check `20261030-140344`):

| Source          | From     | Types                                                                                                                                                        | Bytes   |
| --------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------- |
| plain text      | tool     | `public.utf8-plain-text`                                                                                                                                     | 71      |
| rich text (RTF) | TextEdit | `public.rtf`, `public.utf8-plain-text`, `public.utf16-external-plain-text`                                                                                   | 548     |
| web page (HTML) | Safari   | `com.apple.webarchive`, `public.rtf`, `public.html`, `public.utf8-plain-text`, `com.apple.WebKit.custom-pasteboard-data`, `public.utf16-external-plain-text` | 3,956   |
| a file          | Finder   | `public.file-url`, `public.utf16-external-plain-text`, `public.utf8-plain-text`, `com.apple.icns`                                                            | 849,852 |
| an image (PNG)  | tool     | `public.png`                                                                                                                                                 | 152     |
| two files       | Finder   | item 1: `public.file-url`, `public.utf16-external-plain-text`, `public.utf8-plain-text`; item 2: `public.file-url`                                           | 193     |

Notes: TextEdit wrote no RTFD for plain styled text, so no real copy hit the
skip policy (the guard check and unit tests cover it). Finder's single-file
copy carries the file icon (`com.apple.icns`, 830 KB), which the snapshot
keeps. An earlier self-check copied Safari's address bar instead of the page
(Safari keeps key focus there after `open`); the tool now clicks into the
page first.

## Paste matrix

Unattended (`--self-check`, three runs; the table is `20261030-141229`):

| App                   | Class        | Landed             | Restore identical | First read after ⌘V | Restore after ⌘V |
| --------------------- | ------------ | ------------------ | ----------------- | ------------------- | ---------------- |
| TextEdit 1.20         | native       | 6/6                | 6/6               | 7-15 ms             | 209-220 ms       |
| Terminal 2.15 (`cat`) | native       | 6/6                | 6/6               | 4-11 ms             | 208-215 ms       |
| Safari 26.6.2         | native       | 6/6                | 6/6               | 7-13 ms             | 207-217 ms       |
| Chrome 156.0.8078.79  | Chromium 156 | 6/6 ⌘V + 1/1 typed | 7/7               | 12-18 ms            | 204-222 ms       |

_Owner run:_ Notes, Mail, Messages, Cursor, Obsidian, Claude, ChatGPT, and the
same four again.

## Held ⌃⌘ (S-06 paste-last)

**Owner run 2026-10-30 (`~/wb-bench/results/rs05/20261030-163841`): not
measured.** In both variants the tool saw no modifier held when it posted ⌘V
("held none"), so the owner's hold and the paste did not overlap; the owner
was unsure whether anything else happened. S-06's acceptance run repeats it
with the paste fired from a held key. Design: two variants in TextEdit: ⌘V posted while ⌃⌘ are still held (no
wait), and with the modifier-release wait. The listen-only session tap records
the flags our V key-down arrives with; the field shows whether the text
arrived.

## Clipboard managers

Untested: Maccy, Raycast and Paste are not installed. While the transcript is
on the clipboard, the tool reads the types as a manager's first look would: one
item, carrying `org.nspasteboard.TransientType`, `AutoGeneratedType` and
`ConcealedType` plus the text, nothing else (25/25). Listing types does not fire
the lazy text. Whether a manager also records the **restore** write as a new
copy (RT-24) is not known; the restore is written without markers, so that it
stays byte-identical.

## Findings for S-04

1. Use the objc2 `NSPasteboardItem` snapshot (`src/clip/nspb.rs`), not hex's
   Carbon calls (`src/clip/carbon.rs`, kept only for this comparison).
2. Write the transcript as one item with the three markers as eager data and
   the text as a lazy promise: the provider callback is a reliable receipt
   (4-18 ms), so most restores happen ~210 ms after ⌘V instead of 1.5 s / 7 s.
3. `pasteboardFinishedWithDataProvider:` fires once every promised type has
   been provided; it does not mean another app took the clipboard. Use
   `changeCount` for that.
4. Chrome 156 did not drop ⌘V in 18 tries; keep hex's typing path for 155/156
   (it worked) but treat the race as not reproduced on this Mac.
5. In the IME restore, compare against the enabled list from before and turn
   off everything new (Kotoeri brings siblings along); the in-process TIS list
   goes stale after enable/disable, so check the saved list.
