<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# VoiceOver checklist (S-37)

The scripted by-ear pass for R5 §5.5 and S-37's real-Mac acceptance: a
person with VoiceOver on walks every screen below, in order, and ticks each
line. It covers focus order (what Tab and VoiceOver's next item reach, and
in which order), every control's **spoken name and state**, and the
**announcements** the app posts on its own. Run it on a debug or release
build of the S-37 test identity (`com.wisprbro.s37check`, portable profile;
MAC-TEST-SCRIPT S-37), in English, then again for the starred (★) lines with
**Increase Contrast** and **Reduce Motion** on (System Settings ›
Accessibility › Display).

What the machine already proves, so the person does not re-check it:
`tests/fork/a11y-sweep.spec.ts` (axe with 0 serious/critical findings on every
Hub page, Settings section, Hub dialog and onboarding step, in light, dark and
`prefers-contrast: more`; every Tab stop named with a visible ring; nothing
animating under Reduce Motion), the per-feature specs (their own axe,
keyboard-only and RTL tests), and `scripts/wb-s37/run.sh` (every AX control
in the running app has an AXTitle, AXDescription or title element). This
checklist is what only an ear can judge: that the words make sense, are said
once, in a sensible order, with the right state.

## Before you start

- [ ] VoiceOver on (⌘F5), speech rate as you like it, verbosity default.
      Keyboard navigation on (System Settings › Keyboard › Keyboard
      navigation) so Tab reaches every control.
- [ ] Wispr Bro in English (Settings › General › App language), a fresh
      portable profile for the onboarding part, then a finished one for the
      rest. Note the macOS version and VoiceOver voice in the report.
- [ ] Notation: **Tab** = Tab key; **VO→** = VO-Right (Control-Option-Right);
      "says" = what VoiceOver speaks, words in quotes are the English UI text
      (small wording differences are fine; a missing name, a wrong state,
      silence or a double announcement is a FAIL).

## 1. Onboarding

Fresh profile; the window opens on Welcome.

- [ ] 1.1 Welcome: VoiceOver starts on the window; VO→ reads the heading
      "Talk, and your words appear", the subtitle, "Everything stays on this
      Mac" with its text, then the start button. The stepper says the step
      ("Step 1 of …" or the step names), not a row of unnamed dots.
- [ ] 1.2 Tab order: the language menu (top), then the main button; Return
      on the main button goes on. No Tab stop is silent.
- [ ] 1.3 (If shown) Move to Applications: "Move Wispr Bro to Applications"
      heading, the reason, "Move to Applications" and "Not now" buttons.
      "Moving…" is spoken after pressing.
- [ ] 1.4 Languages: heading "Which languages do you speak?"; the search
      field says "Search languages" (or its placeholder), and is announced as
      a combo box; each language is a chip that says its name and
      "selected"/"not selected" (or a check box with its state); the count
      line updates and is read after a change ("Dictation works in 2
      languages…"). Back and Continue are named.
- [ ] 1.5 Permissions: heading "Two permissions"; each row says its name
      (Microphone, Accessibility), why, and its state ("Allowed" or "Waiting
      for macOS…"); after granting in System Settings, returning to the window
      reads the new state without a reload.
- [ ] 1.6 Microphone test ("Test your microphone"): the microphone menu says
      the selected mic; the level meter is not read as noise (it is hidden or
      read once as a level); the result ("we heard you" / the problem) is
      spoken when it appears.
- [ ] 1.7 Shortcut test ("Test your shortcut"): the instruction names the key
      ("Hold fn"); pressing fn is acknowledged by speech; each diagnosis (tap,
      Secure Input, Globe key, Control+Option) reads its heading, text and
      buttons; "Use Control and Option" is named.
- [ ] 1.8 Practice ("Practice: a whole email, hands-free"): the drawn composer
      is a text area with a name; after a take its text is readable with VO;
      Continue is named and enabled.
- [ ] 1.9 First dictation: "That's it. You dictated your first text." is read
      when the screen appears. ★ With Reduce Motion: no confetti.
- [ ] 1.10 Time saved and typing test: the headline with the hours; the
      typing field is named; the 30-second timer is not read every second;
      the result is read when the test ends; the hours slider says its value
      and changes with VO-Left/Right.
- [ ] 1.11 Summary ("You're all set"): each row (Languages, Shortcut,
      Microphone, Typing speed) reads label and value; "Open at login" is a
      switch/check box with its state; the last button finishes.

## 2. The Hub

Finished profile, Hub open on Dictation.

- [ ] 2.1 Sidebar: VO lands in a navigation landmark ("Main"); each page is a
      button with its name, the current one says "current page" (or
      "selected"); collapsed sidebar: icons still say the page names; the
      collapse button says its action and state.
- [ ] 2.2 Header: the notifications bell says "Notifications" (with "N
      unread" when there are some); Return opens the panel; focus moves into
      it; Esc closes it and focus returns to the bell.
- [ ] 2.3 Dictation (home): the greeting is a heading; the edit-name button
      is named; the stats rail reads each number with its label ("1,284
      words", "streak 4 days"), not bare numbers; the banners' buttons and ✕
      ("Dismiss") are named; the history list: day headings, each row's time
      and text, its row-menu button ("More actions" or similar), Play,
      Copy, "Wrong word?"; search field named.
- [ ] 2.4 Insights: Usage/Voice tabs are a tab list with selected state;
      charts have a text name and summary (not hundreds of bars read one by
      one); "Save as image" named; the heatmap is one labelled image or a
      summary.
- [ ] 2.5 Dictionary: tabs (All, Words, Replacements, Suggested) with
      selected state; "Add word" and "Import" named; each entry reads the
      word (and "→ replacement") and its actions; the empty state is read.
      Add word dialog: title read on open, fields labelled, Esc closes,
      focus returns to "Add word".
- [ ] 2.6 Snippets: "Add snippet"/"New snippet", each snippet's trigger and
      expansion, the dialog's fields and the character counter (read once, not
      on every keystroke).
- [ ] 2.7 Style: the four group tabs with selected state; each app chip says
      the app name; "+" says "Add an app"; the before/after cards are read as
      text; "Press Enter after dictating" is a switch with its state; the app
      picker dialog: search field named, each app a named option.
- [ ] 2.8 Transforms: the opt-in switch with its state; each card's name and
      shortcut; the editor's fields (name, prompt, rules as check boxes with
      states, examples) labelled; Autosave status read when it saves.
- [ ] 2.9 Scratchpad page: "New note", "Add to the bar" switch with state,
      "Search notes", each recent note (title, time) with Pin/Edit/Delete;
      delete asks for confirmation in a dialog.
- [ ] 2.10 Share with a friend: the link, Copy and Share buttons named;
      "Copied" is spoken after Copy.
- [ ] 2.11 Help: the Help and feedback popover opens from the sidebar; its
      menu items are read with their names ("What's new", "Learn
      Transforms", "Learn to dictate quietly", "Report a problem…", "Send
      feedback"); arrows move between them; Esc closes. Report a problem
      dialog: title, Copy diagnostics status read after copying.
- [ ] 2.12 Setup checklist card: its heading, each item with done/not done,
      the coach mark text read when it appears (and Esc dismisses it).
- [ ] 2.13 ★ With Increase Contrast: every line, chip, selected row and
      focus ring is clearly visible in light and dark; with Reduce Motion
      the drawers and dialogs appear without sliding, spinners stand still.

## 3. Settings

Hub › Settings (or ⌘,).

- [ ] 3.1 The modal: title "Settings" read on open, focus inside it; the
      section list is a navigation with each section a button, the current
      one "current"; "Close settings" named; Esc closes and focus returns.
- [ ] 3.2 General: the microphone picker (each mic and "recommended"),
      languages (as 1.4), app language and theme menus say their value,
      "Change shortcuts" named.
- [ ] 3.3 Shortcuts: each action's name, each binding's keys read as words
      ("Control Option S"), Add/Edit/Delete per binding, the recording state
      ("Press the keys…") spoken, conflicts read, Reset to defaults and Done.
- [ ] 3.4 System: every switch (Open at login, Always show the bar, Show in
      Dock, tray icon, start hidden, notifications) says its name and on/off;
      the sound rows; the output device menu and its reset button ("Reset
      Output Device").
- [ ] 3.5 Vibe coding: the setup, each editor, both switches with states.
- [ ] 3.6 Data and Privacy: the promise text, the four folders with "Show in
      Finder", retention menus with values, Export my data (progress and
      result read), Delete all: the dialog asks to type DELETE, the field is
      labelled, the destructive button says what it does.
- [ ] 3.7 Advanced (collapsed in the list, expands): speech models, AI
      engine, cleanup level (radio group with the selected level), Handy's
      output/transcription rows (each switch and menu named, the number
      fields named after their row), history limit field named.
- [ ] 3.8 About: version, licence, links (read as links), the update panel
      state ("Up to date" / "Restart now").

## 4. The Scratchpad window

From the bar's Scratchpad button or the Scratchpad page.

- [ ] 4.1 The window title is the note's name; the tabs are a tab list
      (selected state, close buttons named); the note is a text area named
      after the note.
- [ ] 4.2 Polish, text size, Copy and the rail's buttons are named; "Copied"
      is spoken; the shortcut dialog (if opened) reads its title and fields.
- [ ] 4.3 Dictating into the note: the bar's announcements (section 6) are
      heard and the text lands in the note's text area.

## 5. Lessons

Help › Learn Transforms, then Help › Learn to dictate quietly.

- [ ] 5.1 The lesson dialog: its title read on open, the step list ("Try it
      out", "How it works") as tabs or buttons with the current one marked,
      Skip and Back named, Esc closes.
- [ ] 5.2 Transforms practice: the drawn chat's field named; after ⌥1, "Text
      rewritten" is spoken; ⌥O opens the changes and they are read as
      removed/added text; ★ the scene pause button is named, and with Reduce
      Motion the scenes are still with no pause button.
- [ ] 5.3 Whisper practice: the composer named; under 70 % the tips are read;
      "Open Sound settings" named; the microphone quiz: each question is a
      radio group with its options, the result table has headers ("Best
      match" read with its row), Copy results says "Copied".

## 6. The bar and one dictation

The resting bar at the bottom of the screen; a text field in TextEdit.

- [ ] 6.1 With VO, find the bar (VO-Shift-M is not needed: it is a button in
      the app's elements): it says "Dictation bar, button" with the hint
      "Hold fn to dictate, or click to start"; Dictate and (if added)
      Scratchpad buttons say their names and help.
- [ ] 6.2 In TextEdit, hold fn and say a sentence: "Listening" when the take
      starts; on release "Transcribing", then "Cleaning up your text" if the
      cleanup runs; after the text lands, "Text inserted". Nothing is spoken
      twice; your own words are not read back.
- [ ] 6.3 Esc during a take: "Dictation cancelled", then "Undo is on the
      dictation bar for 4 seconds."
- [ ] 6.4 A take with no text field (click the desktop, dictate): the card's
      reason "Click into a text field first" **and the dictated text** are
      spoken (high priority); the card's Copy and ✕ ("Dismiss") are reachable
      with VO and say their names; "Copied" after Copy. ⌃⌘C copies it too.
- [ ] 6.5 A take into a password field (Safari login form): "Not typed into a
      password field" is spoken and **the text is not**.
- [ ] 6.6 Long take: "No speech for a while. Dictation stops soon." after
      silence; with the microphone unplugged mid-take (if possible) "The
      microphone disconnected. What you said so far is kept."
- [ ] 6.7 First run only: during the speech download a take says "Setting up
      dictation. Your words are saved."; with the spotlight (first-use "Try it
      now"), the hint and "Press Escape to close" are spoken.
- [ ] 6.8 ★ With Increase Contrast and Reduce Motion: the bar and its
      waveform are visible; the waveform does not animate; the take still
      works and the announcements are the same.

## Report

Record PASS/FAIL per line in the Mac wave report
(`docs/fork/test-runs/…`), with the exact words heard for any FAIL. A line
that cannot be reached (no Slack, no password form) is "not run", not PASS.
