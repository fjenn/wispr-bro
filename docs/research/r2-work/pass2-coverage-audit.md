# R2 pass 2: coverage audit (R1 vs pass-1 area files A to E)

**Purpose.** This audit checks that every user-visible item in `docs/research/R1-wispr-flow-teardown.md` appears in at least one pass-1 gap file, flags items covered in more than one area with conflicting classes or claims, and lists Wispr claims in the gap files that R1 does not support.

**Inputs.** R1 (all 227 lines). Pass-1 files: `pass1-A-capture-insertion.md` (A-01..A-69), `pass1-B-onboarding-models.md` (B-01..B-68), `pass1-C-hub-data.md` (C-01..C-106), `pass1-D-text-intelligence.md` (D-01..D-71), `pass1-E-notetaker-platform.md` (E-01..E-95). I did not read Handy code or `docs/research/evidence/`.

**Method.** I split R1 into atomic items with IDs `R1-§<section>-<nn>`, numbered in reading order within each section. Open items are `R1-§OI-nn`. Matching is by meaning. Statuses:
- `COVERED`: one or more rows cover the item, including its stated state, default or label.
- `PARTIAL`: a row covers the feature but misses a specific state, default, control or sub-step.
- `MISSING`: no row covers it.
- `N/A`: meta or process note with nothing user-visible (methodology, privacy notes about evidence, side effects of the session).

An R1 `unknown` that the area file lists as an open question counts as COVERED.

## Summary counts

| Measure | Count |
|---|---|
| R1 atomic items | 419 |
| N/A (meta, process notes) | 6 |
| Items assessed (all items except N/A) | 413 |
| COVERED | 394 |
| PARTIAL | 16 |
| MISSING | 3 |
| Duplicates/conflicts listed (same item in 2 or more areas) | 43 rows (27 with a real conflict in class, effort, ownership, approach or technical claim; 16 consistent duplicates that only need one owner) |
| Unsupported Wispr claims in gap files | 8 (plus 1 checked and found supported) |

Coverage is very high. The three MISSING items are all small cloud-side settings or flow steps. The real risk is in the **duplicates**: 27 items are claimed by two to four areas with different classes or efforts, and two of those have conflicting technical claims (D-48 vs C-63 ordering, B-23 vs A-35 fn support).

## Missing

| R1 item ID | Item | Best-fit area | Why it matters |
|---|---|---|---|
| R1-§5-41 | Data and Privacy toggle **"Show your public information"** (on) | C | Every Data and Privacy control needs an explicit decision for the "all local" privacy page (C-93/E-68). This one should be listed as OUT, so the page does not quietly drop a Wispr control without a reason on record. |
| R1-§5-43 | Data and Privacy **"Sync notes"** (and the cross-device sync named in §7 and in the Free plan's "Mac/Windows/iOS/Android") | C (or E) | No row classifies sync as OUT. The privacy page and the "Store data locally" story (E-64) should say clearly that nothing syncs. |
| R1-§11-44 | After the Notetaker tour, the **Connectors settings page opens automatically as the next suggested step** | B | This is the "chain the next setup step" pattern (the tour ticks a checklist item, then routes to the next one). It matters for the B-44/C-10 checklist design even if Notetaker itself is deferred. |

## Partial

| R1 item ID | Item | Covering row(s) | What is missing |
|---|---|---|---|
| R1-§3-12 | Esc: "Dismiss dictation **and notifications**" | A-25 | A-25 covers dictation, processing and the fallback card. It does not cover dismissing other transient surfaces: toasts (E-26 "Started by mistake?", E-27 screen-share), pill messages (A-13/A-14) and coaching spotlights (A-16). One Esc policy is needed for all of them. |
| R1-§4-17 | Notetaker page **Today section "Try Notetaker" card** (Skip / Start, video-thumbnail icon) | E-30, E-43 | E-30 covers the page and the empty state, and E-43 covers the tour. No row covers the entry card itself or its Skip (dismiss) behavior. |
| R1-§4-50 | Snippets page has the **"same layout"** as Dictionary: tabs, search, sort, refresh | C-69 | C-69 lists the list, Import, Add new and banner. Snippet search/sort/refresh (the equivalent of C-60) and the snippet tabs (the equivalent of C-59, local All / Yours) are not covered. |
| R1-§4-79 | **Launch popup "Dark mode is finally here"** (feature-announcement modal) | C-99, C-20 | C-99 covers release notes per version. A one-off in-app feature announcement (popup on first launch after a feature ships) is not covered. It is distinct from the Notifications inbox (C-17). |
| R1-§5-40 | Data and Privacy **"Dictation cloud storage"** toggle (off) | C-46 (Wispr column only) | It is mentioned but never classified. It needs an explicit OUT (or "audio stays local") line for the privacy page. |
| R1-§6-01 | App menus list: File, Edit, Dictation, **Notetaker**, My Voice, View, Help, **Window** | A-63, C-105, C-106 | Dictation, My Voice and Help are covered. No row covers the Notetaker app menu (OUT with Notetaker, but unstated) or the Edit/Window menus (standard, but Tauri's default menu has to be checked once a custom menu is built). |
| R1-§10-05 | Help popover "Get in touch": **Contact sales** | C-13, C-103 | Help Center and Talk to support are covered. Contact sales is not mentioned (obvious OUT, but unstated). |
| R1-§10-17 | Transforms Try-it step 1: **ticker of benefits** (Make more concise, Reword for clarity...) | D-43, B-40 | The guided modal is covered generically. The benefit ticker is not, and it reuses the Polish rule names from D-33, so it is cheap to keep consistent. |
| R1-§10-22 | **Polish detail modal, left side**: name, "⌥Opt 1 to use", description, transformed example, **"See Updates"** button | D-32, D-33, D-34, D-36 | Rows cover the right-side controls (shortcut, rules, prompt, autosave, reset). The left-side preview with an example and a "See Updates" button (a diff preview) is not covered. |
| R1-§10-33 | Scratchpad floating window: **expand and close buttons** | C-75 | C-75 lists tabs, "+", search, AI sparkle, text size, Copy. Window-level expand/close is not covered. |
| R1-§10-34 | Scratchpad window left rail: **sidebar toggle, new note** | C-75 | Search, sparkle and text size are covered. The sidebar toggle and the new-note button in the rail are not. |
| R1-§10-38 | Three-button bar: **small teal badge** on the Notetaker button | A-03, E-29 | The three buttons are covered. The badge (state indicator) is not. Low impact while Notetaker is OUT. |
| R1-§12-08 | After the whisper tutorial, the **whispered dictation appears in the real Dictation history** | B-42 | B-42 covers the tutorial. Whether practice dictations are saved to history (Wispr: yes) is not decided. Handy's TryIt behavior here is not stated in B. |
| R1-§13-27 | Shortcut test **"No" answer** (and the mic test's negative path) | B-19, B-21, B-26 | The Yes path and Handy's failed-take diagnosis (B-26) are covered. What Wispr does on "No" was not observed, and no row defines our recovery path for "the key did not light up" (for example the Globe key bound to the emoji picker; see A-35). |
| R1-§14-07 | **Transcript tab shows a live-waveform icon while recording; Summary tab shows a sparkle icon** | E-15, E-23 | The tab contents are covered. The tab-level state icons are not. Minor. |
| R1-§1-06 | Product exists on **Mac/Windows/iOS/Android** (Free plan; §9 "mobile" out of scope) | C-53, C-96 | C-53 says "single device" and C-96 covers plans. No row states that mobile is OUT and Windows is later as a platform decision. E-58 trims the release matrix, but only as a CI choice. |

## Duplicates and conflicts

Rows marked **conflict** disagree on class, effort, ownership or a technical claim. Rows marked "consistent" agree and only need one owner named.

| R1 item | Rows | Conflict | Suggested resolution |
|---|---|---|---|
| R1-§2-13 pill processing state | A-11 (HAVE), D-10 (TUNE S) | **Conflict**: class differs. | A-11 owns the state (HAVE). D-10 becomes a copy note ("Polishing…") under A-11. |
| R1-§3-01 push-to-talk on fn | A-22 (TUNE S), A-35 (EXTEND M), B-23 (EXTEND M) | **Conflict**: B-23 says fn capture support in `handy_keys` is `unknown`, while A-35 verified handy-keys supports the FN modifier. Effort differs (S vs M). | A owns it. A-35's verified finding replaces B-23's unknown. B-23 becomes a dependency note for B-21 (shortcut test). Use M (A-22 + A-35 together). |
| R1-§3-05 Press Enter | A-28 (BUILD S), D-61 (EXTEND S) | **Conflict** in meaning: A-28 reads it as a standalone key or mouse button that sends Enter (matches R1). D-61 reframes it as "dictate then press Enter" or auto-send. | Keep A-28 as the R1 feature. Move D-61's per-app auto-send to a separate Handy-side idea, not Wispr parity. |
| R1-§3-11 Open Scratchpad shortcut | A-30 (EXTEND S, "not set"), C-77 (BUILD S, "add an action **and default binding**") | **Conflict**: R1 shows it "not set" by default in the hotkey panel, with Opt S only proposed in the Scratchpad dialog. C-77 implies a default binding. Class differs. | C-77 owns the action (BUILD). The default binding stays unset (needs A-32). The dialog proposes ⌥S. |
| R1-§3-13 Transform hotkeys ⌥1/⌥2 | A-30 (EXTEND S), D-32 (EXTEND M) | **Conflict**: effort differs. D-32 also flags that ⌥1 types "¡" on a US layout and that runtime IDs are needed. A-30 notes runtime IDs too. | D-32 owns it (M). A-30 keeps only the registration plumbing. |
| R1-§3-10 / §6 copy last transcript | A-27 (EXTEND S), A-56/A-57, C-106 (EXTEND S) | Consistent. | A owns hotkey and tray. C-106 owns only the app-menu entry. |
| R1-§4-06 sidebar setup checklist | C-10 (BUILD S), B-44 (BUILD M) | **Conflict**: effort S vs M, and the proposed item lists differ. | B-44 owns content and auto-ticking (M). C-10 owns only the sidebar slot. Merge the item lists. |
| R1-§4-xx example banners (pattern) | B-38 (BUILD M), C-23 (BUILD S), plus C-29, C-61, C-73 | **Conflict**: effort S vs M for the same component. | C-23 owns the component (S). B-38 owns the content and dismissed-state policy across features. |
| R1-§4-21/22 Undo AI edit | C-42 (EXTEND S), D-07 (EXTEND S), D-06 | Consistent class. D-06 adds that "raw" is not verbatim, and C's data-model gap #4 says the same. | C owns the UI. D owns the text-variant semantics (what "original" means). |
| R1-§4-23 Retry transcript | C-43 (HAVE), D-08 (HAVE) | Consistent. | C owns it. |
| R1-§4-31 history retention / audio kept | C-46 (EXTEND M), E-69 (TUNE S), A rough edge 14 | **Conflict**: class and effort differ. E treats it as a default change. C splits text and audio retention and needs stats to survive. | C-46 owns it (EXTEND M). E-69 folds into it. |
| R1-§4-39 fixes made by Flow | C-52 (BUILD M), D-57 (EXTEND S) | **Conflict**: class and effort differ. | D-57 owns returning counts from each text stage (S). C-52 owns persistence and display (M, includes the LLM word diff). |
| R1-§4-41 usage by app category + frontmost app capture | C-54 (BUILD M), D-62 (BUILD S), D-21 (BUILD S) | **Conflict**: effort differs. Frontmost-app capture is scoped in both C-54 and D-21. | D-21 owns capture. D-62 owns storing it. C-54 owns the category map and the chart. |
| R1-§4-44 Voice Profile cards | C-57 (BUILD M), D-63 (BUILD M) | Consistent. | C owns it. |
| R1-§4-46 dictionary Import | C-64 (BUILD S), D-50 (BUILD S); snippets C-71 / D-55 | Consistent. | C owns the wizard. |
| R1-§4-48 from → to replacements | C-63 (BUILD M), D-48 (EXTEND S) | **Conflict**: class and effort differ, and the **technical claims conflict**. C-63 applies replacement rules "after ASR and **before** fuzzy matching". D-48 applies them "**after** fuzzy correction". | D owns the pipeline. Decide the order explicitly. Running before fuzzy matching stops fuzzy matching from rewriting a replaced word, and running after lets rules fix fuzzy mistakes. C owns the dialog only. |
| R1-§4-49 snippet expansion | C-72 (BUILD M), D-53 (BUILD M) | Consistent. Both say it must run after LLM cleanup or in a protected form. | D owns it. |
| R1-§5-07 launch at login | A-64 (TUNE), B-35 (HAVE), C-85 (TUNE), E-87 (TUNE) | **Conflict**: class differs (B says HAVE, because onboarding defaults it on, while the backend default is off). The Wispr-default claim also differs: A-64 says `unknown`, while B-35, C-85 and E-87 say "on by default". | One owner (C-85). Record that the Wispr default is unknown. The product call is "on via onboarding toggle" (B-35 already does this). |
| R1-§5-08 Show Flow Bar at all times | A-66 (BUILD S), C-86 (EXTEND S) | **Conflict**: class differs. | A owns it (BUILD, since the work is A-01). C-86 only places the setting. |
| R1-§5-09 Show app in Dock | A-65, C-87 (both EXTEND S) | Consistent. | C owns the setting. A notes the activation-policy risk. |
| R1-§5-10 sounds | A-42, C-89 (both TUNE S) | Consistent. Both claim "on by default" (see unsupported claims). | A owns it. |
| R1-§5-03 recommended microphone | A-38 (EXTEND S), B-20 (EXTEND S), C-81 (TUNE S) | **Conflict**: C-81 says TUNE, but the "recommended" detection does not exist (A-38). | A-38 owns detection. B-20 and C-81 consume the label. |
| R1-§5-41 context awareness | C-95 (BUILD later L), D-58 (BUILD M) | **Conflict**: effort L vs M, and timing (later vs not stated). | D-58 owns it. Agree on effort and milestone. |
| R1-§5-25/26 Vibe coding (variable recognition, file tagging) | C-97 (OUT v1), D-59 (BUILD L), D-60 (BUILD L) | **Conflict**: OUT vs BUILD. | Take a scope decision once. D-59/D-60 with a "later" milestone tag and C-97 noting the same. |
| R1-§5-47 footer version + Update | C-27 (HAVE), E-51 (TUNE S) | **Conflict**: class differs. | E-51 owns it (TUNE: moving to the settings footer counts as a tune). |
| R1-§5-45/46 download / delete your data | C-94 (BUILD S), E-66, E-67 (BUILD S) | Consistent, but E-67 adds scope (HF cache, logs, settings, meetings DB) that C-94 lacks. | E owns them with E's fuller scope. |
| R1-§6-06..09 My Voice menu | A-63 (BUILD M), C-105 (BUILD S), D-64 (BUILD S) | **Conflict**: three owners, effort M vs S. | A-63 owns the app-menu build (M, all menus). C-105 and D-64 become dependencies (counts and quick-add dialogs). |
| R1-§6-11..13 Help center / Talk to support / Share feedback | A-61 (OUT, local S), C-102 / C-103 (OUT / BUILD S), E-77 (TUNE), E-90 (TUNE) | **Conflict**: four rows across three areas with three classes. | C owns the "Help and Feedback" popover (C-13 + C-103). A-61 covers only the tray entry. E-77/E-90 fold in. |
| R1-§10-06 What's New | C-99 (HAVE), E-89 (TUNE) | **Conflict**: class differs. | C-99 is HAVE for the mechanism. E-89 TUNE is for content. Keep both but cross-reference them. |
| R1-§10-10 / §13-15 system-audio permission | B-14 (OUT), E-02 (EXTEND S) | **Conflict**: OUT vs EXTEND. | Same scope decision as Notetaker. Mark E-02 "deferred with Notetaker milestone" and B-14 "OUT for v1", with matching wording. |
| R1-§10-14..23 Transforms tutorial | B-39 / B-40 (EXTEND M), D-43 (EXTEND M) | Consistent. | B owns the engine. D owns the Transform content. |
| R1-§10-22 / §10 "Done. See changes" pill + spotlight | A-14 (BUILD S), D-39 (BUILD S), A-16 (BUILD M), B-32 (BUILD M), B-41 | Consistent class. Ownership is split five ways. | A owns the pill message state (A-14). A-16 or B-32 owns the spotlight (pick one). D-39 becomes a dependency. |
| R1-§10-29 writing examples | D-35 (BUILD M), D-65 (BUILD M) | Duplicate inside D. | Merge into D-35. |
| R1-§10-42 Data and Privacy "all local" page | C-93 (EXTEND S), E-68 (BUILD S) | **Conflict**: class differs. | E-68 owns the page (it has the network inventory). C-93 lists the existing pieces. |
| R1-§11-45 / §14-03 pill recording state for Notetaker | A-21 (OUT), E-29 (EXTEND S), A-03 (Notetaker slot OUT) | **Conflict**: OUT vs EXTEND. | Same Notetaker scope decision. E-29 marked deferred. |
| R1-§11-51/52/62 tray meeting items, Start Notetaker | A-58 (OUT), E-05 (EXTEND S), E-07 (BUILD M) | **Conflict**: OUT vs EXTEND/BUILD. | Same as above. |
| R1-§13-22 live level meter (mic test) | A-40 (BUILD S), B-19 (EXTEND M) | **Conflict**: class and effort differ for the same plumbing (a mic-test stream emitting levels to the main window). | A-40 owns the backend (S). B-19 owns the onboarding UI. |
| R1-§13-45 spotlight coaching on pill | A-16 (BUILD M), B-32 (BUILD M) | Consistent. | See the "Done. See changes" row above. |
| R1-§13-46 "Try Flow in another app" banner | B-33, C-29 (BUILD S) | Consistent. | C owns it. |
| R1-§13-48 end state = hub | B-34 (EXTEND M), C-02 (EXTEND S) | **Conflict**: effort differs. | C-02 owns the landing page (S). B-34 owns the hand-off only. |
| R1-§5 cloud LLM providers (§7 cloud) | D-69 (OUT: "remove or compile out"), E-76 (TUNE: "hide in UI rather than deleting code") | **Conflict** in approach, which affects the merge strategy and the privacy claim. | Decide once. E's merge argument favors hiding; D's rough edge 8 (providers re-added on load) means hiding must happen in `default_post_process_providers`. |
| §8.1 local LLM runtime | D-67 (BUILD L), E-49 (BUILD L), D-71 (EXTEND M) | Consistent. | D owns it. E-49 depends on it. |
| R1-§5-22 hide from screen capture | A-20, E-28 (both BUILD S) | Consistent. They name different APIs (`sharingType` vs `set_content_protected`, same effect). | E owns it (it lives in Notetaker settings). A-20 notes that the bar must honor it. |

## Unsupported Wispr claims in gap files

| Row(s) | Claim | Why unsupported | Fix |
|---|---|---|---|
| C-66 | Wispr dictionary has "words learned automatically, **marked with a sparkle**" | R1 never mentions a sparkle on dictionary entries. Sparkles appear only in the Scratchpad rail ("AI sparkle") and the Summary tab icon. Auto-learning is only implied by the plan text "learns names and jargon". | Drop the sparkle. Tag auto-learning `inferred` from §5 Plans. |
| C-68 | Dictionary entry metadata: "Source (personal/auto), **sparkle, counts**" as Wispr behavior | R1 shows only single words and "from → to" pairs, plus All/Personal/Shared tabs. There are no per-entry sources, sparkles or counts. | Move to the Handy-side proposal column. |
| B-49 | R1 ref quoted as "13 closing note (**'auto-chosen from the hardware'** is our goal)" | R1 §13 says only "Silent model choice replaces any model picker". Hardware is not mentioned. | Re-cite as a product goal, not R1. |
| D-67 | Wispr behavior "Cloud, invisible. `observed`", citing §13 "Silent model choice" | R1's "silent model choice" is the proposal for our local product, not observed Wispr behavior. Wispr's model setup was never observed. | Retag as `inferred`. |
| B-35, C-85, E-87 (launch at login), A-42, C-89 (sounds) | "**On by default**" in Wispr | R1 §5 saw these on in a used account ("all on by default here", "(on)"). A-64 and A's open question 8 correctly say the fresh-install default is `unknown`. | Reword to "on in the observed account; default unknown". |
| D-61 | Wispr "Press Enter" framed as "after dictation" or "dictate then press Enter" | R1 §3: "Trigger enter with a mouse shortcut (or another key) to send messages faster". That is a key remap, not a post-dictation action. | Reword to match A-28. |
| A-23 | "Decide whether a single tap stays a no-op, **as Wispr implies**" | R1 says nothing about single-tap behavior. Tap and hold thresholds are `unknown` (§3, open item 2). | Remove "as Wispr implies". |
| E-53 | Wispr does "background/periodic update checks and silent download", supported by "version changed after restart (§11)" | R1 shows an Update button. The version changed after a restart caused by a permission grant. A background download is not shown. E tags this `inferred`, but the row title states it as behavior. | Keep it as `inferred` in the title or Wispr column. |
| C-46 | "Raw audio kept per dictation" | This is fine as an inference from "Extract audio" (R1 itself says "So raw audio is kept per dictation"). **Not a problem.** I list it only to record that it was checked. | None. |

The last row (C-46) is not counted; the count of unsupported claims is 8.

## Appendix: full R1 item list

| ID | Item | Status | Rows |
|---|---|---|---|
| R1-§0-01 | Method/version note (v1.6.1264, macOS, view-only) | N/A | – |
| R1-§0-02 | Evidence archiving note | N/A | – |
| R1-§1-01 | Hold fn, speak, release, text appears in focused field | COVERED | A-22, A-46 |
| R1-§1-02 | Pill at the bottom shows state | COVERED | A-01, A-05 |
| R1-§1-03 | Everything else lives in a Hub window | COVERED | C-01 |
| R1-§1-04 | Personalization example-led (style cards, banner chips) | COVERED | C-23, D-25 |
| R1-§1-05 | Soft failure: text kept, one-click copy | COVERED | A-15, A-53, D-09 |
| R1-§1-06 | Product is also account/plan/cloud; multi-platform incl. mobile | PARTIAL | C-96, C-53 |
| R1-§2-01 | Resting thin bar, bottom center | COVERED | A-01 |
| R1-§2-02 | Click expands bar into active capsule | COVERED | A-04 |
| R1-§2-03 | Black capsule look | COVERED | A-05 |
| R1-§2-04 | ✕ cancel on the left | COVERED | A-05, A-07 |
| R1-§2-05 | Live waveform of thin bars, center | COVERED | A-08 |
| R1-§2-06 | ✓ finish on the right | COVERED | A-06 |
| R1-§2-07 | Mic label "Using Built-in mic (recommended)" for ~1-2 s | COVERED | A-10 |
| R1-§2-08 | Waveform heights follow audio continuously | COVERED | A-08 |
| R1-§2-09 | macOS orange mic indicator while recording | COVERED | A-45 |
| R1-§2-10 | Fallback card: glyph, "Select a textbox first, then dictate", transcript, Copy, ✕ | COVERED | A-15, A-51 |
| R1-§2-11 | Click on bar moved focus to Wispr | COVERED | A-52 |
| R1-§2-12 | Held-key / hands-free pill states unknown | COVERED | A open Q2, A-05 |
| R1-§2-13 | Processing / spinner state | COVERED | A-11, D-10 |
| R1-§2-14 | Error state | COVERED | A-12, A-13 |
| R1-§2-15 | Resting-bar hover state | COVERED | A-02 |
| R1-§2-16 | Sizes, colors, timings | COVERED | A-17 |
| R1-§3-01 | Push to talk = fn, "Hold to say something short" | COVERED | A-22, A-35, B-23 |
| R1-§3-02 | Hands-free = double-tap fn | COVERED | A-23 |
| R1-§3-03 | Hands-free = fn + Space | COVERED | A-24 |
| R1-§3-04 | Hands-free described as start/stop with same key | COVERED | A-23 |
| R1-§3-05 | Press Enter (not set; key or mouse shortcut) | COVERED | A-28, D-61 |
| R1-§3-06 | Command Mode fn + Ctrl, "Select text and ask Flow" | COVERED | A-29, D-44, D-45 |
| R1-§3-07 | Command Mode "or Perplexity" | COVERED | D-46 |
| R1-§3-08 | Paste last transcript ⌃⌘V | COVERED | A-26 |
| R1-§3-09 | Copy last transcript ⌃⌘C | COVERED | A-27 |
| R1-§3-10 | Open Scratchpad (not set) | COVERED | A-30, A-32, C-77 |
| R1-§3-11 | Join meeting / start Notetaker ⌥M | COVERED | A-30, E-05 |
| R1-§3-12 | Cancel Esc, "Dismiss dictation and notifications" | PARTIAL | A-25 |
| R1-§3-13 | Transform hotkeys configured in Transform tab (⌥1, ⌥2) | COVERED | A-30, D-32 |
| R1-§3-14 | Transform: view changes ⌥O | COVERED | A-30, D-40 |
| R1-§3-15 | Multiple bindings per action, "+" to add | COVERED | A-31 |
| R1-§3-16 | Per-item pencil (edit), trash (delete) | COVERED | A-33 |
| R1-§3-17 | "Not set" bindings | COVERED | A-32 |
| R1-§3-18 | Panel Reset to default | COVERED | A-33 |
| R1-§3-19 | Panel Done | COVERED | A-33 |
| R1-§3-20 | Tap vs hold thresholds unknown | COVERED | A-34, A open Q1 |
| R1-§4-01 | Light warm off-white UI | COVERED | C-19 |
| R1-§4-02 | Rounded cards | COVERED | C-22 |
| R1-§4-03 | Serif display headings on banners; sans UI | COVERED | C-21 |
| R1-§4-04 | Sidebar items Dictation…Scratchpad (task-grouped) | COVERED | C-01..C-09 |
| R1-§4-05 | Sidebar "Set up Notetaker" checklist (tour, calendar, import, MCP) | COVERED | C-10, B-44 |
| R1-§4-06 | Invite your team (sidebar) | COVERED | C-11 |
| R1-§4-07 | Get a free month (sidebar) | COVERED | C-11 |
| R1-§4-08 | Settings (sidebar) | COVERED | C-12 |
| R1-§4-09 | Help (sidebar) | COVERED | C-13 |
| R1-§4-10 | Top-right dark-mode toggle | COVERED | C-16 |
| R1-§4-11 | Notifications bell | COVERED | C-17 |
| R1-§4-12 | Profile menu | COVERED | C-18 |
| R1-§4-13 | Sidebar-collapse button by window controls | COVERED | C-14 |
| R1-§4-14 | "Welcome back, <name>" | COVERED | C-28 |
| R1-§4-15 | Dismissible promo banner on home | COVERED | C-29 |
| R1-§4-16 | Notetaker page: Start Notetaker button | COVERED | E-30, E-05 |
| R1-§4-17 | Notetaker Today: "Try Notetaker" card (Skip/Start) | PARTIAL | E-30, E-43 |
| R1-§4-18 | Stat card: total words | COVERED | C-30 |
| R1-§4-19 | Stat card: WPM | COVERED | C-31 |
| R1-§4-20 | Stat card: day streak | COVERED | C-32 |
| R1-§4-21 | "Your Voice Profile" progress bar (unlocks after word count) | COVERED | C-33 |
| R1-§4-22 | History grouped by date | COVERED | C-35 |
| R1-§4-23 | Row: time + full transcript | COVERED | C-34 |
| R1-§4-24 | History search icon | COVERED | C-36 |
| R1-§4-25 | Row actions revealed on hover | COVERED | C-37 |
| R1-§4-26 | Row: play audio | COVERED | C-38 |
| R1-§4-27 | Row: copy | COVERED | C-39 |
| R1-§4-28 | Row: flag | COVERED | C-40 |
| R1-§4-29 | Row: ⋮ menu | COVERED | C-41 |
| R1-§4-30 | ⋮ Undo AI edit | COVERED | C-42, D-07 |
| R1-§4-31 | ⋮ Retry transcript | COVERED | C-43, D-08 |
| R1-§4-32 | ⋮ Delete transcript | COVERED | C-44 |
| R1-§4-33 | ⋮ Extract audio | COVERED | C-45 |
| R1-§4-34 | Raw audio kept per dictation; cleanup reversible | COVERED | C-46, D-06, E-44 |
| R1-§4-35 | Notetaker Today, calendar-driven; "No meetings today" | COVERED | E-30 |
| R1-§4-36 | Notetaker tab Past notes | COVERED | E-31 |
| R1-§4-37 | Notetaker tab Shared with me | COVERED | E-32 |
| R1-§4-38 | Notetaker tab Folders | COVERED | E-33 |
| R1-§4-39 | Insights tabs Your usage / Your voice | COVERED | C-50 |
| R1-§4-40 | WPM with percentile | COVERED | C-51 |
| R1-§4-41 | Fixes made by Flow (words corrected, dictionary fixes) | COVERED | C-52, D-57 |
| R1-§4-42 | Total words desktop vs mobile | COVERED | C-53 |
| R1-§4-43 | Usage by category | COVERED | C-54, D-62 |
| R1-§4-44 | Streak calendar | COVERED | C-55 |
| R1-§4-45 | Insights Share | COVERED | C-56 |
| R1-§4-46 | Voice Profile locked until ~1.7K words, blurred cards | COVERED | C-57, D-63 |
| R1-§4-47 | Card: Catchphrase | COVERED | C-57 |
| R1-§4-48 | Card: Peak time and place | COVERED | C-57 |
| R1-§4-49 | Card: Most used word | COVERED | C-57 |
| R1-§4-50 | Snippets page "same layout" as Dictionary (tabs, search/sort/refresh) | PARTIAL | C-69 |
| R1-§4-51 | Card: Most corrected word | COVERED | C-57 |
| R1-§4-52 | Dictionary tabs All / Personal / Shared with team | COVERED | C-59, D-51 |
| R1-§4-53 | Dictionary Import | COVERED | C-64, D-50 |
| R1-§4-54 | Dictionary Add new | COVERED | C-62 |
| R1-§4-55 | Dictionary search / sort / refresh | COVERED | C-60 |
| R1-§4-56 | Banner "Flow spells the way you do" with chips | COVERED | C-61 |
| R1-§4-57 | Dictionary single-word entries | COVERED | C-58, D-47 |
| R1-§4-58 | Dictionary "from → to" entries | COVERED | C-63, D-48 |
| R1-§4-59 | Snippets: spoken trigger → expansion | COVERED | C-72, D-53 |
| R1-§4-60 | Snippets banner with three examples | COVERED | C-73 |
| R1-§4-61 | Style tabs Personal / Work / Email / Other / Auto cleanup | COVERED | D-18, D-05 |
| R1-§4-62 | Style tab applies to app set shown as icons | COVERED | D-19 |
| R1-§4-63 | "+" to add apps | COVERED | D-20 |
| R1-§4-64 | Default app membership per group | COVERED | D-19 |
| R1-§4-65 | Personal presets Formal / Casual / Very casual | COVERED | D-23 |
| R1-§4-66 | Work/Email/Other presets Formal / Casual / Excited! | COVERED | D-24 |
| R1-§4-67 | Presets as before/after cards | COVERED | D-25 |
| R1-§4-68 | Note: style formatting English only | COVERED | D-26 |
| R1-§4-69 | Auto cleanup on every dictation in every app | COVERED | D-01 |
| R1-§4-70 | Level None | COVERED | D-02 |
| R1-§4-71 | Level Light | COVERED | D-03 |
| R1-§4-72 | Level Medium | COVERED | D-04 |
| R1-§4-73 | "Original never lost" note | COVERED | D-06 |
| R1-§4-74 | Transforms opt-in toggle | COVERED | D-28 |
| R1-§4-75 | Transforms page hint "Opt o to view changes" | COVERED | D-40 |
| R1-§4-76 | Banner "Transform works anywhere you write" (Try it out / How it works) | COVERED | B-38, B-39, B-40, D-43 |
| R1-§4-77 | Auto Apply After Dictation toggle + chosen transform | COVERED | D-42 |
| R1-§4-78 | My Transforms: Polish (⌥1, "Improve clarity and conciseness") | COVERED | D-33, D-32 |
| R1-§4-79 | Launch popup "Dark mode is finally here" | PARTIAL | C-99, C-20 |
| R1-§4-80 | My Transforms: Prompt Engineer (⌥2) | COVERED | D-37 |
| R1-§4-81 | Create your own / Create New | COVERED | D-38 |
| R1-§4-82 | Reset to defaults (transforms) | COVERED | D-38 |
| R1-§4-83 | Scratchpad "Add to Flow Bar" toggle | COVERED | C-74, A-03 |
| R1-§4-84 | Scratchpad shortcut button | COVERED | C-74, C-77 |
| R1-§4-85 | Scratchpad "Start new note" | COVERED | C-74 |
| R1-§4-86 | Scratchpad Recents (title, preview, date) | COVERED | C-74 |
| R1-§4-87 | Notifications panel filter and "…" | COVERED | C-17 |
| R1-§4-88 | Notifications empty state "You're all caught up!" | COVERED | C-17 |
| R1-§4-89 | Profile: name and email | COVERED | C-18 |
| R1-§4-90 | Profile: free month / refer a friend | COVERED | C-18, C-11 |
| R1-§4-91 | Profile: Manage account | COVERED | C-18 |
| R1-§4-92 | Dark theme exists | COVERED | C-20, A-68 |
| R1-§4-93 | Beta tags on Style/Transforms/Scratchpad | COVERED | C-15 |
| R1-§5-01 | Settings modal, two columns | COVERED | C-12 |
| R1-§5-02 | General: Shortcuts row "Hold fn and speak", Change | COVERED | C-80 |
| R1-§5-03 | General: Microphone "Built-in mic (recommended)", Change | COVERED | C-81, A-37, A-38 |
| R1-§5-04 | General: My languages, Change | COVERED | C-82, B-61 |
| R1-§5-05 | General: App Language dropdown | COVERED | C-83, B-67 |
| R1-§5-06 | General: Theme dropdown | COVERED | C-84 |
| R1-§5-07 | System: Launch app at login | COVERED | A-64, B-35, C-85, E-87 |
| R1-§5-08 | System: Show Flow Bar at all times | COVERED | A-66, C-86 |
| R1-§5-09 | System: Show app in dock | COVERED | A-65, C-87 |
| R1-§5-10 | Sound: Dictation and notification sounds (on) | COVERED | A-42, C-89 |
| R1-§5-11 | Sound: Mute all audio while dictating (off) | COVERED | A-41, C-90 |
| R1-§5-12 | Notifications: Suggestions | COVERED | C-91 |
| R1-§5-13 | Notifications: Announcements | COVERED | C-91 |
| R1-§5-14 | Notifications: Milestones | COVERED | C-91 |
| R1-§5-15 | System: Scratchpad open behavior | COVERED | C-78 |
| R1-§5-16 | Notetaker: notify before scheduled meetings (15 s) | COVERED | E-07 |
| R1-§5-17 | Notetaker: next meeting in menu bar | COVERED | E-07 |
| R1-§5-18 | Notetaker: auto-detect any call | COVERED | E-06 |
| R1-§5-19 | Notetaker: start automatically | COVERED | E-08 |
| R1-§5-20 | Notetaker: max recording length 2 h | COVERED | E-10 |
| R1-§5-21 | Notetaker: stop when call ends | COVERED | E-09 |
| R1-§5-22 | Notetaker: hide Notepad and Flow Bar from screen capture | COVERED | A-20, E-28 |
| R1-§5-23 | Notetaker: Notepad view options | COVERED | E-35 |
| R1-§5-24 | Notetaker: Start tutorial | COVERED | E-43, B-45 |
| R1-§5-25 | Vibe coding: variable recognition (VS Code/Cursor/Windsurf), Set up | COVERED | D-59, C-97 |
| R1-§5-26 | Vibe coding: file tagging in chat | COVERED | D-60, C-97 |
| R1-§5-27 | Connectors: Google Calendar, Outlook Calendar | COVERED | E-40, C-97 |
| R1-§5-28 | Connectors: Slack | COVERED | C-97 |
| R1-§5-29 | Connectors: "Give your AI access… Go to MCP" | COVERED | E-41 |
| R1-§5-30 | MCP: Add to Claude/ChatGPT/Gemini/Cursor, All other apps | COVERED | E-41, C-97 |
| R1-§5-31 | MCP: states no access to dictations | COVERED | E-41 |
| R1-§5-32 | Account page (name, email, picture, Save, Sign out, Delete account) | COVERED | C-96 |
| R1-§5-33 | Team page (invite, 2-week Pro trial, FAQ) | COVERED | C-96, C-11 |
| R1-§5-34 | Plans: Monthly/Annual toggle | COVERED | C-96 |
| R1-§5-35 | Plan Free contents | COVERED | C-96 |
| R1-§5-36 | Plan Pro ($12) | COVERED | C-96 |
| R1-§5-37 | Plan Growth & Enterprise | COVERED | C-96 |
| R1-§5-38 | Data: "Improve the model for everyone" (off) | COVERED | D-66 |
| R1-§5-39 | Data: Context awareness (on) | COVERED | C-95, D-58 |
| R1-§5-40 | Data: "Dictation cloud storage" (off) | PARTIAL | C-46 |
| R1-§5-41 | Data: "Show your public information" (on) | MISSING | – |
| R1-§5-42 | Data: Local data storage dropdown "Store data locally" | COVERED | E-64 |
| R1-§5-43 | Data: Sync notes | MISSING | – |
| R1-§5-44 | Data: Download your data (by email) | COVERED | C-94, E-66 |
| R1-§5-45 | Data: Delete your data | COVERED | C-94, E-67 |
| R1-§5-46 | Footer "Flow v1.6.1264" | COVERED | C-27, E-51 |
| R1-§5-47 | Footer Update button | COVERED | C-27, E-51 |
| R1-§6-01 | App menus File/Edit/Dictation/Notetaker/My Voice/View/Help/Window | PARTIAL | A-63 |
| R1-§6-02 | Dictation menu: "Words dictated: N" | COVERED | A-63, C-106 |
| R1-§6-03 | Dictation menu: Copy last transcript with preview | COVERED | A-63, C-106 |
| R1-§6-04 | Dictation menu: Microphone ▸ | COVERED | A-63, C-106 |
| R1-§6-05 | Dictation menu: Languages ▸ | COVERED | A-63, C-106, B-61 |
| R1-§6-06 | My Voice: Dictionary words N | COVERED | A-63, C-105, D-64 |
| R1-§6-07 | My Voice: Snippets created N | COVERED | A-63, C-105, D-64 |
| R1-§6-08 | My Voice: Add a word to dictionary | COVERED | A-63, C-105, D-64 |
| R1-§6-09 | My Voice: Create a new snippet | COVERED | A-63, C-105, D-64 |
| R1-§6-10 | Help menu: search box | COVERED | A-63 |
| R1-§6-11 | Help menu: Help center | COVERED | A-61, C-102 |
| R1-§6-12 | Help menu: Talk to support (⌘/) | COVERED | A-61, C-103, E-90 |
| R1-§6-13 | Help menu: Share feedback | COVERED | A-61, C-103 |
| R1-§6-14 | Help menu: Shortcuts opens hotkey panel | COVERED | C-100, A-33 |
| R1-§6-15 | Menu-bar status icon exists | COVERED | A-55 |
| R1-§7-01 | Cloud features: account, plans, teams, referral, connectors, MCP, sync, Notetaker storage | COVERED | C-96, C-97, C-11, E-40, E-41 |
| R1-§7-02 | Product separates local and cloud data (Store locally, opt-in toggles) | COVERED | C-93, E-64, E-68 |
| R1-§8-01 | No decisions at use time; language and mic preselected | COVERED | B-62, A-39, A-38, B-47 |
| R1-§8-02 | Pill confirms active mic | COVERED | A-10 |
| R1-§8-03 | Soft failure; history retry/undo/extract | COVERED | A-15, C-42, C-43, C-45 |
| R1-§8-04 | Corrections feed back visibly | COVERED | C-52, D-57 |
| R1-§8-05 | Visual example-led personalization | COVERED | D-25, C-23 |
| R1-§8-06 | Three-level cleanup instead of prompts | COVERED | D-05 |
| R1-§8-07 | Empty states and banners teach | COVERED | C-23, C-24, B-38 |
| R1-§8-08 | Calm sidebar; settings grouped by task | COVERED | C-01, C-79 |
| R1-§9-01 | Candidate core scope list | COVERED | (classes across A-D) |
| R1-§9-02 | Candidate out-of-scope list | COVERED | C-96, C-97, E OUT rows |
| R1-§10-01 | Pattern: banner + How it works + Try it out per feature | COVERED | B-38, B-39, B-40 |
| R1-§10-02 | Help popover: Updates (What's New) | COVERED | C-13, C-99 |
| R1-§10-03 | Help popover: Essentials (Shortcuts, Microphone, Languages) | COVERED | C-100, C-101 |
| R1-§10-04 | Help popover: Help Center, Talk to support | COVERED | C-102, C-103 |
| R1-§10-05 | Help popover: Contact sales | PARTIAL | C-13, C-103 |
| R1-§10-06 | What's New opens web page | COVERED | C-99, E-89 |
| R1-§10-07 | User-reported: rename to "Help and Feedback", contact + feedback only | COVERED | C-13, A-61, E-90 |
| R1-§10-08 | Notetaker "Take 2-min tour" checklist entry | COVERED | E-43, B-44 |
| R1-§10-09 | Gate "Complete setup to start Notetaker", "These are required settings" | COVERED | E-02, E-43 |
| R1-§10-10 | Gate item Enable System Audio "Capture what others say", Turn on, prompt illustration | COVERED | E-02, B-14, B-10 |
| R1-§10-11 | Gate footer consent + privacy policy | COVERED | E-19 |
| R1-§10-12 | "No meetings found: Add calendar…" + Connect calendar | COVERED | E-42 |
| R1-§10-13 | Notetaker settings lower sections (user-reported) | COVERED | E-35..E-38 |
| R1-§10-14 | Transforms "How it works" modal with hosted video (speed, fullscreen, PiP) | COVERED | B-39, D-43 |
| R1-§10-15 | Video: diff card "8 Changes", strike/highlight | COVERED | D-40 |
| R1-§10-16 | Diff card actions: Configure Polish, copy, thumbs, retry | COVERED | D-41 |
| R1-§10-17 | Try it step 1: three cards Original/Polish/Prompt Engineer + benefit ticker | PARTIAL | D-43, B-40 |
| R1-§10-18 | Try it: two-segment progress bar, close X, CTA | COVERED | B-40 |
| R1-§10-19 | Try it step 2: "Highlight text, then press ⌥1" + sample composer | COVERED | B-40, D-43 |
| R1-§10-20 | "Transformed!" heading with see-changes, ⌥O and ⌘Z hints | COVERED | D-31, D-40, D-43 |
| R1-§10-21 | Screen dims; spotlight on pill showing "Done. See changes" | COVERED | A-14, A-16, B-41, D-39 |
| R1-§10-22 | Polish modal left: name, "⌥1 to use", description, example, See Updates | PARTIAL | D-33, D-34 |
| R1-§10-23 | Final CTA "Turn on Transforms" | COVERED | B-40 |
| R1-§10-24 | Polish modal: Autosave On, Reset | COVERED | D-36 |
| R1-§10-25 | Polish modal: Choose a keyboard shortcut | COVERED | D-32 |
| R1-§10-26 | Polish modal: Select rules toggles (5) | COVERED | D-33 |
| R1-§10-27 | Polish modal: Customize your Polish prompt | COVERED | D-34 |
| R1-§10-28 | Polish modal: Add writing examples (Upload Example) | COVERED | D-35, D-65 |
| R1-§10-29 | Transform replaces selection in place | COVERED | D-29, D-30 |
| R1-§10-30 | ⌘Z undoes transform | COVERED | D-31 |
| R1-§10-31 | Diff view from pill or ⌥O | COVERED | D-40, A-14 |
| R1-§10-32 | Scratchpad Recents hover actions: edit, pin, delete | COVERED | C-74 |
| R1-§10-33 | Scratchpad floating window: expand and close buttons | PARTIAL | C-75 |
| R1-§10-34 | Scratchpad window rail: sidebar toggle, new note | PARTIAL | C-75 |
| R1-§10-35 | Scratchpad window: tabs "Untitled ×", "+" | COVERED | C-75 |
| R1-§10-36 | Scratchpad window rail: search, AI sparkle, text size | COVERED | C-75 |
| R1-§10-37 | Scratchpad window: text area + Copy | COVERED | C-75, C-76 |
| R1-§10-38 | Bar with Scratchpad on: three round buttons (Dictate, Notetaker w/ teal badge, Scratchpad) | PARTIAL | A-03, E-29 |
| R1-§10-39 | Hover tooltips "Dictate fn", "Scratchpad" | COVERED | A-02 |
| R1-§10-40 | User-reported: referral → "share with a friend" | COVERED | C-11, B-30 |
| R1-§10-41 | User-reported: Data and Privacy → short all-local page | COVERED | C-93, E-68 |
| R1-§11-01 | Dictionary dialog "Add to vocabulary" | COVERED | C-62 |
| R1-§11-02 | Toggle "Correct a misspelling" reveals Misspelling → Correct spelling | COVERED | C-63, D-48 |
| R1-§11-03 | Dialog toggle "Share with team" | COVERED | C-59, D-51 |
| R1-§11-04 | "Add a new word" field; Cancel / Add word (disabled until filled) | COVERED | C-62 |
| R1-§11-05 | Import wizard: 4 steps, share toggle, .csv browse/drag, max 3 MB, format link | COVERED | C-64, D-50 |
| R1-§11-06 | Snippet dialog: share toggle, name/trigger field | COVERED | C-70, D-55 |
| R1-§11-07 | Snippet rich-text body (bold, italic, lists, link) | COVERED | C-70, D-54 |
| R1-§11-08 | Snippet 4,000-char counter; Cancel / Add snippet | COVERED | C-70, D-54 |
| R1-§11-09 | Snippets Import | COVERED | C-71, D-55 |
| R1-§11-10 | Style "+" did not respond | COVERED | D-20 |
| R1-§11-11 | Scratchpad shortcut dialog proposes ⌥S, pencil, Done | COVERED | C-77 |
| R1-§11-12 | Invite your team opens Settings > Team (3 email fields, Add more, Continue, trial, FAQ, Contact us) | COVERED | C-11, C-96 |
| R1-§11-13 | Notetaker setting: notify before scheduled meetings | COVERED | E-07 |
| R1-§11-14 | Notetaker setting: show next meeting in menu bar | COVERED | E-07 |
| R1-§11-15 | Notetaker setting: auto-detect any call | COVERED | E-06 |
| R1-§11-16 | Notetaker setting: auto-start | COVERED | E-08 |
| R1-§11-17 | Notetaker setting: max length default 2 h | COVERED | E-10 |
| R1-§11-18 | Notetaker setting: stop when call ends (on) | COVERED | E-09 |
| R1-§11-19 | Notetaker setting: hide from screen capture (off) | COVERED | E-28 |
| R1-§11-20 | Notepad default view dropdown; side-by-side | COVERED | E-35 |
| R1-§11-21 | Keyboard shortcut section "Press Opt + M…", Change shortcut | COVERED | E-05 |
| R1-§11-22 | Transcript: Show live transcript (on) | COVERED | E-36 |
| R1-§11-23 | Consent: Calendar notice dropdown (Off) | COVERED | E-37 |
| R1-§11-24 | Sharing: Default for Notetaker links | COVERED | E-38 |
| R1-§11-25 | Sharing: Auto-share notes | COVERED | E-38 |
| R1-§11-26 | Import your meeting notes | COVERED | E-39 |
| R1-§11-27 | Start tutorial button top right | COVERED | E-43, B-45 |
| R1-§11-28 | Turn on → System Settings > Screen & System Audio; app restarts; gate gone | COVERED | E-02, B-13 |
| R1-§11-29 | Tour: presenter video overlay in small floating window | COVERED | E-43 |
| R1-§11-30 | Tour: simulated meeting in docked right panel with timer | COVERED | E-43, E-12 |
| R1-§11-31 | Tour: notification-style "Join meeting" prompt | COVERED | E-11 |
| R1-§11-32 | Tour: Transcript fills with "Them"/"You" bubbles | COVERED | E-16, E-15 |
| R1-§11-33 | Tour banner "After the meeting, Wispr will improve the transcript and label speakers" | COVERED | E-20 |
| R1-§11-34 | Transcript search, translate, copy icons | COVERED | E-15, E-45, E-46 |
| R1-§11-35 | "Ask anything" + "What did I miss?" chip with recap | COVERED | E-18 |
| R1-§11-36 | Stop button, then Summary tab | COVERED | E-17, E-23 |
| R1-§11-37 | Summary: "1 MIN READ", headline, bullets, Next steps with owners | COVERED | E-23 |
| R1-§11-38 | Summary: Ask anything bar, "1 hour meeting compressed to a 2 min read" footer | COVERED | E-18, E-23 |
| R1-§11-39 | Tabs My thoughts / Transcript / Summary | COVERED | E-14, E-15, E-23 |
| R1-§11-40 | Header: back arrow, "…" menu, side-by-side toggle | COVERED | E-12, E-35 |
| R1-§11-41 | Header: Connect calendar, Share, copy link | COVERED | E-42, E-38 |
| R1-§11-42 | Title with date chip and "Add folder" | COVERED | E-13, E-33 |
| R1-§11-43 | Tour ends: meeting under Past notes, checklist item ticked | COVERED | E-31, B-44 |
| R1-§11-44 | Connectors settings page opens automatically as next step | MISSING | – |
| R1-§11-45 | Floating pill with waveform + record dot while Notetaker records | COVERED | A-21, E-29 |
| R1-§11-46 | "Show me how" banner, "Working around other people?" | COVERED | C-29, C-49, B-42 |
| R1-§11-47 | Whisper step 1: "Flow works even when you speak softly", looping video | COVERED | B-42 |
| R1-§11-48 | Whisper step 2: how to whisper with built-in mic, "Give it a try" | COVERED | B-42 |
| R1-§11-49 | Whisper step 3: simulated Gmail composer, Continue disabled until dictation lands | COVERED | B-42 |
| R1-§11-50 | Whisper modal progress dots, close X | COVERED | B-42 |
| R1-§11-51 | Tray: See all upcoming meetings | COVERED | A-58, E-07 |
| R1-§11-52 | Tray: Start Notetaker | COVERED | A-58, E-05 |
| R1-§11-53 | Tray: Open Wispr Flow | COVERED | A-59 |
| R1-§11-54 | Tray: Check for updates… | COVERED | A-60, E-52 |
| R1-§11-55 | Tray: Paste last transcript ⌃⌘V, disabled until one exists | COVERED | A-26, A-56, A-57 |
| R1-§11-56 | Tray: Shortcuts | COVERED | A-56 |
| R1-§11-57 | Tray: Microphone ▸ | COVERED | A-56 |
| R1-§11-58 | Tray: Languages ▸ | COVERED | A-56 |
| R1-§11-59 | Tray: Help center | COVERED | A-56, A-61 |
| R1-§11-60 | Tray: Talk to support (⌘/) | COVERED | A-61 |
| R1-§11-61 | Tray: Share feedback | COVERED | A-61 |
| R1-§11-62 | Tray: Hide next meeting from menu bar | COVERED | A-58, E-07 |
| R1-§11-63 | Tray: Quit (⌘Q) | COVERED | A-62 |
| R1-§11-64 | File and View menus unknown | COVERED | A open Q9 |
| R1-§11-65 | Update flow: footer version + Update; cloud icon after update | COVERED | E-51 |
| R1-§11-66 | Version changed after restart | COVERED | E-53 |
| R1-§11-67 | Pattern: guided modal + practice in simulated apps | COVERED | B-40 |
| R1-§11-68 | Pattern: permission explained with macOS picture first | COVERED | B-10 |
| R1-§11-69 | Pattern: empty states with one action + example banner | COVERED | C-24, C-23 |
| R1-§11-70 | Pattern: one consistent chrome | COVERED | C-19, C-21, C-22 |
| R1-§11-71 | Pattern: bottom pill as single entry point | COVERED | A-01, A-03 |
| R1-§12-01 | Quiz modal chrome: back arrow, progress dashes, serif heading, Continue | COVERED | B-43 |
| R1-§12-02 | "Whispering is easier with the right microphone" (4 photos) | COVERED | B-43 |
| R1-§12-03 | "Do you work next to other people?" (3 cards) | COVERED | B-43 |
| R1-§12-04 | "Do you work at the same desk every day?" (2 cards) | COVERED | B-43 |
| R1-§12-05 | "Select all the mics that you own" multi-select (8 cards, lilac selection) | COVERED | B-43 |
| R1-§12-06 | Recommendation table (stars, whisper rows, price), "Best match", "See here" | COVERED | B-43 |
| R1-§12-07 | Done / Email results | COVERED | B-43 |
| R1-§12-08 | Whispered dictation appears in real history | PARTIAL | B-42 |
| R1-§12-09 | Set up MCP → Settings > MCP | COVERED | E-41 |
| R1-§12-10 | Import from Granola/Otter dialog "Where would you like to upload from?" | COVERED | E-39 |
| R1-§12-11 | Insight: onboarding as conversation with a result | COVERED | B-43 |
| R1-§12-12 | Privacy note on captured frames | N/A | – |
| R1-§13-01 | Welcome / account screens unknown | COVERED | B-05 |
| R1-§13-02 | Five-stage named stepper (SIGN UP → GET STARTED) | COVERED | B-01 |
| R1-§13-03 | Thin purple progress line | COVERED | B-01 |
| R1-§13-04 | Back link top-left | COVERED | B-02 |
| R1-§13-05 | Help chip bottom-left | COVERED | B-02 |
| R1-§13-06 | Left controls, right illustration/demo/macOS screenshot | COVERED | B-03 |
| R1-§13-07 | Black pill primary, grey until choice | COVERED | B-04 |
| R1-§13-08 | Learn screens dark theme | COVERED | B-03 |
| R1-§13-09 | "How can Flow help you?" Dictation / Notetaker multi-select | COVERED | B-06 |
| R1-§13-10 | Right side animates demo per selected card | COVERED | B-06, B-24 |
| R1-§13-11 | "How do you currently record meetings?" chips (conditional) | COVERED | B-07, E-50 |
| R1-§13-12 | "What's frustrating about meetings?" | COVERED | B-08, E-50 |
| R1-§13-13 | Permissions heading "Thanks for trusting us…" | COVERED | B-09 |
| R1-§13-14 | Row: recognize meetings | COVERED | B-15 |
| R1-§13-15 | Row: use microphone | COVERED | B-09, B-11 |
| R1-§13-16 | Row: capture meeting audio | COVERED | B-14, E-02 |
| R1-§13-17 | Rows tick as granted | COVERED | B-09, B-46 |
| R1-§13-18 | Right side: macOS Settings picture + exact system prompt | COVERED | B-10 |
| R1-§13-19 | Real macOS prompts follow | COVERED | B-11 |
| R1-§13-20 | Connect your calendar (benefits, Google, Outlook, different, mock) | COVERED | B-16, E-40 |
| R1-§13-21 | Google OAuth in browser, "You're now logged in", reopen prompt | COVERED | B-16 |
| R1-§13-22 | "Different calendar" → iCloud / Something else / none | COVERED | B-16, E-40 |
| R1-§13-23 | Test mic copy: built-in or wired, Bluetooth less reliable | COVERED | B-20, A-38 |
| R1-§13-24 | "Do you see purple bars?" live level meter | COVERED | B-19, A-40 |
| R1-§13-25 | Buttons Change microphone / Yes | COVERED | B-19, B-20 |
| R1-§13-26 | Change list: Built-in (recommended), Auto-detect (system setting), others | COVERED | B-20, A-37, A-39 |
| R1-§13-27 | Shortcut test: Yes/No; "No" path | PARTIAL | B-21, B-26 |
| R1-§13-28 | "Hold fn while you speak"; fn/globe key graphic turns purple | COVERED | B-21, A-35 |
| R1-§13-29 | Edit shortcut opens hotkey panel | COVERED | B-22, A-33 |
| R1-§13-30 | Reset to default chip if changed | COVERED | B-22 |
| R1-§13-31 | "Write an email hands free", Skip top-right | COVERED | B-25 |
| R1-§13-32 | Simulated Gmail composer (To, Subject), "Click into the text field" | COVERED | B-25 |
| R1-§13-33 | "Formats for you and fixes mistakes" | COVERED | D-14, D-16, D-03 |
| R1-§13-34 | "Nice job! You just spoke." dark celebration | COVERED | B-27 |
| R1-§13-35 | "Save 0 hours a week" counter | COVERED | B-28 |
| R1-§13-36 | 30-s typing test | COVERED | B-29 |
| R1-§13-37 | Typing test language picker, "100+ languages" | COVERED | B-29, B-65 |
| R1-§13-38 | Sample paragraph, live words/min, seconds left | COVERED | B-29 |
| R1-§13-39 | Recalculate time savings, Try Again | COVERED | B-29 |
| R1-§13-40 | "Save 25 hours" + Typing time/day slider (default 6 h) | COVERED | B-28 |
| R1-§13-41 | "Don't believe us? Test your typing speed" | COVERED | B-29 |
| R1-§13-42 | Referral screen (link, Copy, Continue) | COVERED | B-30, C-11 |
| R1-§13-43 | "How would you like to use Flow first?" 7 chips | COVERED | B-31 |
| R1-§13-44 | Chip swaps a looping captioned demo video | COVERED | B-31 |
| R1-§13-45 | Try it now | COVERED | B-31 |
| R1-§13-46 | Next-step coaching: dim, spotlight pill, hint per choice | COVERED | B-32, A-16 |
| R1-§13-47 | "Try Flow in another app / Get started" banner | COVERED | B-33, C-29 |
| R1-§13-48 | First real dictations in history | COVERED | C-34 |
| R1-§13-49 | End state hub with stats | COVERED | B-34, C-02 |
| R1-§13-50 | Notetaker sidebar item tagged "New!" | COVERED | C-15, B-34 |
| R1-§13-51 | Insight: each setup step is a yes/no test | COVERED | B-19, B-21 |
| R1-§13-52 | Insight: first dictation in simulated app, celebrated | COVERED | B-25, B-27 |
| R1-§13-53 | Insight: ~5 min, 14 screens, single-choice | COVERED | B-37 |
| R1-§13-54 | Local product: drop sign-up etc.; silent model choice | COVERED | B-47, B-50 |
| R1-§13-55 | Privacy note on frames | N/A | – |
| R1-§14-01 | Start: mic indicator on, docked right panel | COVERED | E-12 |
| R1-§14-02 | Title "New note" until named | COVERED | E-13 |
| R1-§14-03 | Flow Bar switches to recording state | COVERED | E-29, A-21 |
| R1-§14-04 | Panel header: traffic lights, back, "…", side-by-side, Connect calendar, Share, copy link | COVERED | E-12, E-38, E-42 |
| R1-§14-05 | Editable title, "Today" chip, "Add folder" | COVERED | E-13, E-33 |
| R1-§14-06 | My thoughts default tab with placeholder | COVERED | E-14 |
| R1-§14-07 | Tab state icons (Transcript waveform, Summary sparkle) | PARTIAL | E-15, E-23 |
| R1-§14-08 | Footer Stop (green square) | COVERED | E-17 |
| R1-§14-09 | Footer Ask anything + "What did I miss?" | COVERED | E-18 |
| R1-§14-10 | Footer "Always get consent… Learn more." | COVERED | E-19 |
| R1-§14-11 | Lilac banner "We'll improve the transcript…" [Explore →] | COVERED | E-20 |
| R1-§14-12 | Screen-share toast with Update settings and X | COVERED | E-27 |
| R1-§14-13 | Update settings opens Settings > Notetaker | COVERED | E-27 |
| R1-§14-14 | Transcript tab "Listening…" | COVERED | E-15 |
| R1-§14-15 | Tip card "Speaker names are added later", Next + dots | COVERED | E-20 |
| R1-§14-16 | Timer at top with search/translate/copy | COVERED | E-15 |
| R1-§14-17 | On stop: auto-titled | COVERED | E-13 |
| R1-§14-18 | Stop becomes Resume | COVERED | E-17 |
| R1-§14-19 | "Started by mistake?" toast, Discard / Keep | COVERED | E-26 |
| R1-§14-20 | Orange banner "We hit a snag…" with retry | COVERED | E-25 |
| R1-§14-21 | Summary processing: "Transcript ready, step 2 of 3", skeleton | COVERED | E-24 |
| R1-§14-22 | "Turning your 1-minute meeting into a two-minute read" | COVERED | E-24 |
| R1-§14-23 | Meeting appears under Today / Past notes | COVERED | E-31 |
| R1-§14-24 | Not captured: typing in My thoughts, long transcript, folder/share dialogs | COVERED | E-14, E-33, E-38 (unknowns) |
| R1-§14-25 | Insight: Notetaker is a second product; later milestone; needs local LLM | COVERED | E-49, C-03, A-21 |
| R1-§OI-01 | Onboarding not observed (superseded by §13) | N/A | – |
| R1-§OI-02 | Keyboard behavior: thresholds, hold-to-lock, held-state pill | COVERED | A-34, A open Q1-2 |
| R1-§OI-03 | Not opened: Command Mode, Transform diff window, idle hover | COVERED | D-44, D-40, A-02 |
| R1-§OI-04 | Side effects of the pass | N/A | – |
