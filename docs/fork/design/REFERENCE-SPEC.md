# Wispr Flow design language: measured reference

Design audit, 2026-11-03. This file describes **how Wispr Flow looks and moves**, measured from the owner's local screen recordings. It is the target for the Wispr Bro visual rebuild ([SYSTEM-PLAN.md](SYSTEM-PLAN.md)); the per-screen gaps are in [GAP-AUDIT.md](GAP-AUDIT.md).

The rule from the owner stays: follow the layout, spacing, type scale, palette, density, motion and interaction patterns closely, but **never copy Wispr Flow's files** (logo, icons, illustrations, photos, sounds) or its UI wording. Everything below describes; nothing here is an asset. No Wispr Flow image is in the repo.

## 0. Sources and method

| Source (local only, gitignored `docs/research/evidence/wispr-flow/`) | Frames | Frame spacing (file mtimes) | Used for                                                                      |
| -------------------------------------------------------------------- | ------ | --------------------------- | ----------------------------------------------------------------------------- |
| `onboarding/o-*.jpg`                                                 | 401    | 0.83 s (1.2 fps)            | Onboarding, first-use chooser, spotlight, Hub home                            |
| `tours/cap-*.jpg` (+ `nt/`, `v1/` subsets)                           | 256    | ~1-2 s                      | Scratchpad, Transforms page, Transforms lesson, Notetaker, What's new         |
| `user-run/u-*.jpg` (+ `s/` subset)                                   | 115    | ~1-1.5 s                    | Whisper lesson and quiz, Insights, Style, Settings modal, Hub home, diff card |
| `notetaker-demo/n-*.jpg`                                             | 220    | 0.8 s                       | Notetaker panel (out of scope; read for chrome only)                          |
| `02-bar-click-01..22.png`, `03-bar-hover-three-modes.png`            | 23     | 0.5 s (2 fps)               | Flow Bar                                                                      |

- Full frames are 3456 × 2234 px: a 16-inch MacBook Pro at **2× backing scale**, so **1 pt = 2 px**. Every size below is in points (CSS px in our web views), measured on the 2× pixels with a small script (ink bounding boxes, edge transitions along a row/column, 3 × 3 median colour). The bar crops (`02-`, `03-`) are also 2× crops.
- Frames are JPEG: hex values carry ±2-3 levels of compression noise. Values are rounded to the nearest plausible token.
- Font sizes come from ink heights (cap height, ascender-to-descender) divided by the typical ratio for the identified face (grotesk cap ≈ 0.70 em; Garamond-style serif ascender ≈ 0.72 em). Expect ±1 px.
- The Wispr Flow window was 1348 × 848 pt (larger than our 1040 × 720 default). Proportions, not the absolute window size, are what we follow.
- **Dark mode:** no Wispr Flow Hub frame is in dark mode. The only dark references are the onboarding "Learn" screens, the bar, the diff card, and a dark screenshot of the Hub shown on Wispr's own website (visible in a browser behind the app in `user-run`). Dark values below are marked accordingly.
- **Motion:** at 0.5-1.2 fps, any transition shorter than ~0.8 s happens between two frames. Only choreography that spans several frames can be timed. Everything else in §8 is marked "not measurable".

## 1. Layout and chrome

### Hub window

| Element                            | Measured                                                                                                                                                                                                                                                |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Window chrome / sidebar background | `#F5F4F0` (warm off-white); one colour for title-bar strip and sidebar, no divider between them                                                                                                                                                         |
| Title-bar strip                    | 46 pt tall; traffic lights at x 21/44/67; sidebar-toggle icon at x 99; right side: theme toggle, bell, profile icons (20 pt, 36 pt apart)                                                                                                               |
| Content panel                      | A separate **inset card**: `#FCFCFA` fill, 1 px `#ECEBE6` border, **corner radius ≈ 22 pt**, starts flush at the sidebar edge, 10 pt from the window's right and bottom edges, and at the bottom of the title strip. The page scrolls inside this card. |
| Sidebar width                      | 216 pt (incl. 12 pt gutters)                                                                                                                                                                                                                            |
| Logo row                           | wordmark 20 pt tall, 28 pt below the title strip                                                                                                                                                                                                        |
| Sidebar items                      | 40 pt pitch, 15 px label, 18 pt icon at x+22, label at x+47; active item: `#ECECE4` fill (≈ 3 % darker than the sidebar), radius 8, 190 × 36 pt, no accent colour, label stays `#1A1A1A`                                                                |
| Sidebar card (setup checklist)     | white `#FFFFFD`, 1 px `#ECEBE6`, radius 12, 16 pt padding, 13 px semibold title, thin progress bar (4 pt, black fill on `#ECEBE6`), 13 px items with 18 pt hollow circles                                                                               |
| Sidebar footer links               | same item style, 40 pt pitch, separated from the checklist by a 1 px line                                                                                                                                                                               |
| Page padding                       | 40 pt left/right inside the content card; page title cap-top 47 pt below the card top                                                                                                                                                                   |
| Content width                      | main column 770 pt + 24 pt gap + 250 pt right rail (Home); full width 920-1000 pt elsewhere; Style centres a 800 pt column                                                                                                                              |

### Page anatomy (every Hub page)

1. **Header row:** page title (sans 24/600) on the left; on Scratchpad/Transforms a small black beta chip right of the title; page-level controls (toggle + labelled shortcut chip) right-aligned on the same baseline.
2. **Hero banner** (Home, Scratchpad, Transforms, Style, and the Dictionary/Snippets example banners described in R1): full content width, 180-200 pt tall, radius 16, **dark photographic background** with a left-to-right darkening gradient, serif headline 28 px white, one-line sans subtitle 15 px at 80 % white with **bold keywords**, a cream button (`#F0EDE6`, black text) and an optional text-only second button.
3. **Section header:** either an uppercase micro-label (12 px, +0.08 em tracking, `#6B6B6B`) such as the history day headers, or a serif section title (28 px) on the Transforms page.
4. **Content:** a list in one bordered white container with 1 px row dividers (History, Recents) or a grid of cards.

### Settings

A **modal over the Hub**, not a page: 960 × 640 pt, radius 16, centred, scrim `rgba(0,0,0,0.29)` (the `#FCFCFC` page became `#B3B2B0`). Left nav 210 pt on `#F5F4F0` with uppercase group labels (12 px tracked) and 15 px items with 18 pt icons, active item `#ECEBE6` radius 8; version and an update/cloud icon pinned at the bottom of the nav. Right pane `#FFFFFD`, 48 pt padding, **serif page title (28 px)**, 15 px intro paragraph at `#555`, then rows as tinted cards (`#F5F4F0`, 1 px `#ECEBE7`, radius 8, 86 pt tall, 16 pt gap, 20 pt padding) with a white secondary button on the right.

### Onboarding window

Same window, no sidebar. Header: a centred five-stage stepper (uppercase 12-13 px, +0.08 em, chevrons between stages; current and past stages `#1A1A1A`, future `#7A7A7A`), a language dropdown at the top right, and a **2 pt purple progress line** (`#9A72BA`) under the header showing progress across the whole flow. "← Back" link 15 px at 116 pt / 96 pt. Two layouts:

- **Split (Get started, Permissions):** left pane 576 pt on `#FCFCFC` with title, subtitle and choices left-aligned at x 112; right pane on `#F0EEE8` with an illustration, a looping product demo, or a drawn macOS screen, vertically centred. Primary button pinned at the bottom of the left pane: **black `#1A1A1A`, 360 × 40 pt, radius 8**, 15 px medium white; disabled = `#919191` fill.
- **Centred (Set up, Learn):** full-width warm canvas `#F0EDE6` with a giant low-contrast decorative glyph pattern (`#E8E4DC`, shapes 300-600 pt), a centred title (32 px/600), a centred white test card (radius 12, soft shadow), primary button centred at the bottom (300 × 40 pt). Learn screens switch to a **dark canvas `#232323`** with the same pattern at `#2A2A2A`.

### Lessons and dialogs (Transforms, Whisper, first-use chooser)

Large **sheets** over a scrim: 940-960 × 640-670 pt, radius 16, background `#F7F6F2` (Whisper) or white (Transforms). Top: back chevron (left), a **progress of short dashes** (3 × 32 pt, 3 pt tall, black = done, grey = to come), close ✕ (right). Centred **serif heading 28-32 px**, with one word in **italic** (and in purple `#8A5DB0` on Transforms). Footer: single centred black button 96-120 × 36 pt (radius 8).

Small dialogs (Shortcuts, Add word): 666 × 450 pt, radius 16, white, 24 pt padding, 17 px semibold title + 15 px description, rows as tinted cards (radius 8), close button 28 pt square on `#F0EFEB` radius 6, footer with a beige secondary (`#ECEAE4`) left and a black primary right.

## 2. Type

| Role                    | Face (look)                | Size / line height | Weight          | Tracking  | Measured from                                  |
| ----------------------- | -------------------------- | ------------------ | --------------- | --------- | ---------------------------------------------- |
| Celebration display     | Garamond-style serif       | ≈ 96-104 px / 1.0  | 400             | -0.01 em  | headline ink 103 pt incl. a descender (o-0262) |
| Display (hours saved)   | serif + italic serif mixed | ≈ 40-48 px         | 400             | 0         | o-0268                                         |
| Onboarding title        | grotesk                    | 32 px / 40 px      | 600             | -0.01 em  | 28.5 pt ascender-to-descender (o-0045)         |
| Lesson / modal heading  | serif, one word italic     | 28-32 px / 1.15    | 400             | -0.01 em  | u-021, cap-206                                 |
| Banner headline         | serif                      | 28 px / 34 px      | 400             | -0.01 em  | 20.5 pt ascender height (u-073)                |
| Stat numbers            | serif                      | 28 px              | 400             | 0         | 19.5 pt figure height (u-073)                  |
| Page title (Hub)        | grotesk                    | 24 px / 32 px      | 600             | -0.005 em | 17.5 pt cap+ascender (u-073)                   |
| Settings page title     | serif                      | 28 px              | 400             | 0         | u-033                                          |
| Section title (in page) | serif                      | 28 px              | 400             | 0         | section title on cap-024                       |
| Card heading (Insights) | grotesk                    | 28 px              | 600             | -0.01 em  | card heading 25.5 pt incl. descender (u-075)   |
| Card title / row title  | grotesk                    | 16 px / 22 px      | 600             | 0         | rail card title 12 pt cap+ascender             |
| Body / list text        | grotesk                    | 15 px / 20 px      | 400             | 0         | history rows, banner subtitle                  |
| Secondary body          | grotesk                    | 14-15 px / 20 px   | 400             | 0         | `#555`-`#6B6B6B`                               |
| Sidebar item            | grotesk                    | 15 px              | 500             | 0         | 11.5 pt cap+ascender                           |
| Button                  | grotesk                    | 15 px              | 500             | 0         | banner button 11.5 pt                          |
| Small button / chip     | grotesk                    | 13 px              | 500             | 0         | chips, kbd keys                                |
| Caption                 | grotesk                    | 12-13 px / 16 px   | 400             | 0         | 8.5 pt x-height+ascender                       |
| Micro label             | grotesk, UPPERCASE         | 12 px              | 500             | +0.08 em  | day headers, stat labels, stepper              |
| Tiny                    | grotesk                    | 11 px              | 400-500         | 0         | progress caption                               |
| Typing test             | monospace                  | 15 px / 26 px      | 400             | 0         | o-0300                                         |
| Bar label / tooltip     | grotesk                    | 15-16 px           | 500 (fn in 700) | 0         | 11 pt cap of the mic label                     |

**Faces.** Two families do all the work, plus a mono for one screen:

- **UI sans:** a geometric-humanist grotesk with a double-storey `a`, round `o`/`e`, short ascenders, slightly squarish dots, tabular-looking figures. Closest free fonts (OFL-1.1): **Figtree** (best match by eye in proportions and the `a`, `e`, `k`, `t`), then **Instrument Sans** or **Inter**. Not system SF Pro (SF is narrower, with a taller x-height and flatter curves).
- **Display serif:** an old-style **Garamond** with high stroke contrast, small x-height, a calligraphic italic (swash-like `f` in the italic). Closest free fonts (OFL-1.1): **EB Garamond** (best; has the italic), then **Cormorant Garamond** (more contrast, too delicate below 28 px). Our current Newsreader is a newspaper serif with a larger x-height, heavier serifs and lower contrast, which is why our headings read "bookish", not "editorial".
- **Mono:** a neutral monospace for the typing test only (e.g. **JetBrains Mono** or **IBM Plex Mono**, both OFL).

**Rules seen everywhere:** serif only for _display moments_ (banners, celebration, lesson headings, stat numbers, Settings page titles, one section title per page); never for body, labels or lists. Mixed roman + italic serif for emphasis (an italic verb in a practice heading, an italic phrase after the hours-saved number). Sans titles are semibold, not bold. Body never goes below 13 px except micro labels and captions.

## 3. Colour

### Light (measured)

| Token role                                | Hex                                                                                                                                                             | Where                                                                          |
| ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Chrome / sidebar / tinted card            | `#F5F4F0`                                                                                                                                                       | window, sidebar, stat card, Settings nav, MCP rows, Insights cards (`#F7F4EF`) |
| Content panel                             | `#FCFCFA`                                                                                                                                                       | Hub page card                                                                  |
| Pure surface                              | `#FFFFFF` / `#FFFFFD`                                                                                                                                           | list containers, sidebar checklist, dialogs                                    |
| Warm canvas (onboarding centred, lessons) | `#F0EDE6` / `#F7F6F2`                                                                                                                                           | Set-up screens, Whisper sheet                                                  |
| Decorative glyph on canvas                | `#E8E4DC`                                                                                                                                                       | onboarding background pattern                                                  |
| Hairline border                           | `#ECEBE6`                                                                                                                                                       | panel, cards, list dividers (1 px)                                             |
| Active nav fill                           | `#ECECE4`                                                                                                                                                       | sidebar + Settings nav                                                         |
| Hover row                                 | `#F3F2EE` (≈ 3 % ink)                                                                                                                                           | history row hover                                                              |
| Ink                                       | `#1A1A1A`                                                                                                                                                       | text, primary buttons, toggles (on), checkboxes                                |
| Ink 2                                     | `#555555`                                                                                                                                                       | descriptions                                                                   |
| Ink 3                                     | `#6B6B6B`-`#7A7A7A`                                                                                                                                             | micro labels, times, future stepper stages                                     |
| Disabled                                  | `#919191` fill / `#B8B8B8` text                                                                                                                                 |
| Cream button                              | `#F0EDE6` on dark banners; `#ECEAE4` secondary on light                                                                                                         |
| Accent purple (brand)                     | `#6A37A0` (voice-profile bar), `#7B3FB0`-`#8A5DB0` (level meter bars, italic accent words), `#9A72BA` (progress line, selected card border 1.5 px)              |
| Accent lilac fills                        | `#F5ECFD` (selected option card), `#ECD8FD` (bar chart fill on dark), `#E8DDF0` (level meter idle bars)                                                         |
| Data teal                                 | `#2E5C5C` (primary series, gauge), `#539289` (secondary), `#A9D3CB` / `#D3ECE7` (light steps of the streak heatmap), `#C4C0B5` (empty heatmap cell)             |
| Success                                   | green check `#3E8E5A` (permission rows), recommended-column header green `#4CAF6A`                                                                              |
| Status / badge                            | new-feature chip `#F6C78A` fill / `#7A4A12` ink; Beta chip black `#1A1A1A` / white, or lilac `#F3E6FB` / `#8A5DB0` on tabs; teal dot badge `#5FC2B4` on the bar |
| Warning / error                           | orange banner `#F8E3CF` / `#9A4A12` (Notetaker snag), red `#E5484D` (delete icon in hover actions)                                                              |
| Scrim                                     | `rgba(0,0,0,0.29)` dialogs; `rgba(0,0,0,0.55)` + circular cut-out for the spotlight                                                                             |

Colour use is **restrained**: the UI is ink on warm whites; purple appears only for progress, selection and one emphasised word; teal only in data. Primary actions are **black**, not coloured.

### Dark (partial, see §0)

| Role                                 | Hex                                                                 | Source                             |
| ------------------------------------ | ------------------------------------------------------------------- | ---------------------------------- |
| Learn canvas                         | `#232323` with pattern `#2A2A2A`                                    | o-0262                             |
| Dark sidebar / chrome                | `#1F1F1F`                                                           | website screenshot of the dark Hub |
| Dark content                         | `#1A1A1A`                                                           | same                               |
| Text on dark                         | `#F3F3F1`, secondary `#D8D7D3`                                      | o-0262                             |
| Dark card (bar, diff card, tooltips) | `#000000` (bar), `#1B1B1B` (diff card), `#30302F` (bar sub-buttons) |
| Light button on dark                 | `#D8D7D3` fill, `#1A1A1A` text                                      | Continue on Learn                  |
| Lilac on dark                        | `#ECD8FD` (bars), `#C9A7F0` (italic accent)                         | o-0262                             |
| Teal inserted chip on dark           | ≈ `#2E5C5C` fill, `#D3ECE7` text                                    | u-067                              |

The full dark Hub palette **could not be measured** (no frames); SYSTEM-PLAN derives it from these anchors.

## 4. Spacing, sizing, radius, elevation

**Grid:** 4 pt base; the common steps are 4, 8, 12, 16, 20, 24, 32, 40, 48.

| Element                           | Size / spacing                                           | Radius                     | Border / shadow                                             |
| --------------------------------- | -------------------------------------------------------- | -------------------------- | ----------------------------------------------------------- |
| Content panel                     | inset 0/10/10 pt                                         | 22                         | 1 px `#ECEBE6`, no shadow                                   |
| Hero banner                       | 180-200 pt tall, 32 pt padding                           | 16                         | none (photo)                                                |
| Stat card                         | 250 pt wide, 24 pt padding, rows 44 pt                   | 14                         | 1 px `#ECEBE6`, tinted fill                                 |
| List container (history, recents) | rows 52 pt min, 16-17 pt side padding, time column 98 pt | 12                         | 1 px `#ECEBE6`, row dividers 1 px                           |
| Card grid (Transforms, Style)     | 255-270 pt wide, 16-24 pt gap, 24 pt padding             | 12 (Transforms), 8 (Style) | 1 px `#ECEBE6`; selected 1.5 px purple                      |
| Insights cards                    | 16-24 pt gap                                             | 12                         | 1 px `#ECEBE7`, `#F7F4EF` fill                              |
| Settings row card                 | 86 pt, 20 pt padding, 16 pt gap                          | 8                          | 1 px `#ECEBE7`                                              |
| Option card (onboarding)          | 344 × 110 pt, 22 pt padding, 16 pt gap                   | 10                         | 1 px ink; selected lilac fill                               |
| Test card (onboarding)            | 516 pt wide, 24 pt padding                               | 12                         | shadow `0 8px 24px rgba(0,0,0,0.06)`                        |
| Sheets / dialogs                  | 940-960 × 640 / 666 × 450                                | 16                         | shadow `0 24px 64px rgba(0,0,0,0.18)` + scrim               |
| Primary button                    | 36-40 pt tall, 16-24 pt padding                          | 8                          | none                                                        |
| Secondary button (white on tint)  | 32 pt tall                                               | 8                          | 1 px `#E6E5E0`                                              |
| Chip (choice)                     | 36 pt tall, 12 pt padding, 13-15 px                      | 8                          | 1 px ink-ish `#C9C8C3`; selected lilac fill + purple border |
| kbd key chip                      | 24-26 pt tall, 6-8 pt padding                            | 5-6                        | `#F0EFEB` fill, 1 px `#E3E2DD`                              |
| Toggle                            | 40 × 24 pt                                               | full                       | on = black track                                            |
| Text field                        | 40 pt                                                    | 8                          | 1 px `#E3E2DD`                                              |
| Tooltip / bar label               | 34 pt tall                                               | full                       | black, no border                                            |

**Elevation** is almost flat: cards are separated by 1 px warm hairlines and a 1-3 % tint step, not by shadows. Shadows appear only on floating things (dialogs, test cards, composer mocks, popovers). **No blur/vibrancy** is visible on the Hub (the sidebar is an opaque `#F5F4F0`; it did not pick up the blue desktop behind it). The bar and the diff card are opaque black.

## 5. Icons

- **Style:** outline, round caps and joins, ~**1.5 pt stroke at 18-20 pt** (≈ 1.5 px at 2×: 3 px lines), drawn on a 24 grid scaled to 18-20 pt. Filled glyphs only for state (record dot, the checked circle, the bar's mic).
- **Sizes:** 18 pt sidebar/nav, 16-18 pt row hover actions (play, copy, flag, ⋮), 20 pt title-bar icons, 14 pt inline info icons.
- **Colour:** always ink (`#1A1A1A`) or ink-3; never coloured, except the destructive trash (red) in hover actions.
- **Closest free set:** **Lucide** (ISC; already a dependency) at `strokeWidth={1.5}`; alternatives Phosphor Regular (MIT) and Tabler (MIT). Hero/feature art uses no icon set.

## 6. Illustration and imagery (describe only; never reuse)

| Kind                                   | Where                                                                                           | Description                                                                                                                                                                                                               | Licensed equivalent for us                                                                                                                                                                                                              |
| -------------------------------------- | ----------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Dark, blurred, warm photographs**    | Hub hero banners (Home, Scratchpad, Transforms, Style)                                          | Shallow-depth photo (hands on a laptop, paper notes, an interior) in browns/blacks, heavy blur on the right, darkened on the left for text. Transforms/Style banners float **frosted circular app icons** over the photo. | Our own photos (the owner's), or CC0 photos (Openverse/Wikimedia CC0, Unsplash is _not_ CC0) processed by us: blur, grade, grain. App icons: the user's installed apps' icons read at runtime (we already do for Style), never shipped. |
| **Flat hand-drawn characters**         | Onboarding questions (person at desk, person with laptop)                                       | Bold black outlines, flat pink/magenta/maroon fills, oversized limbs, a bit of texture.                                                                                                                                   | Open Peeps or Humaaans (CC0), Open Doodles (CC0); recoloured to our palette.                                                                                                                                                            |
| **Doodle stickers**                    | Celebration (hand-drawn arrow, a scooter with flames)                                           | Single-weight white strokes on dark, one small colour sticker.                                                                                                                                                            | Draw our own SVGs (arrow, rocket); Open Doodles (CC0).                                                                                                                                                                                  |
| **Decorative brand glyph pattern**     | Onboarding centred screens and Learn screens                                                    | The logo's curve enlarged to 300-600 pt, tiled, 3-5 % contrast.                                                                                                                                                           | Make from **our** logo shape.                                                                                                                                                                                                           |
| **Drawn macOS screens**                | Permissions                                                                                     | A redrawn System Settings window with the real prompt in it.                                                                                                                                                              | We already ship our own SVGs (`onboarding/art`); restyle.                                                                                                                                                                               |
| **App mocks**                          | Practice steps, Transforms lesson, first-use chooser                                            | Simplified composers in the style of a chat app, an email client, a notes app, an AI chat, with the real bar animating under them.                                                                                        | Our scenes engine; generic, unbranded mocks.                                                                                                                                                                                            |
| **Colourful gradient wallpaper**       | First-use chooser demo backdrop                                                                 | macOS-like orange/blue/purple mesh gradient behind a mock window.                                                                                                                                                         | CSS mesh gradient we write.                                                                                                                                                                                                             |
| **Real photos of people and products** | Whisper lesson (a person whispering; a row of 4 mic photos), mic grid (product photos on white) | Lifestyle photography; studio product shots.                                                                                                                                                                              | CC0/CC-BY-4.0 photos, or our own 3D/flat illustrations of generic mic types (Blender renders from CC0 models, or SVG line art).                                                                                                         |
| **Video**                              | "How it works" modals, Notetaker tour                                                           | Hosted screen recordings with a custom player (purple scrubber), a Loading screen before.                                                                                                                                 | Our own WebM screen recordings of Wispr Bro, bundled; or our animated scenes.                                                                                                                                                           |
| **Confetti / emoji**                   | none observed in frames                                                                         | n/a                                                                                                                                                                                                                       | canvas-confetti (ISC) is already ours; keep subtle.                                                                                                                                                                                     |
| **Circular rotating text badge**       | Insights share badge                                                                            | Text set on a circle around an icon.                                                                                                                                                                                      | CSS/SVG `textPath`, ours.                                                                                                                                                                                                               |

## 7. Bar (Flow Bar)

Measured on 2× crops (pt):

| State                                 | Size                                                           | Look                                                                                                                                                                                                                      |
| ------------------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Resting                               | **38 × 6 pt** pill                                             | dark grey `#3A3A3A` at ~70 % on the desktop, no border; on hover/press a **purple radial glow** (`#A687EB` → transparent, ~12 pt) blooms at its centre                                                                    |
| Resting with Scratchpad (three modes) | mic pill 49 × 29, two 29 pt circles, 6 pt gaps                 | `#30302F` fills, 1 px lighter edge, white 16 pt glyphs; teal 6 pt status dot top-right of the record button                                                                                                               |
| Hover tooltip                         | 97 × 34 pt above, 8 pt gap                                     | black pill, 16 px white text, key name in bold                                                                                                                                                                            |
| Push-to-talk (held)                   | **72 × 30 pt**                                                 | black capsule, ~11 white bars 2 pt wide, 3 pt gap, heights 4-14 pt, centre-weighted                                                                                                                                       |
| Hands-free / click-started            | **≈ 100 × 30 pt**                                              | ✕ in an 18 pt `#30302F` circle (left), waveform (centre; a dotted line before audio arrives), ✓ in an 18 pt **white** circle with black check (right)                                                                     |
| Mic label                             | 260 × 40 pt, 8 pt above the capsule                            | black pill, 16 px white text; visible ~2 s (4 frames at 2 fps), enters with **opacity + blur** (frame 3 shows it blurred and translucent)                                                                                 |
| Processing (transform in progress)    | ≈ 136 × 30 pt                                                  | split capsule: dark grey text segment + black segment with a purple/white spinner                                                                                                                                         |
| Diff card                             | ≈ 500 × 250 pt above the bar                                   | `#1B1B1B`, radius 16, 15 px white text, deleted words struck through on a grey chip, inserted words on a teal chip, a header row (change count, configure link), 4 outline actions (copy, thumbs up/down, retry) at 18 pt |
| Spotlight                             | full-screen dim with a ~220 pt circular cut-out around the bar | the bar shows a hint label                                                                                                                                                                                                |

Our geometry (`src/fork/pill/geometry.gen.json`): rest 60 × 12, held 112 × 34, locked 168 × 34. **Ours is 1.6× wider and 2× taller at rest and ~1.5× larger when active.**

## 8. Motion

What could be timed from multi-frame sequences:

| Moment                              | Observed                                                                                                                                                                                                                 | Timing                                                   |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------- |
| Celebration (first-dictation)       | **Staged reveal**: headline → subtitle + two speed bars → a hand-drawn arrow _draws itself_ while the speed multiplier **rolls like an odometer** → the sticker illustration pops in → caption → Continue                | 4 stages over **~3.3 s** (o-0255→0261, 0.83 s per frame) |
| Hours saved                         | number counts up 0 → 25 between two frames; dragging the slider rolls digits (mid-roll glyphs visible)                                                                                                                   | count-up < 0.8 s; digits roll vertically                 |
| First-screen demo (right pane)      | loops a scene: composer types, the bar expands with a waveform, a caption chip of the spoken words appears under it, text lands, cursor blinks; scenes alternate (email ↔ chat) and swap when the selected card changes | ~6-8 s per scene                                         |
| Mic test                            | bars light up live left-to-right in purple; idle bars lilac                                                                                                                                                              | live (30 fps class)                                      |
| Shortcut test                       | on-screen key turns purple while pressed                                                                                                                                                                                 | live                                                     |
| Bar mic label                       | appears with the capsule, holds, fades                                                                                                                                                                                   | visible ~2 s, blur-in                                    |
| Bar capsule before audio            | dotted placeholder line, then bars                                                                                                                                                                                       | < 0.5 s                                                  |
| Lesson practice → done              | heading text swaps in place; then the whole sheet dims and a **circular spotlight** isolates the bar; then returns                                                                                                       | spotlight held ~3 s (4 frames)                           |
| Video lesson                        | a loading card with the logo and a thin progress line precedes the player                                                                                                                                                | ~1-2 frames                                              |
| Modals, page switches, hover, press | appear complete in the next frame                                                                                                                                                                                        | **not measurable** (< 0.8 s)                             |

Inferred (not measurable at this frame rate, stated as a target, not a measurement): fades/scales under 250 ms; no visible bounce in any frame.

## 9. Density and copy length

| Screen                       | Words on screen (approx.) | Headline              | Body copy                                                      |
| ---------------------------- | ------------------------- | --------------------- | -------------------------------------------------------------- |
| Hub home                     | 60 + history              | 3 words               | banner: ≤ 6-word serif headline + 1 sentence (≤ 15 words)      |
| Scratchpad / Transforms page | 50-70                     | 1 word + Beta chip    | banner 1 headline + 1-2 sentences; card descriptions ≤ 5 words |
| Insights                     | ~70 (mostly numbers)      | 1 word                | none; labels are 2-3 uppercase words                           |
| Style                        | ~60                       | 1 word                | banner 1 sentence; card = 2 short lines + a sample bubble      |
| Settings section             | 40-70                     | 1-2 words (serif)     | 1 intro sentence; row = title + ≤ 10-word description          |
| Onboarding screen            | 15-40                     | ≤ 6 words             | ≤ 2 lines (≤ 20 words); options: 1-word title + ≤ 10-word line |
| Lesson step                  | 10-30                     | ≤ 8 words, one italic | ≤ 1 line                                                       |
| Dialog                       | 20-40                     | 1 word                | 1 line + row descriptions ≤ 8 words                            |

Density rule: **one idea per screen, one primary action**, and whitespace instead of explanations. Wispr Flow explains with pictures and practice, not paragraphs.
