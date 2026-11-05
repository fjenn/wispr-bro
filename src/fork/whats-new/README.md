# What's New notes

One Markdown file per version, named exactly like the version
(`1.0.0-alpha.2.md`, `1.0.0.md`). After an update, the Hub shows the newest
note that is newer than the last one the user saw and not newer than the
running version, once, and never while a dictation is in progress (S-28).
A fresh install records its version as seen, so a new user sees no note
(Handy's default for a new store; the Hub also marks it seen for a new
user whose store predates the setting: the first-use chooser is still due).
The note never opens over the chooser, a lesson, the bar spotlight or any
other dialog; it waits until they close.

Write our own wording (R5 §1): a short heading and two to five bullets of
user-visible changes. Markdown is rendered by Handy's `MarkdownContent`
(no raw HTML; links open in the browser).
