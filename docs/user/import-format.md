# Importing words into the Dictionary

Wispr Bro's Dictionary (sidebar › Dictionary › Import) reads a spreadsheet
saved as **CSV**. The same rules are shown in the app under "Accepted
format"; Export writes this format too, so an exported file imports back
unchanged.

## The format

One entry per row, two columns:

| Column           | What goes in it                                                               |
| ---------------- | ----------------------------------------------------------------------------- |
| 1: word / phrase | A word or name to spell right, or what you say for a replacement              |
| 2: replacement   | Optional. When filled, the text written instead of column 1, exactly as typed |

```csv
phrase,replacement
Siobhán,
Kubernetes,
gonna,going to
"say, ""hi""",Hi
```

- **Header row:** optional. If the first row holds column names
  (`phrase`, `word`, `term`, `from`, `replacement`, `to`, `correct`, …) the
  wizard skips it and picks the columns by name; otherwise it uses the
  first two columns. Step 2 of the wizard lets you change both choices and
  shows a preview.
- **Other columns** are ignored. Commas, semicolons, tabs and `|` all work
  as separators. A cell that contains the separator, a quote or a line
  break is quoted (`"…"`, a quote inside doubled), as spreadsheet apps do.
- **Limits:** a word (or "what you say") up to 60 characters, a
  replacement up to 200, no line breaks or tabs inside a cell; files up to
  3 MB; the dictionary holds up to 5,000 entries.
- **Text encoding:** save as **CSV UTF-8** so accents and other scripts
  (Siobhán, Zoë, 東京) come through. UTF-16 files with a byte-order mark
  and older Windows "CSV" exports (Windows-1252) are read too.
- **Spreadsheet formulas:** Export writes a `'` before a cell that starts
  with `=`, `+`, `-` or `@`, so a spreadsheet app shows it as text instead
  of running it. Import removes that `'` again.

## What the wizard checks

Step 3 shows, before anything changes:

- **New** entries that will be added;
- **Already there:** the same word with the same spelling, or the same
  replacement, is in your dictionary (or earlier in the same file). These
  are skipped;
- **Conflicts:** the same word with a different spelling (`iphone` vs
  `iPhone`), or a replacement for the same words with different text. Your
  version is kept unless you tick "use the file's" for that row;
- **Can't be imported:** empty rows, too long, a replacement equal to the
  word, or a line break inside a cell, each with its row number.

Matching ignores letter case: `Gonna` and `gonna` are the same entry.

## How entries are used

- **Words** help the speech model hear and spell them (they become its
  custom vocabulary).
- **Replacements** change the written text: whole words only, in any case,
  written exactly as you typed them; web addresses and e-mail addresses are
  never changed.

Both apply from your next dictation.
