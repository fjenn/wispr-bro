# What's New notes

One Markdown file per version, named exactly like the version
(`1.0.0-alpha.2.md`, `1.0.0.md`). After an update, the Hub shows the newest
note that is newer than the last one the user saw and not newer than the
running version, once, and never while a dictation is in progress (S-28).
A fresh install records its version as seen, so a new user sees no note.

Write our own wording (R5 §1): a short heading and two to five bullets of
user-visible changes. Markdown is rendered by Handy's `MarkdownContent`
(no raw HTML; links open in the browser).
