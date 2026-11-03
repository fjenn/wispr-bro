# Gap audit: Wispr Bro next to Wispr Flow, screen by screen

Design audit, 2026-11-03, on `design/audit` (= `wispr-bro` at `da4574a8`, rc2 code). The owner's verdict on the release candidate: the flows are right, the look is not. This file says concretely why each screen looks cheaper than its Wispr Flow counterpart and what to change, ranked by visual impact. Measurements and the reference language: [REFERENCE-SPEC.md](REFERENCE-SPEC.md). The plan to fix it: [SYSTEM-PLAN.md](SYSTEM-PLAN.md).

**How this was made.** Every screen of ours was rendered by the Playwright fork suite (mocked IPC, installed Chrome, 2× device scale) in light and dark: Hub pages at 1040 × 720, onboarding steps at 1040 × 720, the bar's web view at each state's exact size from `geometry.gen.json`, the Scratchpad note window at the spec's default viewport (wider than its real 560 × 440; the layout gaps are the same). The Wispr Flow side is the owner's local screen recordings (`docs/research/evidence/wispr-flow/`, never committed). A side-by-side comparison page with both sets of images was built for the owner's local viewing only (it lives in the session scratch folder, not in the repo).

**What does not change.** Flows, steps, feature set and our copy's honesty rules stay. Our screens already beat the reference on some things (permission pictures for macOS 13 and 26, privacy messaging, keyboard and VoiceOver support, RTL); the rebuild must keep all of that, including the axe/contrast gates.

## 0. Pairing table

Frame ids refer to the local evidence folder (`o-` onboarding, `cap-` tours, `u-` user-run, `02-`/`03-` bar crops). "—" means no counterpart.

| Our screen / state                                                               | Our source                                                      | Wispr Flow frame(s)                                          | Note                                                    |
| -------------------------------------------------------------------------------- | --------------------------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------- |
| Hub shell, sidebar, title strip                                                  | `hub/{ForkApp,Sidebar,Header,Footer}.tsx`                       | u-073, u-075, cap-001                                        |                                                         |
| Home: greeting, banner, stats rail, voice profile                                | `home/*`                                                        | u-073, o-0369                                                |                                                         |
| History list (rows, day groups, hover actions, search)                           | `history/*`                                                     | u-073                                                        | row menu (⋮) not captured                               |
| Insights › Usage / Voice                                                         | `insights/*`                                                    | u-075                                                        | Voice tab not captured                                  |
| Dictionary (page, Suggested tab, Add word, import wizard)                        | `dictionary/*`                                                  | —                                                            | not archived (R1); nearest: cap-001 list, o-0178 dialog |
| Snippets (page, dialog, import)                                                  | `snippets/*`                                                    | —                                                            | same                                                    |
| Style (four groups, Cleanup tab, app picker)                                     | `style/*`                                                       | u-080, u-099                                                 | Auto-cleanup tab not captured                           |
| Transforms page + editor                                                         | `transforms/*`                                                  | cap-024, cap-213                                             |                                                         |
| Scratchpad page                                                                  | `scratchpad/ScratchpadPage.tsx`                                 | cap-001, cap-240                                             |                                                         |
| Scratchpad note window                                                           | `scratchpad/NoteWindow.tsx`                                     | cap-231                                                      |                                                         |
| Share with a friend                                                              | `share/SharePage.tsx`                                           | o-0330 (closest: invite screen)                              | different purpose                                       |
| Settings modal (General, System, Vibe coding, Data and Privacy, Advanced, About) | `settings/*`                                                    | u-033 (MCP section only)                                     | other sections not archived                             |
| Hotkeys panel                                                                    | `settings/hotkeys/*`                                            | o-0178                                                       |                                                         |
| Notifications panel, toasts                                                      | `hub/NotificationsPanel.tsx`                                    | —                                                            | R1 describes it; n-\* frames show a dark toast          |
| Help popover, Report a problem                                                   | `hub/{HelpPopover,ReportProblemDialog}.tsx`                     | —                                                            |                                                         |
| What's new, feature popup                                                        | `whats-new/*`, `hub/FeaturePopup.tsx`                           | cap-007 (web page)                                           | theirs is a website                                     |
| Shortcut health banner, Fix sheet                                                | `hub/ShortcutHealthBanner.tsx`, `settings/FixShortcutSheet.tsx` | —                                                            |                                                         |
| Onboarding: Welcome                                                              | `onboarding/Welcome.tsx`                                        | o-0045                                                       |                                                         |
| Onboarding: Move to Applications                                                 | `onboarding/MoveToApplications.tsx`                             | —                                                            |                                                         |
| Onboarding: Languages                                                            | `onboarding/Languages.tsx`                                      | o-0103 (chips + illustration pattern)                        |                                                         |
| Onboarding: Permissions                                                          | `onboarding/Permissions.tsx`                                    | o-0130                                                       |                                                         |
| Onboarding: Mic test                                                             | `onboarding/MicTest.tsx`                                        | o-0160                                                       |                                                         |
| Onboarding: Shortcut test (+ diagnoses)                                          | `onboarding/ShortcutTest.tsx`                                   | o-0172, o-0178                                               | diagnoses have no counterpart                           |
| Onboarding: Practice                                                             | `onboarding/HandsFreePractice.tsx`                              | o-0192, o-0214, o-0230                                       |                                                         |
| Onboarding: First dictation                                                      | `onboarding/FirstDictation.tsx`                                 | o-0255…o-0262                                                |                                                         |
| Onboarding: Time saved                                                           | `onboarding/TimeSaved.tsx`                                      | o-0263…o-0274                                                |                                                         |
| Onboarding: Typing test                                                          | `onboarding/TypingTest.tsx`                                     | o-0275…o-0326                                                |                                                         |
| Onboarding: Summary                                                              | `onboarding/Summary.tsx`                                        | o-0330 (closing screen)                                      | different content                                       |
| First-use chooser                                                                | `onboarding/FirstUseChooser.tsx`                                | o-0333…o-0362                                                |                                                         |
| Bar spotlight                                                                    | `spotlight/*`                                                   | o-0363…o-0368, cap-200                                       |                                                         |
| Setup checklist                                                                  | `hub/SetupChecklist.tsx`                                        | u-073 (sidebar card)                                         |                                                         |
| Transforms lesson                                                                | `tutorials/lessons/*`, `LessonModal.tsx`                        | cap-183 (video), cap-190, cap-196, cap-200, cap-206, cap-213 |                                                         |
| Whisper lesson and quiz                                                          | `tutorials/lessons/whisper.tsx`, `MicQuiz.tsx`                  | u-002…u-028                                                  |                                                         |
| Bar: resting (native), three-mode buttons, tooltip                               | `src-tauri/src/fork/pill/native.rs`                             | 02-bar-click-01/02, 03-bar-hover                             | ours not rendered by Playwright                         |
| Bar: held, held + mic, locked (+ silence warning)                                | `pill/states/*`                                                 | o-0200, 02-bar-click-03…22                                   |                                                         |
| Bar: transcribing / cleaning up / setting up                                     | `pill/states/*`                                                 | u-065 (label + spinner)                                      |                                                         |
| Bar: messages (cancelled/undo, errors, done/view changes)                        | `pill/states/*`                                                 | o-0367 (hint label)                                          | errors not captured                                     |
| Bar: card (no text field, focus moved, secure field)                             | `pill/states/Card.tsx`                                          | —                                                            | described in R1 only                                    |
| Bar: changes (diff) card                                                         | `pill/states/*`                                                 | u-065…u-072                                                  |                                                         |
| Notetaker (theirs)                                                               | —                                                               | u-074, cap-011…022, n-\*                                     | out of scope (M2)                                       |

## 1. Top 15 changes across the app (by visual impact)

| #   | Change                                                                                                                                                                                                                                                                                                                         | Why it matters                                                                                                                                                                                                                                                                                                      | Screens                                                    |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| 1   | **Swap the type system:** Figtree for UI text, EB Garamond (with its italic) for display moments only; sans semibold page titles.                                                                                                                                                                                              | Our pages use Newsreader for every title and SF Pro for everything else. Newsreader's large x-height and heavy serifs read "document", and serif page titles everywhere flatten the hierarchy. The reference's look is one geometric grotesk plus a Garamond used sparingly; this single change moves every screen. | all                                                        |
| 2   | **Primary actions become black; indigo leaves fills.** Purple only for progress, selection and an emphasised word.                                                                                                                                                                                                             | Saturated indigo buttons, banners and the ✓ in the bar are the strongest "template UI" signal on our screens; the reference is ink on warm white with a restrained accent.                                                                                                                                          | all                                                        |
| 3   | **Hub shell: inset rounded content panel** (22 pt radius) on a warm chrome, 46 pt title strip with traffic lights centred, sidebar flush with the chrome.                                                                                                                                                                      | Ours is a flat two-tone page (sidebar strip + canvas) with a full-width footer bar and a lone collapse icon floating top-left; it looks like a web dashboard, not a Mac app.                                                                                                                                        | Hub, Settings, every page                                  |
| 4   | **Hero banners with imagery** (dark graded photo, serif headline, cream button) instead of flat lilac boxes with chip clouds.                                                                                                                                                                                                  | Our banners are the largest element on Home, Dictionary, Snippets, Transforms, and they are flat lavender rectangles with three pill chips and an indigo button. The reference banners carry the whole premium feel of a page.                                                                                      | Home, Dictionary, Snippets, Transforms, Scratchpad, Style  |
| 5   | **Onboarding layouts:** split screen (choices left, picture/demo right) and centred test-card screens on a patterned warm canvas, a five-stage stepper with a progress line, a 360 pt black CTA pinned low.                                                                                                                    | Ours centres a small column (icon-in-a-tile, bold title, a card, a button) in an empty 1040 pt window with a persistent footer sentence; every step looks the same and the window feels empty.                                                                                                                      | all 11 onboarding steps                                    |
| 6   | **Motion system** (`motion`): dialogs and sheets that spring in and out, page and step transitions, staged celebration, rolling numbers, hover/press feedback.                                                                                                                                                                 | Ours has almost no motion: 13 spinners, 6 colour transitions, nothing enters or leaves. Even with identical visuals, a static UI reads as cheap next to one that settles into place.                                                                                                                                | all                                                        |
| 7   | **The bar: smaller, opaque, black, quieter.** Rest 38 × 6 pt (ours 60 × 12), held 72 × 30 (ours 112 × 34), click/hands-free 100 × 30 with a white ✓ circle (ours 168 × 34, indigo ✓), mic label as a separate pill above that blurs in and leaves after ~2 s (ours: inside a 272 pt capsule). Drop the 55 % white edge stroke. | The bar is on screen all day; ours is grey, translucent, haloed and up to twice the size, with 12.5-13 px text where theirs uses 15-16 px in a smaller shape.                                                                                                                                                       | bar, card, diff, spotlight                                 |
| 8   | **One icon style:** Lucide at 1.5 stroke via a wrapper, 18 pt in nav, 16 in rows.                                                                                                                                                                                                                                              | Ours mixes 2 px Lucide at several sizes with custom glyphs and icon-in-tinted-tile badges (onboarding, empty states); the strokes look heavy against the reference's hairline icons.                                                                                                                                | all                                                        |
| 9   | **Lists in containers:** history, dictionary, snippets and recents as one bordered white container with 1 px row dividers, 52 pt rows, aligned time column and hover actions.                                                                                                                                                  | Our history rows float on the canvas with no container; the time and text baselines do not align (times sit ~3-4 pt lower); dictionary rows are bare lines.                                                                                                                                                         | Home, Dictionary, Snippets, Scratchpad                     |
| 10  | **Settings as the reference's modal:** nav with icons and uppercase group labels, serif section title, tinted setting-row cards with right-aligned controls, Radix selects instead of native `<select>`.                                                                                                                       | Ours: text-only nav, white cards with native dropdowns, raw error text ("Couldn't list the microphones: null"), the modal nearly fills the window. Native selects and checkboxes are the most "web" controls in the app.                                                                                            | Settings, hotkeys panel, Transforms page                   |
| 11  | **Replace checkboxes with switches** for on/off settings; keep checkboxes only for multi-select lists.                                                                                                                                                                                                                         | Transforms, Scratchpad ("Add to the bar"), Summary, Dictionary auto-learn, and Settings use small square checkboxes for app settings; the reference uses black toggles.                                                                                                                                             | Transforms, Scratchpad, Settings, onboarding Summary       |
| 12  | **Lesson sheets:** big serif heading with one italic word, progress dashes centred at the top, warm sheet background, centred black CTA, generous whitespace.                                                                                                                                                                  | Our lessons are small dialogs (≈ 540 × 400) with a 13 px header bar, a left-aligned 17 px serif title and tiny drawn mocks; the reference sheets fill ~70 % of the window and lead with the heading.                                                                                                                | Transforms lesson, Whisper lesson, quiz, first-use chooser |
| 13  | **Celebration and "time saved" as show moments:** 96 px serif headline on a dark patterned canvas, staged reveal, drawn arrow, rolling numbers, one sticker illustration.                                                                                                                                                      | Ours is an icon tile + bold sans headline + indigo button on flat dark grey; the moment that should feel like a reward looks like an alert.                                                                                                                                                                         | first_dictation, time_saved, typing_test                   |
| 14  | **Empty states with art, not icon tiles:** one illustration, one serif line, one action.                                                                                                                                                                                                                                       | Our empty states (History, Insights, Suggested, Scratchpad) repeat the same grey circle with a 24 px icon and a two-line explanation ("Insights can't be read right now…"); they look like error pages.                                                                                                             | Home, Insights, Dictionary, Snippets, Scratchpad           |
| 15  | **Cut copy to the reference's density:** ≤ 20 words per onboarding screen, ≤ 12 per setting description, one sentence per banner; remove the persistent onboarding footer line and explanatory paragraphs under controls.                                                                                                      | Our Transforms page alone shows ~150 words (Command Mode's description is 45 words); onboarding repeats "Speech setup continues in the background…" on every step. Text walls are a large part of the "slop" impression.                                                                                            | Transforms, Settings, onboarding, lessons                  |

## 2. Per screen

Each entry lists the reference frame(s) (local evidence ids), then the gaps ranked by visual impact. "Ours" sizes are CSS px at 1040 × 720; "theirs" are pt from REFERENCE-SPEC.

### Hub shell (sidebar, title strip, footer)

Reference: `u-073`, `u-075`, `u-080`, `cap-001`.

1. No inset content panel; the page canvas meets the sidebar with a colour step only. Theirs: a 22 pt rounded `#FCFCFA` panel with a hairline, inset 10 pt from the window edge.
2. Title strip: our collapse icon sits alone top-left of the sidebar, theme/bell icons float top-right of the canvas; a version footer bar (`v1.0.0-alpha.1`) spans the bottom. Theirs: one 46 pt strip holding traffic lights, sidebar toggle and the right-side icons; no footer (version lives in Settings).
3. Sidebar items 77 pt pitch at 2× → 38.5 pt (ok) but the active fill is a heavy `#E7E4DC` pill with 12 px radius and a bold label; theirs is a barely-there `#ECECE4`, radius 8, label weight unchanged.
4. No wordmark at the top of the sidebar (ours has none at all); "New!"/"Beta" tags are lilac/sand pills, theirs are a black beta chip and a warm orange new-feature chip.
5. Mixed icon weights in the nav (2 px strokes, the waveform glyph custom).

### Home: greeting, banner, stats rail, voice profile

Reference: `u-073`, `o-0369`.

1. Greeting is a 36 px Newsreader serif with an edit pencil; theirs is 24 px sans semibold, no visible pencil.
2. Banner: flat lilac box, serif headline, three outlined chips, an indigo CTA; theirs: dark photo banner, serif headline in white, a sentence with bold keywords, a cream button.
3. Stats rail: white card with icons and right-aligned serif numbers (label left, number right); theirs: tinted card, **serif number first then a small sans unit** stacked, no icons, voice profile in the same card under a hairline with a thin purple bar.
4. Voice profile bar is indigo 8 px; theirs is purple 4 pt with a caption to its right.
5. History empty state: grey circle + serif headline + two lines; looks like an error.

### History list

Reference: `u-073`.

1. Rows float on the canvas; theirs sit in one bordered white container with dividers.
2. Day headers are 15 px semibold sentence case; theirs are 12 px uppercase tracked micro labels.
3. Time and text baselines are misaligned (time ~4 pt lower than the first line).
4. Hover actions appear in a tinted capsule with a focus ring (keyboard state) that looks like a toolbar; theirs are four bare 16 pt outline icons right-aligned in the row.
5. Search and export icons sit at the section header; theirs put a single search icon at the right of the day label.

### Insights

Reference: `u-075`.

1. Our empty state is the most common view (needs a few dictations) and shows a generic icon tile and an apology; theirs always shows the dashboard frame (zeros/placeholders), so the page never looks broken. Show the frame with zero states.
2. Data palette: our Recharts bars and heatmap use indigo; theirs use dark teal with lighter teal steps, warm grey empty cells.
3. Cards: ours white with 14 px radius; theirs tinted `#F7F4EF`, radius 12, a serif/sans big number + uppercase micro label, generous 24 pt padding.
4. "Save as image" outlined button top-right vs. their circular rotating-text Share badge.
5. Tabs underline style differs (ours 2 px indigo-ish/black, theirs 2 pt black under a 15 px medium label).

### Dictionary and Snippets

Reference: no Dictionary/Snippets frame was archived (R1 viewed them live); patterns from `u-073` (list), `cap-001` (banner, recents), `o-0178` (dialog).

1. Banner is flat lilac with chips (see Home #2).
2. Toolbar: three outlined buttons (Import, Export, Add word) with indigo fill on the last; use one black primary + ghost buttons.
3. List rows are bare lines with pencil/trash always visible; use the list container with hover-revealed actions.
4. The "Learn from my corrections" checkbox + 4-line paragraph above the list is a text wall; make it a switch row with one line.
5. Search field + "Sort" native select + refresh icon look like a web form; use `SearchField` + a ghost dropdown.
6. Tabs "All (5) · Added by you (3) · Suggested (0)": counts in parentheses; use small count chips.

### Style

Reference: `u-080`, `u-099`.

1. No banner; the page starts with a grey sentence and "Used in these apps" + an outlined "Add app" button; theirs: a photo banner with the apps as frosted circular icons and a "+".
2. Preset cards: radio buttons + bold title + description + a quote box; theirs: a two-line title (name + rule), a chat-bubble sample on a lilac tint with an avatar dot, selected = 1.5 pt purple border; no radios.
3. The italic "What you say" sample line between sections adds noise; fold it into the cards.
4. Tabs are fine structurally; the Cleanup tab needs the reference's beta chip treatment if it stays labelled.

### Transforms page

Reference: `cap-024`, `cap-213` (editor).

1. Copy density: ~150 words before the first transform card (banner, enable box, auto-apply paragraph, character warning, Command Mode's 45-word description).
2. "Enable Transforms" is a checkbox inside a bordered box; theirs is an opt-in switch in the page header next to a kbd hint.
3. Header buttons: "Start tutorial" ghost + outlined "Reset to defaults" + indigo "Create new"; theirs: the two secondary actions sit next to a serif section title, and the create button is black.
4. Native `<select>` for the auto-apply transform; theirs a custom select + switch on one row.
5. Transform cards (below the fold for us): use the reference's card (kbd chips top-left, 16 px title, 15 px grey description, 12 radius).

### Scratchpad page

Reference: `cap-001`, `cap-240`.

1. No banner; "Add to the bar" is a checkbox inside a bordered card, "Open shortcut" a second card; theirs: header row with a labelled switch and a shortcut chip, then a photo banner with a drawn note-window peek.
2. Recents: one bordered card per note with tiny text (11-13 px); theirs: tinted cards, 15 px title, two preview lines, date and time at the bottom corners.
3. Empty Recents shows nothing distinctive.

### Scratchpad note window

Reference: `cap-231`.

1. The window is a raw white rectangle with a grey tab strip, a 40 pt left rail of loose icons and an unstyled textarea; theirs: compact title strip with tab chips and a "+", expand and close at the right, rail icons in ink at 18 pt, a floating black "Copy" pill bottom-right.
2. Status line ("Saves automatically", "Copied") in a footer bar; theirs has no footer.
3. The selected text-size and search controls look like form inputs.

### Share with a friend

Reference: `o-0330` (closest: an invite/referral screen with a link field + Copy and an illustration).

1. A form-like card with a heart icon, a URL text field and a "Copied" button, plus a second card; theirs: a short serif headline, three bullet steps with icons, a link chip + Copy, an illustration on the right.

### Settings modal

Reference: `u-033` (only one section archived; General/System described in R1).

1. Nav: text-only items with a bold active pill and a chevron on Advanced; theirs: 18 pt icons, uppercase group labels, version + update icon pinned at the bottom.
2. Page title: 28 px sans; theirs serif 28 with a one-line intro.
3. Rows: white bordered groups under uppercase section labels, native selects with 2 px borders, info icons after labels; theirs: tinted row cards with title + one-line description on the left and a white button/select/switch on the right.
4. Error states leak raw values ("…: null"); design a quiet inline error row.
5. Modal size ≈ 900 × 650 at a 1040 window: fine, but the scrim is a flat 45 % grey; theirs 29 % black.
6. Handy-hosted sections (Advanced) inherit Handy's controls; restyle through tokens and the a11y layer, not by editing Handy files.

### Hotkeys panel

Reference: `o-0178`.

1. Ours: a full Settings-pane list of every action with bordered rows, key fields as outlined boxes with separate Hold/Edit/Delete buttons and native selects ("Double-tap"); theirs: a compact dialog, each action a tinted card with title + one-line hint on the left, a white key field with kbd chips and inline pencil/trash icons, and a "+" square to add.
2. Footer: "Reset to defaults" outlined + indigo "Done"; theirs beige secondary + black primary.

### Dialogs: Add word, Snippet, Import wizard, Flag, Report a problem

Reference: `o-0178` (dialog chrome).

1. Dialog chrome varies by dialog (title sizes, close buttons, footers); one `Dialog` primitive: 17 px semibold title + 15 px line, 28 pt close on a beige square, footer beige secondary left / black primary right.
2. The import wizard's step indicator and file drop zone look like a web upload form; use progress dashes and a dashed drop area with an illustration.
3. Toasts ("Added 'postgres'") float bottom-right as small white chips; theirs (Notetaker frames) are dark cards bottom-left with an action.

### Notifications panel and Help popover

Reference: R1 (described, not archived): notifications panel with an illustrated empty state; help popover with sections.

1. Popovers are plain bordered lists; give them `--elev-1`, 12 radius, micro section labels, 18 pt icons and the illustrated empty state.

### What's new and feature popup

Reference: `cap-007` (theirs is a web page), R1 launch popup.

1. Ours is a plain dialog of text; make it a `Sheet` with a hero image per release, serif title, 3 bullets, one CTA.

### Onboarding (all steps)

Reference: `o-0045` (split), `o-0103`/`o-0113` (illustration questions), `o-0130` (permissions), `o-0160` (mic), `o-0172`/`o-0178` (shortcut), `o-0192`/`o-0214` (practice), `o-0262` (celebration), `o-0268` (hours), `o-0300` (typing test), `o-0330` (closing screen).
Shared:

1. Stepper: 13 px words with chevrons and check marks, no progress line; theirs uppercase tracked labels with a 2 pt purple progress line under the header.
2. A persistent footer line on every step and a "‹ Back" in the footer; theirs: a back link top-left of the content, no footer.
3. Primary buttons: indigo 36 pt, sized to label, centred mid-screen; theirs: black, 360 × 40 pt, anchored low.
4. Every step leads with an icon in a lilac tile; theirs never uses icon tiles; they use illustrations, drawn macOS screens or live demos.
5. Title: Handy's `--font-display` (`ui-rounded`, so SF Pro Rounded in the app's WebKit, plain SF Pro in Chrome) at ~34 px bold with tight tracking; theirs 32 px semibold geometric grotesk (and serif on Learn screens).
6. A visible focus ring sits on the Welcome title in our render (programmatic focus on mount showing `:focus-visible` in Chromium); check in WebKit and keep the ring for keyboard users only.

Per step (biggest gap first):

- **Welcome:** no picture at all; theirs pairs choices with a live demo of the bar typing into an email (our scenes engine can do this).
- **Move to Applications:** no counterpart; apply the centred layout.
- **Languages:** a dense 3-column language grid in a bordered box; use chips on the left and an illustration on the right (`o-0103` pattern).
- **Permissions:** our drawn System Settings art is good; put it in the right pane at the reference's scale (~520 pt) and make the rows tall outlined cards with a tick at the right.
- **Mic test:** a tiny inline meter next to a "Stop test" button, Yes/No as small outlined buttons; theirs: a white test card with 13 large rounded purple bars (14 × 46 pt) on a tinted well, and a secondary change-mic button + black confirm.
- **Shortcut test:** a 64 pt key tile; theirs a key graphic that fills purple when pressed, Edit shortcut / No / Yes in the card footer.
- **Practice:** a plain bordered composer; theirs a styled chat/email mock with a header band, avatar and a send button, centred with a soft shadow.
- **First dictation:** see top-15 #13.
- **Time saved:** "about 0 hours a week" in 24 px sans with a clock tile and two buttons; theirs a 44-48 px mixed roman/italic serif number that rolls, a slider, on dark.
- **Typing test:** a bordered paragraph and a textarea; theirs a white card on dark with a serif question, mono text with live colouring, big tabular counters (wpm, seconds).
- **Summary:** a table of settings ("Not set", "Not measured"); end on a confident closing screen instead.

### First-use chooser

Reference: `o-0340`.

1. Ours: a 640 pt dialog with a left list of small rows and a lilac scene panel; theirs: a near-window-size sheet with a 32 px serif two-line heading, centred outlined chips with icons, and a large demo on a colourful gradient backdrop with captions.
2. Buttons "Maybe later" (link) + indigo "Try it now"; theirs one black button.

### Spotlight

Reference: `o-0364`, `cap-200`.

1. Close in concept. Our hint card is a small white card with three text sizes; theirs is a black pill label under a circular cut-out. Match the cut-out size (~220 pt) and use the bar label style for the hint.

### Setup checklist (sidebar card)

Reference: `u-073`.

1. Ours: a bordered card with "Get started · 0 of 6 done", a highlighted row with an indigo button inside, small icons per item; theirs: a white card, 13 px semibold title, a 4 pt black progress bar, hollow 18 pt circles that tick with a filled check, no buttons inside.

### Transforms lesson

Reference: `cap-183` (video), `cap-190` (examples), `cap-196` (practice), `cap-200` (spotlight), `cap-206` (done), `cap-213` (Polish editor).

1. Sheet size and hierarchy (top-15 #12): our heading is 17 px serif left-aligned in a 540 pt dialog with a "HOW IT WORKS" eyebrow; theirs a 30 px centred serif with an italic purple first word.
2. Our drawn mocks are small and low-contrast inside a lilac well; theirs are full-width composers with a formatting toolbar.
3. Kbd hints inline in body text (⌥O, ⌘Z) are plain characters; theirs are kbd chips with a black inline pill.
4. "Pause animation" link under each scene looks like debug UI; use a small icon button inside the scene.

### Whisper lesson and microphone quiz

Reference: `u-003` (photo), `u-006` (practice), `u-016` (question), `u-021` (mic grid), `u-026` (table).

1. Questions: ours are 2-column rows with small icons and radio circles; theirs one large serif question, 3 stacked 344 pt option cards (title + one line), or a 4 × 2 grid of image tiles with labels.
2. Results table: ours is a 5-column text table with dot ratings and a "Best match" pill in the last column; theirs illustrates each mic in the column header, uses star ratings and price signs, and marks the best column with a green header band.
3. No imagery; theirs leads the lesson with a photo of a person speaking softly (we need our own, SYSTEM-PLAN §6).

### The bar (web view states and native rest)

Reference: `02-bar-click-*`, `03-bar-hover-three-modes`, bars in `o-0200`, `u-065`, `u-067`.

1. Size (top-15 #7) and colour: ours `rgba(24,24,27,.92)` with a 55 % white edge in light mode (a visible halo on light backgrounds); theirs opaque `#000` with no edge.
2. Finish ✓ is an indigo disc; theirs a white disc with a black check; cancel ✕ is `#30302F`.
3. Typography: our bar text is 12.5-13 px (measured ink 11.5 pt for cap + descender) in 34 pt capsules; theirs is 15-16 px medium in 29-30 pt capsules and 34 pt labels. Use one size (15 px medium) everywhere.
4. Mic name inside a wide held capsule ("Mic: MacBook Pro Microphone"); theirs shows the mic in a separate label pill above the capsule for ~2 s.
5. Diff card: close in layout; theirs uses a teal chip for insertions and a grey struck chip for deletions, 15 px text, outline action icons; ours red strike + green underline text and pill buttons.
6. Messages (Canceled · Undo, Done · View changes, errors): ours are 360 pt wide bars with a white pill button and ✕; keep, but restyle to the black capsule, 15 px, and the reference's split capsule for progress (label + spinner segment).
7. Waveform: ours 9 bars always; theirs ~11 thin bars and a dotted idle line before audio.
8. No motion between states (ours cuts between web-view shapes); theirs morphs (SYSTEM-PLAN §5 rule 6).

## 3. What could not be compared

- Dictionary, Snippets, Notifications panel, Help popover, Settings › General/System/Data and Privacy, the bar's result card ("select a text box first") and Wispr Flow's dark Hub: no frames were archived (R1 kept personal pages out). Patterns were taken from neighbouring frames; the owner could archive non-personal shots of these if exact references are wanted.
- Motion under ~0.8 s (all transitions, hover, press): the recordings run at 0.5-1.2 fps (REFERENCE-SPEC §0, §8).
- Our native resting bar (`fork/pill/native.rs`) is drawn by AppKit and is not rendered by Playwright; its size comes from `geometry.gen.json`.
