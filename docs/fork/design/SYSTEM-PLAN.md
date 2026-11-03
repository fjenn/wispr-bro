# Design system v2: the plan for the rebuild

Design audit, 2026-11-03. Inputs: [REFERENCE-SPEC.md](REFERENCE-SPEC.md) (what Wispr Flow measures) and [GAP-AUDIT.md](GAP-AUDIT.md) (where we fall short). This file proposes what to build next: tokens, fonts, icons, native effects, the animation library and its rules, the assets to source, and how to split the rebuild across parallel agents.

Direction (owner, 2026-11-03): follow Wispr Flow's design language closely. Keep our own name, logo, copy, illustrations, photos and sounds; never ship a Wispr Flow file or sentence.

Decisions I made on the obvious calls, so agents do not stop to ask:

1. **Primary buttons become black** (`ink`), not indigo. Indigo is retired from fills; our brand colour becomes a purple used the way Wispr Flow uses theirs (progress, selection, one emphasised word).
2. **The Hub stays opaque.** Wispr Flow has no vibrancy in its main window; we match it. Native effects are used where they buy feel without changing the look (§4).
3. **Page titles go sans; serif becomes a display accent.** Newsreader is replaced by EB Garamond.
4. **System SF Pro is replaced by Figtree** for UI text, with Inter as the fallback for scripts Figtree lacks.
5. **One motion library (`motion`)**, one set of presets, Reduce Motion handled in one place.

## 1. Tokens

All tokens live in `src/fork/styles/tokens.css` (v2), same light/dark/`data-theme` scheme as today, exposed to Tailwind 4 through `@theme`. Names below are the CSS variables; Tailwind utilities follow (`bg-surface`, `text-ink-2`, `rounded-card`…).

### 1.1 Type scale

| Token                | Family        | Size / line | Weight  | Tracking            | Use                                                                              |
| -------------------- | ------------- | ----------- | ------- | ------------------- | -------------------------------------------------------------------------------- |
| `--type-hero`        | display serif | 96 / 96     | 400     | -0.02 em            | celebration headline                                                             |
| `--type-display`     | display serif | 44 / 50     | 400     | -0.015 em           | time saved, big numbers                                                          |
| `--type-heading-xl`  | sans          | 32 / 40     | 600     | -0.012 em           | onboarding titles                                                                |
| `--type-sheet`       | display serif | 30 / 36     | 400     | -0.01 em            | lesson / chooser / quiz headings (one word italic)                               |
| `--type-serif-title` | display serif | 28 / 34     | 400     | -0.01 em            | banner headlines, Settings page titles, one section title per page, stat numbers |
| `--type-title`       | sans          | 24 / 32     | 600     | -0.006 em           | Hub page titles                                                                  |
| `--type-heading`     | sans          | 17 / 24     | 600     | 0                   | dialog titles                                                                    |
| `--type-card-title`  | sans          | 16 / 22     | 600     | 0                   | card and row titles                                                              |
| `--type-body`        | sans          | 15 / 20     | 400     | 0                   | body, list text, inputs                                                          |
| `--type-body-strong` | sans          | 15 / 20     | 500     | 0                   | buttons, nav items                                                               |
| `--type-small`       | sans          | 13 / 18     | 400/500 | 0                   | chips, secondary buttons, kbd, captions                                          |
| `--type-micro`       | sans          | 12 / 16     | 500     | +0.08 em, uppercase | group labels, day headers, stat labels, stepper                                  |
| `--type-tiny`        | sans          | 11 / 14     | 500     | 0                   | progress captions                                                                |
| `--type-mono`        | mono          | 15 / 26     | 400     | 0                   | typing test                                                                      |

Numbers in stats and counters use `font-variant-numeric: tabular-nums`.

### 1.2 Palette

Light values are measured (REFERENCE-SPEC §3); dark values extend the measured dark anchors and must pass the existing AA checks in `tests/fork/hub.spec.ts` (rerun and adjust by ±1 step if a pair fails).

| Token                            | Light                      | Dark                       | Use                                                         |
| -------------------------------- | -------------------------- | -------------------------- | ----------------------------------------------------------- |
| `--chrome`                       | `#F5F4F0`                  | `#1C1C1B`                  | window, sidebar, Settings nav                               |
| `--panel`                        | `#FCFCFA`                  | `#161615`                  | inset content panel                                         |
| `--surface`                      | `#FFFFFF`                  | `#222220`                  | lists, dialogs, sidebar card                                |
| `--surface-tint`                 | `#F5F4F0`                  | `#262624`                  | stat cards, Settings rows, Insights cards                   |
| `--canvas-warm`                  | `#F0EDE6`                  | `#232323`                  | onboarding centred screens, lesson sheets (`#F7F6F2` light) |
| `--pattern`                      | `#E8E4DC`                  | `#2A2A2A`                  | decorative glyph pattern                                    |
| `--line`                         | `#ECEBE6`                  | `#2E2D2A`                  | hairlines, card borders, dividers                           |
| `--line-strong`                  | `#D9D7D0`                  | `#45433F`                  | inputs, chips, focusable edges (3:1)                        |
| `--fill-active`                  | `#ECECE4`                  | `#2B2A27`                  | selected nav item                                           |
| `--fill-hover`                   | `rgba(26,26,26,0.04)`      | `rgba(255,255,255,0.05)`   | row/item hover                                              |
| `--fill-press`                   | `rgba(26,26,26,0.08)`      | `rgba(255,255,255,0.09)`   | pressed                                                     |
| `--ink`                          | `#1A1A1A`                  | `#F3F2EE`                  | text, primary button fill, toggles on                       |
| `--ink-2`                        | `#555555`                  | `#B9B7B1`                  | descriptions                                                |
| `--ink-3`                        | `#6B6B6B`                  | `#94928C`                  | micro labels, times (AA on chrome and panel)                |
| `--on-ink`                       | `#FFFFFF`                  | `#1A1A1A`                  | text on primary buttons                                     |
| `--cream`                        | `#F0EDE6`                  | `#F0EDE6`                  | button on dark photo banners                                |
| `--secondary`                    | `#ECEAE4`                  | `#302F2C`                  | secondary button fill                                       |
| `--accent`                       | `#6A37A0`                  | `#C3A0EE`                  | progress bars, links in body, focus ring                    |
| `--accent-2`                     | `#9A72BA`                  | `#9A72BA`                  | progress line, selected card border                         |
| `--accent-soft`                  | `#F5ECFD`                  | `#2E2438`                  | selected option/chip fill                                   |
| `--accent-meter`                 | `#7B3FB0` / idle `#E8DDF0` | `#B48AE6` / idle `#3A3045` | live level bars, pressed key                                |
| `--data-1`                       | `#2E5C5C`                  | `#6FB3A8`                  | primary chart series                                        |
| `--data-2`                       | `#539289`                  | `#4E8C82`                  | secondary series                                            |
| `--data-3`, `--data-4`           | `#A9D3CB`, `#D3ECE7`       | `#335C57`, `#24403C`       | heat steps                                                  |
| `--data-empty`                   | `#E3E0D8`                  | `#2E2D2A`                  | empty cells                                                 |
| `--success`                      | `#3E8E5A`                  | `#6CC08A`                  | ticks, "best match"                                         |
| `--warning-bg` / `--warning-ink` | `#F8E3CF` / `#8A4210`      | `#3B2A1C` / `#F2C49B`      | warnings                                                    |
| `--danger`                       | `#C93C3F`                  | `#F07D80`                  | destructive                                                 |
| `--tag-new-bg` / `-ink`          | `#F6D9B0` / `#6E410C`      | `#4A3418` / `#F6D9B0`      | "New"                                                       |
| `--tag-beta-bg` / `-ink`         | `#1A1A1A` / `#FFFFFF`      | `#F3F2EE` / `#1A1A1A`      | "Beta"                                                      |
| `--hud`                          | `#000000`                  | `#000000`                  | bar, tooltips, bar labels                                   |
| `--hud-2`                        | `#1B1B1B`                  | `#1B1B1B`                  | bar card and diff card                                      |
| `--hud-control`                  | `#30302F`                  | `#30302F`                  | ✕ button, bar sub-buttons                                   |
| `--scrim`                        | `rgba(0,0,0,0.29)`         | `rgba(0,0,0,0.55)`         | dialogs                                                     |

Increase Contrast keeps today's approach (S-37): darker `--ink-2/3`, `--line` → `--line-strong`, thicker focus ring.

### 1.3 Spacing and layout

`--space-1..12` = 4, 8, 12, 16, 20, 24, 32, 40, 48, 56, 64, 80 pt.

| Token               | Value                                                               |
| ------------------- | ------------------------------------------------------------------- |
| `--sidebar-width`   | 216 (collapsed 64, unchanged)                                       |
| `--titlebar-height` | 46 (title strip; traffic lights re-centred, §4)                     |
| `--panel-inset`     | 0 left, 10 right, 10 bottom                                         |
| `--page-pad-x`      | 40 (32 below 860 pt wide)                                           |
| `--page-pad-top`    | 36                                                                  |
| `--rail-width`      | 250, gap 24                                                         |
| `--nav-item-h`      | 36, pitch 40                                                        |
| `--control-h`       | 36 (buttons, inputs), 40 for primary CTAs in onboarding, 32 compact |
| `--row-h`           | 52 min (history), 64 (settings rows with description)               |

### 1.4 Radius

| Token           | Value | Use                                                     |
| --------------- | ----- | ------------------------------------------------------- |
| `--radius-xs`   | 5     | kbd keys, tiny chips                                    |
| `--radius-sm`   | 8     | buttons, inputs, nav items, settings rows, option chips |
| `--radius-md`   | 12    | cards, list containers, test cards, popovers            |
| `--radius-lg`   | 16    | banners, dialogs, sheets, diff card                     |
| `--radius-xl`   | 22    | inset content panel                                     |
| `--radius-full` | 9999  | bar, tooltips, toggles, avatars                         |

Today's 14/10 values go away.

### 1.5 Elevation

| Token      | Value                                                    | Use                                    |
| ---------- | -------------------------------------------------------- | -------------------------------------- |
| `--elev-0` | none + 1 px `--line`                                     | every card and list                    |
| `--elev-1` | `0 1px 2px rgba(0,0,0,.04), 0 4px 16px rgba(0,0,0,.08)`  | popovers, menus, tooltips on light     |
| `--elev-2` | `0 8px 24px rgba(0,0,0,.06)`                             | onboarding test cards, drawn composers |
| `--elev-3` | `0 24px 64px rgba(0,0,0,.18), 0 2px 6px rgba(0,0,0,.06)` | dialogs and sheets                     |

Dark: same offsets, alpha × 2.5, plus a 1 px `rgba(255,255,255,.06)` top inner highlight on raised surfaces.

### 1.6 Motion

| Token           | Value                                                     | Use                                       |
| --------------- | --------------------------------------------------------- | ----------------------------------------- |
| `--dur-instant` | 90 ms                                                     | press feedback                            |
| `--dur-fast`    | 140 ms                                                    | hover, colour, small fades                |
| `--dur-base`    | 220 ms                                                    | popovers, chips, tabs underline           |
| `--dur-slow`    | 320 ms                                                    | dialogs in, page/step change              |
| `--dur-stage`   | 80 ms                                                     | stagger between staged items              |
| `--ease-out`    | `cubic-bezier(0.22, 1, 0.36, 1)` (today's `--ease-glide`) | anything entering                         |
| `--ease-in`     | `cubic-bezier(0.55, 0, 1, 0.45)`                          | anything leaving (≈ 0.7 × the enter time) |
| `--ease-in-out` | `cubic-bezier(0.65, 0, 0.35, 1)`                          | moves between two places                  |
| spring `snappy` | stiffness 520, damping 40, mass 1                         | buttons, toggles, chips, list reorder     |
| spring `gentle` | stiffness 260, damping 30                                 | dialogs, sheets, cards                    |
| spring `bar`    | stiffness 600, damping 38, mass 0.8                       | Flow Bar size morphs                      |

## 2. Fonts (all OFL-1.1, vendored through `scripts/wb-fonts.ts` like today)

| Role                                  | Font                                         | Package                               | Coverage                    | Licence |
| ------------------------------------- | -------------------------------------------- | ------------------------------------- | --------------------------- | ------- |
| UI sans                               | **Figtree** (variable 300-900)               | `@fontsource-variable/figtree`        | Latin, Latin-ext            | OFL-1.1 |
| UI sans fallback                      | **Inter** (variable)                         | `@fontsource-variable/inter`          | Cyrillic, Greek, Vietnamese | OFL-1.1 |
| Display serif                         | **EB Garamond** (variable, roman + italic)   | `@fontsource-variable/eb-garamond`    | Latin, Cyrillic, Greek      | OFL-1.1 |
| Mono                                  | **JetBrains Mono** (variable)                | `@fontsource-variable/jetbrains-mono` | Latin, Cyrillic, Greek      | OFL-1.1 |
| CJK, Arabic, Hebrew, Thai, Devanagari | system fonts (unchanged), Handy's CJK subset | n/a                                   |                             |         |

Stacks: `--font-ui: "WB Figtree", "WB Inter", -apple-system, system-ui, sans-serif`; `--font-display: "WB EB Garamond", ui-serif, "New York", Georgia, serif`; `--font-mono: "WB JetBrains Mono", ui-monospace, "SF Mono", monospace`. Newsreader and Source Serif 4 leave the bundle (LICENSE-REGISTER and NOTICE §7 updated in the same PR). Budget: ≤ 450 KB of woff2 for Latin; other subsets load only by `unicode-range`.

Rules: serif only for the display roles in §1.1; never below 24 px; emphasis in serif headings is the italic, never bold. UI weights: 400, 500, 600 only.

## 3. Icons

**Lucide** (`lucide-react`, ISC, already a dependency) through one wrapper, `src/fork/ui/Icon.tsx`: `size` 18 default (16 in rows, 20 in the title strip, 14 inline), `strokeWidth` 1.5 always, `absoluteStrokeWidth`, colour `currentColor`. An ESLint rule (or a guard in `scripts/wb-guards.ts`) bans direct `lucide-react` imports outside the wrapper and bans inline `<svg>` icons in fork pages, so the set never mixes again. Filled variants only for state (selected radio, record dot). The native resting bar keeps SF Symbols drawn by AppKit (system-provided at runtime; never exported as files).

Alternatives considered: Phosphor Regular (MIT) is closer in feel to some Wispr glyphs but would add a 2nd dependency for no structural gain; Tabler (MIT) has a "text size" glyph we lack. If a needed glyph is missing in Lucide, draw it on Lucide's 24 grid at 1.5 stroke and keep it in `src/fork/ui/icons/`.

## 4. macOS-native effects

| Effect                                           | Plan                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Title strip                                      | Keep `TitleBarStyle::Overlay` + hidden title (`fork/hub/mod.rs`); make the strip 46 pt and move the traffic lights to its centre (Tauri 2 `traffic_light_position` on macOS where our pinned Tauri exposes it, else one AppKit call next to the existing title-bar code), x 18 / y 23. Theme, bell and profile icons on the right of the same strip.                                    |
| Window corner and inset panel                    | Opaque window; the 22 pt panel is drawn in CSS. No `transparent: true` (costs compositing, breaks text AA on WKWebView).                                                                                                                                                                                                                                                                |
| Vibrancy                                         | **Not in the Hub** (Wispr Flow has none). Use `window-vibrancy`/Tauri `windowEffects` only on the **Scratchpad note window's rail** (`sidebar` material, `followsWindowActiveState`) if the rail is kept, and on the macOS **menus/popovers that are native** (tray, app menu: already native). Popovers inside the web view use solid `--surface` + `--elev-1`, not `backdrop-filter`. |
| `backdrop-filter`                                | Only for the dialog scrim on the onboarding pattern (`blur(2px)` + `--scrim`), measured in WebKit for frame time before shipping; off under Reduce Transparency (`prefers-reduced-transparency`).                                                                                                                                                                                       |
| Flow Bar                                         | Stays native at rest (`fork/pill/native.rs`): opaque black capsule with a 0.5 pt `rgba(255,255,255,0.12)` edge and a soft shadow (`NSShadow`, 0/2/8, 35 %). Resting size moves to 38 × 6 pt, hover glow drawn natively (a radial `CAGradientLayer` in `--accent-2`).                                                                                                                    |
| Haptics                                          | `NSHapticFeedbackManager` `.alignment` on drag snap of the bar (Force Touch trackpads only).                                                                                                                                                                                                                                                                                            |
| Reduce Motion / Transparency / Increase Contrast | Read in CSS media queries (already) and, for the native bar, `NSWorkspace.accessibilityDisplayShouldReduceMotion`.                                                                                                                                                                                                                                                                      |

## 5. Animation library and motion rules

**Library:** [`motion`](https://motion.dev) (the successor of Framer Motion, MIT), React API from `motion/react`, pinned to an exact 12.x version at install time. It gives springs, `AnimatePresence` for exit animations, layout animations (FLIP) and `useReducedMotion`. Supporting parts:

| Need                                       | Part                    | Licence |
| ------------------------------------------ | ----------------------- | ------- |
| Rolling numbers (stats, hours saved, wpm)  | `@number-flow/react`    | MIT     |
| Confetti (existing)                        | `canvas-confetti`       | ISC     |
| Coach marks (existing)                     | `driver.js`             | MIT     |
| Path drawing (hand-drawn arrow, underline) | `motion`'s `pathLength` | MIT     |

Rules (put in `src/fork/ui/motion.ts` as presets; components never hand-roll durations):

1. **Everything that appears, animates in; everything that leaves, animates out.** No element pops. Dialogs: scrim fade 220 ms + panel `gentle` spring from scale 0.97, y 8, opacity 0; exit 150 ms ease-in to scale 0.98, opacity 0.
2. **Hover is 140 ms colour/background only**; press is `scale(0.97)` for 90 ms on buttons and chips (snappy spring back). No hover lift on cards; hover reveals row actions with a 140 ms fade.
3. **Pages crossfade** (content panel only, sidebar stays): out 120 ms, in 220 ms with y 6 → 0. Onboarding steps slide 24 pt in the direction of travel with a 320 ms crossfade; Back reverses the direction.
4. **Stagger staged reveals** at 80 ms per item, max 6 items (lists render the first screenful staggered once, never on every update).
5. **Celebrations are choreographed**, not decorated: headline → supporting numbers (rolling) → drawn arrow (`pathLength` 0 → 1, 700 ms) → sticker pops (snappy spring from scale 0.6) → caption → button, total ≈ 2.5-3.3 s like the reference; confetti only once, after the numbers settle.
6. **The bar morphs, never cuts:** rest → active is one `bar` spring on width/height/corner radius from the bar's centre; the mic label enters with opacity + `blur(6px → 0)` over 220 ms, holds 1.8 s, leaves in 180 ms; the waveform shows a dotted idle line until the first audio frame.
7. **Live data animates at its own rate** (levels at 30 fps, no easing on top beyond the existing smoothing).
8. **Reduce Motion:** every preset swaps to an opacity-only crossfade of 120 ms; no scale, slide, blur, path drawing, number rolling or confetti; scenes stand still (today's rule stays).
9. **Budget:** animate only `transform`, `opacity`, `filter` (bar label only); no layout-thrashing properties; 60 fps in WebKit on an M1 checked with the Web Inspector timeline for each new screen.

## 6. Asset sourcing list (redistributable in a public GPL-3.0 repo)

Licence rule: an asset ships only if it is CC0, CC-BY-4.0 (FSF: compatible with GPLv3), OFL, MIT/ISC/Apache-2.0, or made by us/the owner and released under one of those. Unsplash and Pexels licences are **not** acceptable (custom licences that restrict redistribution as-is and compilation). Every asset gets a row in `docs/fork/LICENSE-REGISTER.md` with source URL, author, licence and a sha256.

| Asset                                                                                        | Need                                                 | Source                                                                                             | Licence                  | Notes                                                                                                                   |
| -------------------------------------------------------------------------------------------- | ---------------------------------------------------- | -------------------------------------------------------------------------------------------------- | ------------------------ | ----------------------------------------------------------------------------------------------------------------------- |
| UI and display fonts                                                                         | §2                                                   | Fontsource packages                                                                                | OFL-1.1                  | vendored, sha256-pinned                                                                                                 |
| Icons                                                                                        | §3                                                   | Lucide                                                                                             | ISC                      | already in NOTICE                                                                                                       |
| Hero banner photos (5-6: home, scratchpad, transforms, style, dictionary/snippets, insights) | dark, warm, shallow-focus desk/hands/notes/interiors | **owner's own photos** (best), else Wikimedia Commons / Openverse filtered to CC0                  | CC0 (owner releases his) | we grade, crop to 1600 × 400 @2x, blur the right half, add grain; WebP ≤ 120 KB each; light and dark use the same photo |
| Character illustrations (onboarding questions, empty states)                                 | flat, outlined people                                | **Open Peeps** (Pablo Stanley), **Humaaans**                                                       | CC0                      | recolour to our palette (pink/lilac/maroon family), export SVG                                                          |
| Doodles and stickers (celebration arrow, rocket, sparkles)                                   | hand-drawn                                           | **Open Doodles** (Pablo Stanley), or drawn by us                                                   | CC0                      | SVG, animatable paths                                                                                                   |
| Decorative pattern                                                                           | onboarding and dark Learn backgrounds                | made from **our logo curve**                                                                       | ours                     | SVG tile, 3-5 % contrast                                                                                                |
| Product mocks (composers, chat, email, notes, IDE)                                           | practice steps, lessons, chooser                     | our scenes engine                                                                                  | ours                     | generic and unbranded; no third-party logos                                                                             |
| App icons in banners                                                                         | Style/Transforms banners                             | read at runtime from installed apps (existing `fork/context/apps.rs`)                              | not shipped              | fallback: Lucide glyph in a circle                                                                                      |
| Gradient wallpapers                                                                          | chooser and lesson backdrops                         | CSS/SVG mesh gradient + `feTurbulence` grain we write                                              | ours                     |                                                                                                                         |
| Microphone illustrations (quiz grid, table)                                                  | 8 generic mic kinds                                  | draw SVG line art on our grid, or render CC0 models from **Poly Haven** / Sketchfab CC0 in Blender | CC0 / ours               | no brand marks on the products                                                                                          |
| People photos (whisper lesson)                                                               | someone speaking softly to a laptop                  | owner-shot photo or a short owner-shot clip; else Wikimedia CC0                                    | CC0                      | with a model release if a person is identifiable                                                                        |
| "How it works" videos                                                                        | Transforms, Whisper, Scratchpad                      | screen recordings of **Wispr Bro** (ScreenCaptureKit), WebM VP9 ≤ 2 MB each, captions as WebVTT    | ours                     | or keep the animated scenes                                                                                             |
| Sounds (later wave)                                                                          | start/stop/done ticks                                | recorded by us, or Freesound filtered to CC0                                                       | CC0                      | out of this rebuild                                                                                                     |

The asset agent (W7 below) produces a contact sheet of every candidate for the owner before anything lands.

## 7. Rebuild split

### Phase 0 (one agent, first, blocking): foundations

Branch `design/foundations`. Delivers, behind no flag (the old look is removed as pages migrate, see "migration"):

1. `tokens.css` v2 and Tailwind `@theme` mapping (§1), fonts (§2), `Icon` wrapper and the guard (§3), `motion` + presets in `src/fork/ui/motion.ts` (§5).
2. Primitives in `src/fork/ui/` (each with light/dark/RTL/Reduce Motion Playwright shots and axe): `Button` (primary-ink, secondary, ghost, cream, destructive; sizes 32/36/40), `IconButton`, `Kbd`, `Chip` (choice, removable), `Tag` (New, Beta), `Toggle` (switch), `Checkbox`, `Radio`, `Select` (Radix-based, replaces native selects), `TextField`, `SearchField`, `Tabs` (underline), `Card`, `ListContainer` + `ListRow` (hover actions slot), `SettingRow`, `SectionLabel`, `PageHeader`, `HeroBanner` (photo, gradient, serif headline, actions), `EmptyState` (illustration slot, one action), `Dialog`, `Sheet` (lesson size, progress dashes, back/close), `Popover`, `Menu`, `Tooltip`, `ProgressDashes`, `ProgressLine`, `Stepper`, `StatNumber` (number-flow), `Toast`.
3. The **Hub shell**: 46 pt title strip, inset rounded content panel, sidebar restyle (items, active fill, checklist card, footer), page transition.
4. A gallery page `#/fork/gallery` (debug builds only) and a Playwright spec that screenshots it, so later agents see every primitive.

Exit: gallery reviewed by the owner; `hub.spec.ts` AA pairs pass with the new palette.

**As built (2026-11-03, `design/foundations`).** Delivered as above with these calls (full list in R5, after S-13's "As built"):

- Token names carry the `--wb-` prefix (`--wb-chrome`, `--wb-panel`, `--wb-surface`…): Handy already owns `--font-*`, `--radius-*`, `--text-*` and `--color-line`. v1 names are aliases of the v2 roles until the Phase 1 sweep. No Tailwind `@theme` hook (it would edit Handy's `App.css`); use `bg-(--wb-surface)` and the `type-*` classes (`type-title`, `type-serif-title`, `type-micro`…).
- Changed values: `--wb-ink-3` `#666666` / `#9C9A94` (AA on the secondary fill); `--wb-line-strong` stays a 3:1 control edge (`#8F8B82` / `#7D786E`) and the plan's `#D9D7D0` is `--wb-line-2`. New roles: `--wb-sheet`, `--wb-secondary-hover`, `--wb-ink-hover`, `--wb-ink-disabled`, `--wb-danger-soft`, `--wb-hero`, `--wb-on-hero(-2)`, `--wb-hero-grade`, `--wb-on-hud(-2)`.
- Springs as tuned (no overshoot on large surfaces): `snappy` 520/42, `soft` 380/38, `gentle` 300/34, `bar` 600/38/0.8, `pop` 460/22 (stickers only).
- Primitives not built in Phase 0: `Chip`, `Radio`, `Toast` (sonner stays), `SettingRow` and `Popover`-based `Menu` variants beyond the existing ones; `Tabs`, `Menu`, `Popover` were restyled, not rebuilt. W3 builds `SettingRow` from `Card tone="tint"` + `Switch`/`Select`; a shared `Chip` goes to the foundations owner when W1/W5 need it.
- **Phase 0b (2026-11-03, `design/foundations-2`)** added the missing pieces: `Chip`/`ChipGroup`, `RadioGroup`/`Radio`/`OptionCard`, `SettingRow`/`SettingGroup`, `ToastView` (sonner kept and restyled through the tokens), `Wordmark`/`BrandGlyph` (in the sidebar), and `ThemeScope` (portalled popups keep a pinned theme). The v1 aliases are gone and `scripts/wb-guard-tokens.ts` fails on them.
- Onboarding keeps the overlay title bar (the traffic-light inset applies to the whole window).
- Gallery route: `#/fork/gallery`.

### Phase 1 (6 agents in parallel, each in its own worktree, each owning its folders)

| #   | Area                              | Owns                                                                                                                                                                                                                          | Reference frames (local only)                               | Notes                                                                                                                                                                                                                                  |
| --- | --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| W1  | **Onboarding**                    | `src/fork/onboarding/*` (not scenes), `fork-onboarding` strings                                                                                                                                                               | o-0045 … o-0332                                             | split and centred layouts, five-stage stepper + progress line, pattern background, test cards, black CTAs, celebration choreography, hours-saved rolling number, typing test in mono                                                   |
| W2  | **Hub pages**                     | `home/`, `history/`, `insights/`, `dictionary/`, `snippets/`, `style/`, `transforms/` (page only), `share/`                                                                                                                   | u-073, u-075, u-080, cap-024                                | hero banners with photos, history list container, stat rail, Insights card grid with data palette, Style cards, Transforms cards + kbd chips, empty states with art                                                                    |
| W3  | **Settings**                      | `settings/*`, `hub/HandySettingsPage.tsx`, `settings/hotkeys/*`                                                                                                                                                               | u-033, o-0178                                               | modal 960 × 640, nav with icons and group labels, serif section titles, tinted setting-row cards, Radix selects, hotkeys panel restyle, Handy-hosted sections reskinned through the token retune                                       |
| W4  | **Bar and spotlight**             | `src/fork/pill/*`, `src/fork/spotlight/*`, `src-tauri/src/fork/pill/{geometry,native}.rs`                                                                                                                                     | 02-bar-click-\*, 03-bar-hover, bars of o-0200, u-065, u-067 | new geometry (rest 38 × 6, held 72 × 30, locked 100 × 30, label 34 tall), morph springs, blur-in label, dotted idle waveform, ✓ white / ✕ dark circles, card and diff card restyle, spotlight cut-out; Rust geometry tests regenerated |
| W5  | **Lessons and first use**         | `src/fork/tutorials/*`, `onboarding/FirstUseChooser.tsx`, `onboarding/scenes/*`                                                                                                                                               | cap-183 … cap-213, u-002 … u-026, o-0340                    | `Sheet` with progress dashes, serif headings with italic accent, option cards, mic grid art, recommendation table, practice composers restyled, chooser with gradient backdrop                                                         |
| W6  | **Scratchpad and dialogs**        | `scratchpad/*`, `hub/{NotificationsPanel,HelpPopover,FeaturePopup,ReportProblemDialog}.tsx`, `dictionary/{AddWordDialog,ImportWizard}.tsx`, `snippets/{SnippetDialog,ImportWizard}.tsx`, `ui/ImportWizard.tsx`, `whats-new/*` | cap-001, cap-231, o-0178                                    | note window (tabs, rail, copy pill), Recents cards, every dialog on `Dialog`, notifications panel and help popover on `Popover`, What's new as a sheet                                                                                 |
| W7  | **Assets** (can start in Phase 0) | `src/assets/fork/art/*`, `LICENSE-REGISTER.md`, `NOTICE`                                                                                                                                                                      | §6                                                          | sources, licenses, grades and exports every asset; delivers a contact sheet; no UI code                                                                                                                                                |

Rules for Phase 1:

- Primitives are frozen after Phase 0. A missing variant goes to the foundations owner as a small PR; area agents do not fork a primitive locally.
- Copy changes ride along only where density demands (REFERENCE-SPEC §9: ≤ 20 words per onboarding screen, ≤ 12 per settings description); new strings follow the translation protocol in CONTEXT.md (strict i18n is on).
- Each area's gate: its existing Playwright specs green, new screenshot pairs (ours vs. the same reference frames, kept local) reviewed in the comparison page, axe light/dark/Increase Contrast, RTL, Reduce Motion, keyboard-only, and a WebKit frame-time check on its heaviest animation.
- Migration: each area deletes the old styling it replaces; after Phase 1 a sweep removes the v1 tokens (`--wb-banner`, indigo, `--wb-radius-*`) and fails CI if any remain.

### Phase 2 (one agent): polish pass

Cross-app consistency review against the comparison page, motion tuning on the real Mac (WebKit, 60 Hz and 120 Hz), the native bar on a real display, a dark-mode pass on every screen, and the owner's review.
