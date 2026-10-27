# Fluss design language

Black and gold. A precision instrument, not a SaaS dashboard. References: the Deus Ex HUD
(amber on black, chamfered plates, thin rules, tabular readouts), the dial of a good mechanical
watch, a laboratory oscilloscope. Quiet until it is working, then exact and responsive.

**Never:** gradients (no linear/radial fills, no gradient text, no glows made of gradients), blue/purple,
glassmorphism, blurred blobs, emoji, rounded "bubbly" cards for everything, drop-shadow stacks,
generic level-meter bars, stock illustration.

## Colour (dark only; the app commits to one look)

| token | value | use |
|---|---|---|
| `--void` | `#000000` | window background |
| `--plate` | `#0B0A08` | raised areas (sidebar, popup plate) — barely lifted from black |
| `--rule` | `#2A2418` | 1px hairlines, dividers, inactive outlines |
| `--rule-strong` | `#4A3F27` | hovered outlines, scale ticks |
| `--gold` | `#E3B04B` | the accent: active state, focus, key numbers, the voice visual |
| `--gold-dim` | `#8C6B2A` | secondary accent, inactive ticks, borders of active elements |
| `--gold-ink` | `#1A1408` | text on solid gold |
| `--text` | `#EDE6D6` | primary text (warm white) |
| `--text-2` | `#9A9284` | secondary text |
| `--text-3` | `#5E584D` | hints, disabled |
| `--danger` | `#E5484D` | errors only |

Gold is used sparingly: one gold element should be the obvious focus of any view.
Solid fills only. Translucency only as flat alpha of the same colours (e.g. gold at 12% for a selected row).

## Type

- **Licht Serif** (Light 300 / Regular 400) — display: page titles, hero statements, big numbers that are
  words. Large and light (32–64px), tight leading (1.0–1.1), slight negative tracking (-0.01em).
  Licht may not be redistributed (zon.sh/licht LICENSE §3): it is loaded from `fonts/licht/` (git-ignored,
  present on dev machines) and falls back to **Fraunces** (OFL, bundled) → Georgia.
  `font-family: "Licht Serif", "Fraunces", Georgia, serif;`
- **Geist** (400/500) — all UI and body text, 13–15px, leading 1.45.
- **Geist Mono** (400/500) — labels, values, keys, numbers, statuses, IDs, paths.
  Section labels: mono, 11px, uppercase, letter-spacing 0.12em, `--text-3`, often prefixed with an index
  (`01 — MODEL`). Numbers always mono with tabular figures.

## Geometry

- Chamfered corners instead of radii: `clip-path: polygon(...)` cutting 6–10px at two opposite corners
  (top-left + bottom-right) for plates and primary buttons. Small controls: square corners (radius 0–2px).
- 1px hairlines (`--rule`) separate regions; structure comes from alignment and rules, not boxes.
  Use cards only where an object is truly a discrete thing (e.g. a model choice).
- HUD details, sparingly: corner brackets around the focused element, tick marks on sliders,
  index numbers, `[ ]` around keyboard keys in mono.
- Strict grid: 8px base. Content column aligned to one left edge; label column fixed width.

## Motion

Fast and snappy with a small touch of smoothness. Never floaty.
- Durations: 90ms (hover/press), 140ms (state change, page switch), 220ms max (overlay/popup entrance).
- Easing: `cubic-bezier(0.2, 0.9, 0.1, 1)` (quick out). No springs/bounce, no stagger longer than 20ms/item.
- Page switch: content fades in with a 4px rise, 140ms. Hover: colour/outline change only.
- Respect reduced motion (instant).

## The voice visual ("aperture")

The signature element, used by the overlay, tray icon, app icon, home screen, onboarding and website —
never generic bars. A six-blade camera iris: a faint gold blade disc, blade edges in `--gold-dim`, a
`--gold` hexagonal opening, a rim with 24 lens markings, and the gold diamond (◆) visible through the
pinhole when it is nearly closed. The opening twists as it opens (rotation += 1.3 rad × openness).
- idle: nearly closed (13% of the radius), opening outlined in `--gold-dim`, slow drift (0.2 rad/s)
- listening: opening = 13% + 72% × voice level, smoothed (attack fast, release ~140ms); outline `--gold`
- hands-free: the rim turns `--gold`, dashed and turning
- transcribing / loading: blades spin (4.2 rad/s) around a small pulsing opening; loading also opens
  with progress and draws a `--gold` progress arc on the rim
- notice ok/err: opening held at 34% / closed with a `--danger` diamond and outline
- tray: static apertures per state (closed dim = off, pinhole + diamond = ready, half = busy,
  open with gold rim = recording, red core = problem)

## Voice & copy

Short, plain, confident. Sentence case for UI text; mono uppercase only for labels.
No exclamation marks, no "magic", no "AI-powered". Say what happens: "Hold Ctrl+Win and speak."
