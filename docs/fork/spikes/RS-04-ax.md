# RS-04: Accessibility (AX) matrix, results

**Status:** unattended part done 2026-10-30 (agent, `--auto`); **the guided
owner session is still to run** (`./scripts/wb-spike-ax.sh`, procedure in
`RS-04-procedure.md`). Fill the owner sections below from
`~/wb-bench/results/rs04/<timestamp>/summary.txt` when it is done.

**Machine:** MacBook Pro Mac17,6 (M5 Max), macOS 26.6.2 (25G83). Raw results
stay on the Mac (never committed): `~/wb-bench/results/rs04/`.

Tool: `src-tauri/crates/wb-spike-ax` (`src/ax/` is the S-05 prototype of
`fork/ax/`; see the procedure for the file map, ports and what each event
records).

## Short version (unattended part only)

- **TextEdit, Safari and Chrome pass every RS-04 criterion** on what the agent
  could drive: editable verdict right on 39/39 text-field events (all firm
  "editable", no fail-open needed), 0 false cards, 4/4 password events
  refused, 16/16 non-text events got the card, selected text read on 15/15
  selection events with the exact text, URL read on 54/54 web-page events,
  p95 AX call 0.1-0.4 ms (budget 100 ms).
- **System-wide `AXFocusedApplication` failed on every call from the agent's
  process** (kAXErrorCannotComplete at once, never a timeout), though
  `AXIsProcessTrusted()` is true and app-level calls work. The tool falls
  back to NSWorkspace's frontmost app, which was always right (it reported
  the newly opened app 40-59 ms after `open -a`). The owner's run, from a
  plain Terminal, shows whether this is specific to the agent's process tree.
  **For S-05: never depend on system-wide `AXFocusedApplication` alone.**
- **`AXSelectedText` writes report success and do nothing in Safari and
  Chrome web fields** (0/5: input, textarea, contenteditable; the field's
  length and text did not change within 500 ms). In TextEdit they work (4/4
  over two runs, and the original was put back each time). This confirms
  RT-21's risk for the web: replace-selection must be clipboard paste
  (already the default).
- **Chrome does not support `AXManualAccessibility`** (`AttributeUnsupported`
  on read and write), and its web tree was there anyway: without and with
  our lease gave the same answers (14/14 text fields both ways). The lease
  matters for Electron. Cursor accepts the write (`Success`) but reads back
  `false` right after it.
- **Secure Input lingers.** After focus leaves a password field for a
  button, link or the page body, Safari and Chrome keep Secure Input on for
  several seconds. With a "refuse whenever Secure Input is on" rule, the
  first Safari run refused 6 of 8 non-text events (button, link, body) and
  the first Chrome run 4 more (body, read-only). The prototype
  refuses on Secure Input only when AX cannot tell (`editable.rs`).

## Results against every RS-04 criterion

| RS-04 criterion (R5 §7) | Unattended (agent) | Owner session |
|---|---|---|
| Editable verdict correct in ≥ 95 % of focused text fields (fail-open counts) | PASS: TextEdit 9/9, Safari 16/16, Chrome 14/14 | _to fill_ |
| 0 false "no text field" | PASS: 0 in all three | _to fill_ |
| Selected text readable in ≥ 90 % via the chain | PASS: TextEdit 4/4, Safari 6/6, Chrome 5/5, all by plain `AXSelectedText`, exact text each time | _to fill_ |
| URL host readable in Chrome, Arc, Brave, Edge, Safari | Readable 28/28 Safari, 26/26 Chrome, but for a `file:` page (no host). **Hosts only from the owner's tabs.** Arc, Brave, Edge not installed | _to fill (Safari tab, Gmail, Docs)_ |
| p95 AX call ≤ 100 ms | PASS: p95 0.12 ms TextEdit, 0.39 ms Safari, 0.11 ms Chrome (max 30 ms); whole read p95 6-23 ms | _to fill_ |
| ≥ 20 focus events per app | TextEdit 9, Safari 28, Chrome 26 | 2-6 per app by design (see Coverage) |
| With and without the `AXManualAccessibility` lease | Chrome: identical (attribute unsupported). Electron: see below | _to fill (Cursor, Obsidian, Claude)_ |
| Latency at 0.1 and 0.25 s timeouts | No call came near either timeout; 0.1 s never changed an answer | _to fill_ |
| `frontmostApplication` vs `AXFocusedApplication` freshness | AX call failed every time (see above); NSWorkspace 40-59 ms after `open -a` | _to fill_ |
| `AXSelectedText` set | TextEdit 4/4 worked and put back; Safari 0/3, Chrome 0/2 (success reported, nothing changed) | _to fill_ |

### Unattended runs

`wb-spike-ax --auto` (results `~/wb-bench/results/rs04/20261030-135835-auto`
and `20261030-140423-auto`):

- **TextEdit**, 9 events, driven through AX (the tool set the caret and
  selection itself in its practice file): all `AXTextArea`, verdict
  editable, caret window, bounds and window title read every time.
- **Safari and Chrome**, test page (the page's own script focused input,
  textarea, contenteditable, search, e-mail, password, button, link, page
  body, read-only and drop-down, twice each). Roles: inputs `AXTextField`
  (search: subrole `AXSearchField`), textarea and contenteditable
  `AXTextArea`, password `AXSecureTextField`, button `AXButton`, link
  `AXLink`, body `AXWebArea`, drop-down `AXPopUpButton`. Chrome dropped 2 of
  28 steps (the first two, before the tool saw the page).
- Read-only input: `AXValue` not settable but `AXSelectedText` settable, so
  the verdict says editable (Safari). Recorded as information only.
- Caret/selection bounds: missing on some non-text and empty-selection
  events in Safari (19/29), fine elsewhere.
- **Electron/ChatGPT "as opened"** (no clicks possible, so not scored):
  Cursor and Obsidian had no focused element right after `open -a`, with or
  without the lease, and the fail-open rule pasted (window present). Claude
  had a focused `AXButton` without the lease. ChatGPT (native) had its
  message box `AXTextField` focused.

## Owner session

_To fill from the owner's `summary.txt`: the per-app table, the
with/without-lease rows for Cursor, Obsidian and Claude, the hosts seen,
`AXSelectedText` per app, the activation timings, and whether system-wide
`AXFocusedApplication` worked from Terminal._

## Coverage and what is not tested

- Apps not installed: Slack, VS Code, Discord, Notion, Arc, Brave, Edge,
  Firefox, Word, iTerm2. Cursor stands in for VS Code, Chrome for the other
  Chromium browsers.
- Events: the guided part has 2-6 events per app (58 in all) to keep the
  session near 15 minutes; RS-04 asks for ≥ 20 per app. Only Safari and
  Chrome reach 20, through the test page.
- Selection chain steps 3-4 (Edit › Copy press, ⌘C sentinel) are not run:
  they change the clipboard (RS-05). The tool only checks that Edit › Copy
  is available (found by its ⌘C shortcut, enabled).
- Not covered: focus moving between key-up and delivery, Secure Input held
  by another app (RS-14), multiple displays, macOS 14/15.
