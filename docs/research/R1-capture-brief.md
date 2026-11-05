# R1 capture brief: Wispr Flow, observed as a consumer

Read this first, then do the capture. Output goes in this folder.

## Why this exists

We are building a free, fully local, no-account, no-cloud macOS dictation app (a fork of Handy, in this repo) that should feel as polished as Wispr Flow. To design it we need an accurate, evidence-backed description of how Wispr Flow looks, behaves and flows for a normal user. You are producing that description by using the installed app, not by taking it apart.

## Rules

- **Observe only.** Use the app like a consumer. Do not decompile, unpack, or read the app bundle's code, scripts, prompts or assets, and do not copy any of its text, images or icons into the repo. Screenshots go only in `evidence/` as reference.
- **Do not change anything that matters.** No changes to account, plan, billing, sharing or team settings. No sign-out, data reset or reinstall (the user handles that separately for onboarding). Toggling a normal preference to see its effect is fine; put it back afterwards and note it.
- **Do not click links** in the app with computer-use tools. Note the destination instead.
- **Use only harmless test text** when dictating (no real names, passwords or private content). Test in a scratch document, e.g. a new TextEdit or Notes note.
- **Tag every claim** `observed` (you saw it, evidence file named), `inferred` (reasoned from what you saw) or `unknown` (could not determine). Never present a guess as observed.
- Say what you could not do (blocked permission, missing screen, feature behind a paywall) instead of skipping it silently.

## Setup

- App: `~/Applications/Wispr Flow.app` (or `/Applications`). The user is already signed in.
- Use background computer-use tools where possible. Request access to Wispr Flow, TextEdit/Notes and System Settings only if needed.
- Save screenshots to `docs/research/evidence/wispr-flow/` as `NN-short-description.png` (zero-padded, in the order taken). Zoom or crop for small details like the pill.

## What to capture (in this order)

1. **Pill and overlay.** Every state: resting, hover, ready, recording while held, recording locked/hands-free, processing, error, anything after paste. Exact position, size, colors, animation, what the waveform reacts to, what the buttons do. Capture while really speaking (the user may speak, or use a text-to-speech command, if you can start it).
2. **Shortcut grammar.** Default shortcut(s). Behavior of: press and hold, quick tap, double tap, hold then lock, cancel, stop. Timing thresholds as best you can measure. What the user sees and hears at each step.
3. **Paste and insertion.** Behavior into TextEdit, Notes, a browser text field, a terminal, a field with nothing focused, and a password field. What happens to the clipboard. What happens on failure and how the user is told.
4. **Main window / hub.** Every sidebar page and its contents: Home, history, dictionary, snippets, style/tone, notes, settings, help. Empty states and populated states.
5. **Settings.** Every settings page and each control with its default. Note which controls look essential for a first-time user and which are advanced.
6. **Capabilities.** For each feature (dictionary, snippets, styles/tones, command or edit-by-voice mode, per-app behavior, language handling, cleanup/formatting), try it with harmless input and record the exact behavior and a before/after example.
7. **Menu bar and system integration.** Menu bar icon and menu, launch at login, permissions requested, notifications, sounds.
8. **Microcopy.** Record the main labels and empty states so we understand tone and information design (reference only; do not reuse wording).

## Deliverable

Write `docs/research/R1-wispr-flow-teardown.md` with:
- A one-page summary: what makes the experience feel premium (10 bullets max).
- Sections matching the numbered list above, each with tagged claims and links to evidence files.
- A feature inventory table: feature, where it lives, default, observed behavior, evidence.
- A list of open questions the user should answer or verify (e.g. things behind a paywall).

Keep it scannable. Do not edit any other file.
