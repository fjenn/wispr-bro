# R1: Wispr Flow teardown

Observed as a consumer on macOS (Apple Silicon), Wispr Flow v1.6.1264, signed-in account. Method: computer-use screenshots and clicks (view-only; no settings, account or billing changes), plus background captures of the pill area. Nothing was decompiled.

**Tags:** `observed` = seen directly this session. `inferred` = reasoned from what was seen. `unknown` = not determined.
**Evidence:** only pill-area frames are archived (`evidence/wispr-flow/02-bar-click-*.png`, contact sheet `02-bar-click-contact-sheet.png`). Hub and settings pages were viewed live; their screenshots are **not archived**, because several pages show the account owner's real dictations, email and snippets. Non-personal pages (Style, Settings, Transforms, Help/Shortcuts) can be archived in a follow-up.
**Not tested on purpose:** whether dictation works or how accurate it is. This document maps experience and capabilities only.

## 1. Experience summary (what a user sees and does)

1. Hold a key (fn) and speak; release; text appears in the focused field. A pill at the bottom of the screen shows state. `observed` hotkey, pill; insertion into a focused field `unknown` (not tested).
2. Everything else lives in a "Hub" window: history, dictionary, snippets, styles, transforms, scratchpad, meeting notes (Notetaker), insights.
3. Personalization is example-led: style cards with before/after text, banners with example chips.
4. Failure is soft: text is kept and can be copied with one click.
5. The product is also an account, a plan and a cloud service (Notetaker, sync, teams, referral, billing). Those parts are out of scope for a local, no-account product.

## 2. Pill (the "Flow Bar")

- **Resting:** a thin horizontal bar centered at the bottom edge of the screen. `observed` (contact sheet frames 1-2)
- **Active (started by click):** the bar expands into a black capsule with **✕ (cancel) left, live waveform of thin vertical bars center, ✓ (finish) right**. `observed`
- **Mic label:** a black rounded label "Using Built-in mic (recommended)" appears above the capsule for roughly the first second or two, then disappears. `observed` (frames 3-6). `inferred`: it confirms which microphone is active.
- **Waveform:** bars change height continuously with audio. `observed`
- **System indicator:** the macOS orange microphone indicator shows in the menu bar while recording. `observed` (standard macOS).
- **Result/fallback card:** when finishing with no text field focused, a black floating card appears above the bar: Flow glyph, hint **"Select a textbox first, then dictate"**, the transcript, a **Copy** button and a ✕. `observed`
- **Focus note:** after the synthetic click on the bar, macOS focus moved to Wispr Flow (the target app lost focus). Whether a real click or the keyboard path does the same is `unknown`.
- **Unknown:** held-key and hands-free states from the keyboard (may differ from click-started), processing/spinner and error states, resting-bar hover state, pixel sizes, colors, animation timings.

## 3. Hotkeys (Help > Shortcuts, "Change hotkeys" panel) `observed`

| Action | Default | Notes |
|---|---|---|
| Push to talk | `fn` | "Hold to say something short" |
| Hands-free mode | double-tap `fn`, or `fn` + `Space` | "Dictate hands-free by pressing this hotkey to start and stop". Two bindings are listed; more can be added with "+". |
| Press Enter | not set | "Trigger enter with a mouse shortcut (or another key) to send messages faster" |
| Command Mode | `fn` + `Ctrl` | "Select text and ask Flow or Perplexity" |
| Paste last transcript | `Ctrl` `Cmd` `V` | |
| Copy last transcript | `Ctrl` `Cmd` `C` | |
| Open Scratchpad | not set | |
| Join meeting / start Notetaker | `Opt` `M` | |
| Transform | configured in the Transform tab | Polish `Opt` `1`, Prompt Engineer `Opt` `2` |
| Transform: view changes | `Opt` `O` | shows a diff after a Transform |
| Cancel | `Esc` | "Dismiss dictation and notifications" |

The panel has **Reset to default** and **Done**. Per-item edit (pencil), delete (trash) and add (+) controls. Timing thresholds for tap vs hold: `unknown`.

## 4. Main window ("Hub")

Light, warm off-white UI, rounded cards, serif display headings on promo banners, sans UI text. Left sidebar: **Dictation, Notetaker, Insights, Dictionary, Snippets, Style, Transforms, Scratchpad**; a "Set up Notetaker" checklist card (2-min tour, connect calendar, import from Granola/Otter, set up MCP); then **Invite your team, Get a free month, Settings, Help**. Top right: dark-mode toggle, notifications bell, profile menu. A sidebar-collapse button sits next to the window controls. `observed`

- **Dictation (home):** "Welcome back, <name>", a dismissible promo banner, then a right-rail stat card (total words, WPM, day streak) and a **"Your Voice Profile"** progress bar (unlocks after a word count). Below: **history grouped by date**, one row per dictation with time and full transcript, a search icon. `observed`
  - **Row hover actions:** play (audio), copy, flag, and a ⋮ menu: **Undo AI edit, Retry transcript, Delete transcript, Extract audio**. `observed` So raw audio is kept per dictation and cleanup is reversible.
- **Notetaker:** meeting-notes product. "Start Notetaker" button, Today (calendar-driven; empty state "No meetings today"), tabs Past notes / Shared with me / Folders. `observed`
- **Insights:** tabs **Your usage / Your voice**. Usage: words per minute with percentile, "fixes made by Flow" (words corrected, dictionary fixes), total words (desktop vs mobile), usage by category (documents, other tasks, AI prompts, emails, work/personal messages), streak calendar, Share. Voice: a locked "Voice Profile" (unlocks after ~1.7K words) with blurred cards: Catchphrase, Peak time and place, Most used word, Most corrected word. `observed`
- **Dictionary:** tabs All / Personal / Shared with team; Import, Add new; search/sort/refresh. Dismissible banner "Flow spells the way you do" with example chips. Entries are single words or **"from → to" replacements**. `observed`
- **Snippets:** same layout. Entries are **spoken trigger → expansion** (links, email, long prompts). Banner shows three examples. `observed`
- **Style:** tabs **Personal messages / Work messages / Email / Other / Auto cleanup (Beta)**. Each style tab applies to a set of apps (shown as app icons, "+" to add): personal messengers (WhatsApp, Telegram, Discord, Instagram…), workplace messengers (Slack, Teams, LinkedIn…), email apps (Gmail, Outlook, Mail…), all other apps. Presets are shown as before/after cards: **Formal** (caps + punctuation), **Casual** (caps, less punctuation), **Very casual** (no caps, less punctuation) for personal; **Formal / Casual / Excited!** (more exclamations) for work, email and other. A note: style formatting applies only in English. `observed`
  - **Auto cleanup (Beta):** three levels applied to every dictation in every app: **None** (exact transcript incl. mistakes), **Light** (filler words and grammar), **Medium** (clarity and conciseness). The page says the original dictation is never lost ("Undo AI edit" in the row menu). `observed`
- **Transforms (Beta):** opt-in toggle; hotkey hint "Opt o to view changes". Banner "Transform works anywhere you write" (Try it out / How it works). **Auto Apply After Dictation** toggle with a chosen transform (Polish). **My Transforms:** Polish (Opt 1, "Improve clarity and conciseness"), Prompt Engineer (Opt 2, "Constructs optimal prompts"), Create your own; Reset to defaults; Create New. `observed`
- **Scratchpad (Beta):** quick-notes area. "Add to Flow Bar" toggle, a shortcut button, "Start new note", and a Recents list of notes with a title, preview and date. Settings > System has "Scratchpad open behavior" (Resume last note, Customize). `observed`
- **Notifications panel:** bell opens a panel with filter and "…" controls; empty state illustration "You're all caught up!". `observed`
- **Profile menu:** name and email, "Get a free month of Flow Pro / Refer a friend", "Manage account". `observed`
- **Dark mode:** a dark theme exists (announced by a launch popup "Dark mode is finally here"); the Theme setting offers Light (and presumably Dark/System). `observed` popup, `unknown` full theme list.

## 5. Settings (modal, two columns) `observed`

- **General:** Shortcuts (shows "Hold fn and speak", Change), Microphone (Built-in mic (recommended), Change), My languages (Change), App Language (dropdown), Theme (dropdown, Light).
- **System:** *App settings:* Launch app at login, Show Flow Bar at all times, Show app in dock (all on by default here). *Sound:* Dictation and notification sounds (on), Mute all audio while dictating (off). *Notifications:* Suggestions, Announcements, Milestones (all on). *Scratchpad:* open behavior.
- **Notetaker:** meeting detection (notify before scheduled meetings: 15 sec; show next meeting in menu bar; automatically detect any call; start Notetaker automatically; max recording length 2 hours; stop when a call ends), "While Notetaker is active: don't show Notepad and Flow Bar in screen capture", Notepad view options, Start tutorial.
- **Vibe coding:** variable recognition for VS Code / Cursor / Windsurf (Set up), file tagging in chat (toggle).
- **Connectors:** Google Calendar, Outlook Calendar, Slack (each "Connect"), plus "Give your AI access to your meeting transcripts and notes → Go to MCP".
- **MCP:** "Add to Claude / ChatGPT / Gemini / Cursor" and "All other apps". States the MCP does not have access to dictations.
- **Account:** first/last name, email, profile picture, Save, Sign out, Delete account.
- **Team:** invite teammates by email; pitches a 2-week Pro trial; Team FAQ.
- **Plans and Billing:** Monthly/Annual toggle. **Free** ($0): dictation in any language in any app on Mac/Windows/iOS/Android, learns names and jargon, meeting notetaker for Mac, speaker identification, ask questions about meetings, MCP, calendar/Slack, HIPAA-ready, opt out of model training. **Pro** ($12 per user/month annual): unlimited dictations, more meetings kept longer, early access, priority support, shared dictionary and snippets, team features. **Growth & Enterprise** (from $18): SSO/SCIM, audit logs/MDM, org-wide HIPAA with BAA, admin controls, usage reporting, dedicated support, invoicing, custom legal terms. (Prices as shown; may change.)
- **Data and Privacy:** toggles for "Improve the model for everyone" (off here), "Dictation cloud storage" (off here), "Context awareness" (on: uses limited relevant text from the app you dictate in to spell names correctly), "Show your public information" (on). Local data storage dropdown ("Store data locally"), Sync notes, Download your data (request a copy by email), Delete your data.
- Footer: "Flow v1.6.1264" and an **Update** button.

## 6. Menu bar and app menus `observed`

- App menus: File, Edit, Dictation, Notetaker, My Voice, View, Help, Window.
- **Dictation menu:** "Words dictated: <count>", **Copy last transcript** (with preview), **Microphone ▸**, **Languages ▸**.
- **My Voice menu:** "Dictionary words: N", "Snippets created: N", **Add a word to dictionary**, **Create a new snippet**.
- **Help menu:** search box, Help center, **Talk to support (⌘/)**, Share feedback, **Shortcuts** (opens the hotkeys panel).
- A menu-bar status icon exists (menu content `unknown`; File and View menus did not open in my attempts).

## 7. Cloud, account and local data (relevant to a local-only product)

- Account sign-in, plans, teams, referral, calendar/Slack connectors, MCP, cross-device sync and Notetaker cloud storage are all cloud features. `observed`
- Settings expose "Store data locally" and opt-in toggles for model-training data and cloud dictation storage, so the product itself separates local and cloud data. `observed`

## 8. What seems to make it feel premium (`inferred`)

1. Almost no decisions at the moment of use: one key, language and mic preselected.
2. The pill confirms which microphone is active as soon as you start.
3. Failure is soft: text is kept and copyable; per-dictation history lets you retry, undo AI edits, extract audio.
4. Corrections feed back visibly (dictionary fixes counted in Insights).
5. Personalization is visual and example-led (before/after style cards, banners with sample chips).
6. A simple three-level cleanup control (None/Light/Medium) instead of prompts.
7. Empty states and banners teach each feature with concrete examples.
8. Everything is reachable from a calm sidebar; settings are grouped by task, not by technology.

## 9. Candidate scope for a local product (`inferred`, for R4/R5)

Core: pill states, hotkeys (push-to-talk, hands-free, cancel, paste/copy last), history with retry/undo/audio, dictionary with replacements, snippets, cleanup levels, per-app styles, Transforms (polish), scratchpad, launch at login, sounds, mic and language pickers.
Probably out of scope or later: account/teams/billing/referral, cloud sync, Notetaker meeting product, calendar/Slack connectors, MCP, Insights sharing, mobile.

## 10. Second pass: guided flows, Help, Scratchpad, pill modes

Tags: `observed` unless marked `user-reported` (stated by the product owner while using the app).

**Pattern: every feature teaches itself.** Each major feature has (1) a dismissible banner with concrete examples, (2) a "How it works" video or guided modal, and (3) an interactive "Try it out" practice step. Notetaker has a "Try Notetaker, 2 minute guided tour"; Transforms has a video plus a two-step practice; Dictionary and Snippets have example banners. `observed`

- **Sidebar Help popover:** sections **Updates** (What's New), **Essentials** (Shortcuts, Microphone, Languages), **Get in touch** (Help Center, Talk to support, Contact sales). What's New opens a web page in the default browser. `observed`. `user-reported` for our product: call it "Help and Feedback", keep only contact and feedback.
- **Notetaker tour:** "Take 2-min tour" (sidebar checklist) and the "Try Notetaker" card (Today section, Skip / Start, video-thumbnail icon) open a **"Complete setup to start Notetaker"** gate: "These are required settings", first item **Enable System Audio ("Capture what others say") with a Turn on button** and an illustration of the macOS permission prompt; footer "Always ask for consent before transcribing. Read our privacy policy". Later steps were not reached because Turn on triggers a macOS permission. `observed`. The Notetaker page also shows "No meetings found: Add calendar for accurate speaker names and richer summaries", with a Connect calendar button. `observed`
- **Notetaker settings (lower sections)** `user-reported`: meeting detection, Notepad, Transcript, Consent, Sharing, each with its own options; a Start tutorial button. Not independently observed.
- **Transforms guide, step by step** `observed`:
  1. **How it works:** modal with an embedded hosted video "How to use Transforms (Beta)" (about 1 min 22 s, speed control, fullscreen, picture-in-picture). It shows: dictate a message in Gmail, highlight it, press the Transform shortcut, a diff card ("8 Changes", removed words struck through, additions highlighted, "Configure Polish", copy, thumbs up/down, retry), then the Polish settings page.
  2. **Try it out, step 1:** "Transform your text to sound however you want", three cards (Original / Polish / Prompt Engineer) with before/after text, and a ticker of benefits (Make more concise, Reword for clarity, Reorder for readability, Add structure for readability, Maintain your tone, Optimize text for...). Two-segment progress bar, close X, CTA "Try it out".
  3. **Try it out, step 2:** "Highlight text, then press ⌥Opt 1 to Transform", with a sample message composer. After pressing the shortcut: heading becomes "**Transformed!** Press [see what's changed] or ⌥Opt O to see changes. You can always ⌘Z to undo Transform anytime". The screen dims and a spotlight highlights the bottom pill, which shows **"Done. See changes"**. A final CTA "Turn on Transforms".
- **Polish detail modal** (click a transform card): left: name, "⌥Opt 1 to use", description ("rewrites your text to sound clearer, in your voice"), example of transformed text with a "See Updates" button. Right: Autosave On, Reset; **Choose a keyboard shortcut**; **Select rules** toggles (Make more concise, Reword for clarity, Reorder for readability, Add structure for readability, Maintain your tone); **Customize your Polish prompt** (free-text); **Add writing examples** (Upload Example, "help Flow learn your unique style"). `observed`
- **Transform result UX:** the transform replaces the selected text in place; ⌘Z undoes; a diff view opens from the pill or ⌥O. `observed` (replace, hint text); diff window itself `unknown`.
- **Scratchpad (Beta):** page has "Add to Flow Bar" toggle, shortcut button, "Start new note", Recents (hover actions: edit, pin, delete). "Start new note" opens a **small floating note window**: tabs ("Untitled ×", "+"), expand and close buttons, a left rail (sidebar toggle, new note, search, AI sparkle, text size), a text area and a **Copy** button. `observed`
- **Flow Bar with Scratchpad enabled** `user-reported` + `observed`: the resting bar becomes a group of **three round buttons: Dictate (mic), Notetaker (record-dot with a small teal badge), Scratchpad (note icon)**. Hovering shows tooltips: "**Dictate fn**" above the mic, "**Scratchpad**" above the note button. Evidence: `evidence/wispr-flow/03-bar-hover-three-modes.png`.
- **Referral/free month** `user-reported`: not needed since our product is free; a simple "share with a friend" tab could replace it.
- **Data and Privacy for our product** `user-reported`: all local, no data leaves the computer; a short, simple explanation page replaces the cloud toggles.

## 11. Third pass: dialogs, Notetaker tour, other guides

`observed` unless marked.

- **Dictionary "Add new"**: dialog "Add to vocabulary". Toggles: **Correct a misspelling** (reveals a "Misspelling → Correct spelling" pair of fields) and **Share with team**. Text field "Add a new word"; Cancel / Add word (disabled until filled).
- **Dictionary "Import"**: 4-step wizard ("Import Dictionary"), step 1: Share with team toggle and "Select a .csv file to import" (browse or drag and drop, max 3 MB, link to accepted-format info). Later steps not reached (needs a file).
- **Snippets "Add new"**: dialog "Add snippet": Share with team toggle, a **name/trigger** field, a **rich-text body** (bold, italic, bullet list, numbered list, link) with a **4,000-character counter**, Cancel / Add snippet. Snippets also have Import.
- **Style "+"** next to the app icons did not respond to a click (may be decorative or need a different gesture). `unknown`
- **Scratchpad shortcut dialog**: "Open Scratchpad. Press to open the scratchpad window", proposes **⌥Opt S**, edit pencil, Done. (Dismissed without saving.)
- **Invite your team**: sidebar item opens Settings on the Team page (invite by email, three email fields, Add more, Continue, "2-week pro trial", Team FAQ, Contact us).
- **Notetaker settings, all sections**: *Meeting detection:* notify before scheduled meetings, show next meeting in menu bar, auto-detect any call, auto-start, **max recording length (default 2 hours)**, **Stop Notetaker when a call ends** (on). *While Notetaker is active:* "Don't show Notepad and Flow Bar in screen capture" (off). *Notepad:* default view dropdown ("Open Notepad"; side-by-side option exists). *Keyboard shortcut:* "Press Opt + M to join a meeting or start the Notetaker" with Change shortcut. *Transcript:* **Show live transcript** (on). *Consent:* **Calendar notice** dropdown (Off), indicates notetaker use in calendar event descriptions. *Sharing:* **Default for Notetaker links** ("Anyone with the link"; transcripts only visible to invitees), **Auto-share notes** ("share them with [no one] for [all meetings]"). **Import your meeting notes** (Import button). **Start tutorial** button at top right.
- **Notetaker setup gate** ("Complete setup to start Notetaker", reached from the sidebar checklist, the Try Notetaker card and Start tutorial): item **Enable System Audio ("Capture what others say")** with a Turn on button and a picture of the macOS permission screen; footer "Always ask for consent before transcribing". Turn on sends the user to macOS System Settings > Screen & System Audio Recording; after granting, the app restarts and the gate is gone.
- **Notetaker guided tour** (after the permission, "Try Notetaker, 2 minute guided tour", Start): **a video overlay** (presenter, the CEO) plays in a small floating window, while the app **simulates a live meeting** in a **docked right-hand "Meeting with Tanay" window** with a timer. The simulated flow: (1) a notification-style prompt at the bottom to join the meeting ("Join meeting"), (2) **Transcript** tab fills live with speaker bubbles labeled "Them" and "You", with a banner "After the meeting, Wispr will improve the transcript and label speakers", search, translate and copy icons, (3) an **Ask anything** field with a **"What did I miss?"** chip, which answers with a short recap, (4) a **Stop** button, then the **Summary** tab: "1 MIN READ", headline paragraph, "How Notetaker works" bullets, "Next steps" with owner names, plus an "Ask anything" bar and a "1 hour meeting compressed to a 2 min read" footer. Tabs: **My thoughts, Transcript, Summary**. Header: back arrow, "…" menu, side-by-side toggle, **Connect calendar, Share, copy link**; meeting title with date chip and "Add folder". The tour ends by returning to the main window with the meeting under Past notes and the checklist item "Take 2-min tour" ticked; a **Connectors settings** page then opens automatically as the next suggested step. A small floating pill at the bottom shows a waveform and a record dot while Notetaker records. `observed` (contact-sheet frames)
- **"Show me how" banner** (Dictation page, "Working around other people?"): a 3-step guided modal (progress dots, close X): (1) "**Flow works even when you speak softly**" with a looping video of someone whispering into a gooseneck mic with a caption overlay, button "Show me how"; (2) "**Great, here's how you can whisper with your built-in microphone**" with a video, button "Give it a try"; (3) "**Your turn! Try whispering this email**": instruction "Press fn, then get close to your mic and whisper as softly as you can", a **simulated Gmail composer** with a prompt text to read aloud, and a Continue button (disabled until the dictation lands). Not completed (it requires dictating). `observed` steps 1 to 3
- **Menu-bar status icon menu**: "See all upcoming meetings", **Start Notetaker**, **Open Wispr Flow**, **Check for updates…**, **Paste last transcript (⌃⌘V, disabled until one exists)**, **Shortcuts**, **Microphone ▸**, **Languages ▸**, **Help center**, **Talk to support (⌘/)**, **Share feedback**, **Hide next meeting from menu bar**, **Quit Wispr Flow (⌘Q)**.
- **File and View menus** did not open under computer control; likely standard application menus. `unknown`
- **Update flow**: Settings footer shows "Flow v1.6.xxxx" and an **Update** button (a cloud icon appears after updating). The version changed from 1.6.1264 to 1.6.1297 during this session after the restart. `observed`

**Patterns to carry into the product (`inferred`):**
1. Every feature is taught by a short guided modal with before/after examples and an interactive practice step; practice steps use simulated apps (Gmail composer, a fake meeting).
2. Permission requests are explained with a picture of the macOS screen before the user is sent there.
3. Empty states offer one clear action (Connect calendar, Start new note, Add new word) and a dismissible example banner.
4. The product keeps one consistent chrome: sidebar, light warm theme, serif display headings for emphasis, rounded cards.
5. A persistent bottom pill is the single entry point for dictation, Notetaker and Scratchpad.

## 12. Whisper tutorial, continued (user-run, captured by screen frames)

`observed` from background frames while the product owner ran the flow; step order read from the frame sequence, small text cross-checked at full size where noted.

After the "Your turn! Try whispering this email" step (a simulated Gmail composer; the dictated text then shows up in the Dictation history), the guide **continues into a short personalization quiz**, each step a full-modal with a back arrow, a progress-dash bar, serif heading, and a Continue button:
1. **"Whispering is easier with the right microphone"**: a row of four photos of people using different mics; subtitle says the built-in mic works but a more comfortable one may be available.
2. **"Do you work next to other people?"**: three option cards: *No / Mostly private, quiet spaces*; *Sometimes / Mix of private and public spaces*; *Yes / Mostly working around others*. `observed` (full size)
3. **"Do you work at the same desk every day?"**: two option cards (*Yes, mostly the same desk*; *No, I change my work location often*).
4. **"Select all the mics that you own"**: multi-select grid of illustrated cards (Wired Headphones, Bluetooth Earbuds, Headset, Clip-on Lav Mic, Podcast Mic, Headset with Mic, Gooseneck Mic, "Something else"); selected cards get a lilac fill and border. Continue. `observed` (full size)
5. **"Because you work in different locations, we recommend..."**: a **comparison table** of mics (Podcast Mic, Gooseneck Mic, Clip-on Lav Mic, Wired Headphones, Bluetooth Earbuds, Headset...) with rows *Mic quality (stars), Whispering around others, Whispering at home, Price ($ to $$$)*, a green **"Best match"** header on the recommended column, "Want more info? See here", and buttons Done / Email results. `observed` (full size)

Then it returns to the main window where the whispered dictation appears in history ("Hey Jerry, can you believe I'm whispering this email to you right now? Talk soon, <name>").

**Set up MCP** (sidebar checklist) opens Settings > MCP: "Sync Notetaker with your favorite AI apps... The Wispr MCP does not have access to your dictations", with Add to Claude / ChatGPT / Gemini / Cursor and "All other apps". The **Import from Granola or Otter** item opens a "Where would you like to upload from?" dialog (Granola / Otter / ...). `observed`

**Product insight (`inferred`):** the tutorial teaches the feature, then asks about the user's context to recommend hardware and tune the experience, finishing with a concrete recommendation. Onboarding is a conversation with a result, not a settings checklist.

**Privacy note:** the background capture also recorded other windows on the screen (a browser, Terminal, an AI coding tool). These frames stay local in the gitignored `docs/research/evidence/wispr-flow/user-run/` folder and must never be committed or shared.

## 13. Onboarding (user-run first-run, screen frames, about 5.5 minutes)

`observed` from 401 screen frames (kept locally; see privacy note). Coverage starts at the "How can Flow help you?" screen; the very first welcome and account screens were before the recording started, so they are `unknown`.

**Frame of the flow:** a full-window, light, warm layout with a **five-stage stepper in the header: GET STARTED > PERMISSIONS > SET UP > LEARN > PERSONALIZE** (the first label reads SIGN UP on the earliest screens, then GET STARTED), a thin purple progress line under it, a Back link top-left, and a Help chip bottom-left. Left half: heading and controls; right half: a calm illustration, product demo, or macOS screenshot. Primary button is a black pill (disabled/grey until a choice is made). Later "Learn" screens switch to a **dark, dramatic theme**.

**Step by step:**
1. **GET STARTED, "How can Flow help you?"** Multi-select cards: **Dictation** ("Speak instead of typing, and get perfectly formatted text in any app") and **Notetaker** ("Transcribe all your meetings, and get auto-generated notes and summaries"). The right side animates a demo for whichever card is selected (a simulated email composer with the pill underneath; a meeting recording with a notes panel). Continue enables once one is chosen.
2. **"How do you currently record meetings?"** (shown because Notetaker was selected): chips for Otter, Zoom, Granola, Microsoft Teams, Notion, Fathom, Fireflies, Grain, Plaud, Google Meet, "Something else", "I don't record meetings"; illustration of a person at a desk. "This helps us personalize Notetaker for you."
3. **"What's frustrating about meetings?"** Multi-select list (prepping for meetings, taking notes during the call, tracking action items, finding notes afterward, something else); illustration.
4. **PERMISSIONS, "Thanks for trusting us, we value your privacy."** Three rows, each ticking as granted: **Allow Wispr to recognize meetings; Allow Wispr to use your microphone; Allow Wispr to capture meeting audio.** The right side shows a **picture of the macOS System Settings screen and the exact system prompt** to expect ("Wispr Flow would like access to record your system audio", Don't Allow / Allow). Real macOS prompts follow.
5. **"Connect your calendar"**: benefits list (reminders to join and transcribe, relevant meeting context, speaker names in transcripts), buttons **Connect Google Calendar, Connect Outlook, "I use a different calendar"**, and a mock of upcoming meetings on the right. Choosing Google opens the **browser** for the standard Google account chooser, then a web page "You're now logged in! You can close this window", then a system prompt to open the app again. A different calendar reveals "What calendar do you currently use? iCloud / Something else / I don't use a calendar".
6. **SET UP, "Test your microphone":** "We recommend built-in or wired microphones. Bluetooth devices are less reliable." A card "Do you see purple bars while you speak?" with a **live level meter (purple bars)**, buttons **Change microphone** and **Yes**. Change opens a list: **Built-in mic (recommended)**, Auto-detect (system setting), other connected devices.
7. **"Test your dictation shortcut":** "Hold fn while you speak. Flow Dictation turns your voice to text." Card: "Does the button turn purple while pressing it?" with an on-screen **fn key graphic (globe icon) that turns purple when pressed**, buttons **Edit shortcut**, No, Yes. **Edit shortcut** opens the same hotkey panel found in Help (Push to talk fn, Hands-free double-tap fn or fn+Space, Reset to default, Done). A "Reset to default" chip appears bottom-right if changed.
8. **"Write an email hands free"** (Skip link top-right): "Flow Dictation formats for you and fixes mistakes". A simulated **Gmail composer** (To, Subject prefilled) with "Click into the text field here"; the user dictates and the text appears.
9. **LEARN, dark screen: "Nice job! You just spoke."** A celebratory moment after the first successful dictation.
10. **Time-saved pitch:** "With Flow you could save **0 hours a week!**" (counter) leading to **"Can you type faster than you dictate?"**: a **30-second typing test** (language picker, "Wispr works in 100+ languages", a sample paragraph in monospace, live **speaking words/min** and **seconds left**, "Recalculate time savings", Try Again), then the headline updates to "With Flow you could save **25 hours a week!**" with a **"Typing time/day" slider** (default 6 hours) and "Don't believe us? Test your typing speed". Button Continue.
11. **PERSONALIZE, "Give the magic of Flow. Get a month of Pro."** Referral screen: how it works (share your invite link, they sign up and get a free month of Pro, you get a free month when they dictate 2,000 words), your referral link with a Copy button, an illustration, Continue.
12. **Main window opens with a modal "How would you like to use Flow first?"** Choice chips (each swaps the demo on the right to a looping, captioned video of that use case): **Chat with AI, Take a note, Write a message, Draft an email, Write a post or comment, Write a document, Prompt Cursor or Windsurf**; button **Try it now**. The demo videos show the app in use with a large caption of what is being said ("What's on your mind today?", "Hey can you tell me...", "can you fix the bugs", "Our goal is to show...").
13. **"Next step" coaching:** after picking one, the modal closes, the screen dims, and a **spotlight highlights the bottom pill** with a hint such as "**Next step: Open Cursor or any IDE**" / "Open ChatGPT or any AI app" / "Open your email inbox", and a "Try Flow in another app / Get started" banner appears on the Dictation home page. The first real dictations appear in history.
14. **End state:** normal hub with the "Try Flow in another app" banner, stats (total words, wpm, streak), and the Notetaker sidebar item tagged **"New!"**.

**What makes this onboarding work (`inferred`):**
- Every permission is explained with a picture of the macOS prompt before it appears.
- Every setup step is a **tiny test with an immediate yes/no** (see purple bars, see the key turn purple, dictate into a fake email), so the user knows it works before leaving.
- The first dictation happens inside onboarding, in a safe simulated app, then is celebrated ("Nice job!").
- A short personalization quiz shapes later screens, and the **typing-speed test turns a feature into a number** (hours saved).
- It ends by sending the user into a real app with a "next step" hint on the pill, not by dumping them in settings.
- Total time about 5 minutes with 14 screens, most of them single-choice or single-tap.

**For a local, no-account product** (`inferred`, for R4/R5): drop the sign-up, calendar/meeting-tool questions, referral and Pro screens; keep Permissions (with the macOS-prompt pictures), Test microphone, Test shortcut, Hands-free practice in a simulated composer, the "Nice job" moment, an optional typing-speed test, a "How would you like to use Flow first?" chooser with captioned demos, and the pill "next step" coaching. Silent model choice replaces any model picker.

**Privacy note:** these frames also captured other windows on the screen, including a Google account chooser listing several accounts and emails, a browser and a Terminal. They stay local in the gitignored `docs/research/evidence/wispr-flow/onboarding/` folder; never commit or share them, and delete them once the packet is final.

## 14. Notetaker, a real recording (user-run, screen frames, about 3 minutes)

`observed` from 220 frames. A short real session started by the product owner (a quick audio test, very little speech).

- **Start:** starting Notetaker turns on the macOS microphone indicator and opens a **docked panel on the right edge of the screen** (about a third of the screen height-wise full), titled **"New note"** until it is named. The Flow Bar at the bottom switches to a recording state.
- **Panel layout:** header with traffic-light buttons, back arrow, "…" menu, side-by-side toggle, a highlighted **Connect calendar** button, **Share**, and a copy-link button. Below: editable title, a **"Today"** date chip and **"Add folder"**. Tabs: **My thoughts** (default; placeholder "Write your notes here. We'll make a separate summary at the end."), **Transcript** (a small live-waveform icon while recording), **Summary** (a sparkle icon).
- **Footer while recording:** a **Stop** button (green square), an **"Ask anything"** field, a **"What did I miss?"** chip, and the line **"Always get consent when transcribing others. Learn more."** A lilac banner above: **"We'll improve the transcript and speaker names after the meeting [Explore →]"**.
- **Screen-share notice (toast):** a small dark card at the bottom-left: **"Notetaker screen share: your Flow Bar and Notepad show up in screen shares and screenshots." [Update settings]** with a close X. It links to the Notetaker setting "Don't show Notepad and Flow Bar in screen capture". Clicking Update settings opens Settings > Notetaker.
- **Transcript tab while recording:** shows **"Listening…"** and a small tip card ("**Speaker names are added later**: we'll identify each speaker and clean up the transcript after the call", with a Next button and step dots). A timer sits at the top of the list with search/translate/copy icons.
- **On stop:** the note is **auto-titled** (here "Audio Test Check"). The footer Stop becomes a **Resume** button.
- **Short or empty recordings:** a toast appears bottom-left: **"Started by mistake? Only a few words were captured. Keep this meeting or discard it." [Discard] [Keep]**. In this run the panel also showed an inline orange banner **"We hit a snag processing your meeting"** with a retry icon, with My thoughts and Transcript empty.
- **Summary tab while processing:** a pale-green status row **"Transcript ready" with "step 2 of 3"** above **skeleton placeholder lines**, then the tab shows the summary when ready (the tour version shows the finished layout: "1 min read", summary, Next steps). Earlier a similar status read "Turning your 1-minute meeting into a two-minute read".
- **After:** the meeting appears in the Notetaker page under Today / Past notes.
- **Not captured:** typing into **My thoughts** (only the placeholder was visible), a populated live Transcript from a long conversation, "What did I miss?" output, folder and share dialogs. `unknown`.

**Product insight (`inferred`):** Notetaker is a second major product (meetings) built on the same engine. Its core loop is start, glance at live words, stop, get a structured summary, ask follow-up questions. Everything depends on a local model for summarization and "ask anything", which is a much larger scope than dictation; propose treating it as a later milestone.

## Open items

1. **Onboarding:** not observed (account already set up). Needs a sign-out/data-reset and a screen recording by the user.
2. **Real keyboard behavior** (tap vs hold thresholds, hold-then-lock transitions, held-state pill look): deliberately not tested.
3. **Not opened:** menu-bar status icon menu, File/View menus, "Show me how" banner flow, Command Mode behavior, Transform diff window, Scratchpad window, idle-bar hover state.
4. **Side effects of this pass:** one test dictation was added to the account's history and counters; a scratch TextEdit document remains open.
