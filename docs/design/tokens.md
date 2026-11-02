# Design tokens

All visual primitives live in `crates/unity-hub-ui/src/tokens.rs`. When the
Figma file arrives, map its variables onto these names and change values in that
one file where possible.

## Palette

| Token | Current value | Figma role |
|---|---|---|
| `palette.background` | `#0b0e13` | App/window background |
| `palette.panel` | `#141922` | Card surface |
| `palette.panel_raised` | `#1a202b` | Hero/modal raised surface |
| `palette.border` | `#242c39` | Default border |
| `palette.border_strong` | `#313c4c` | Modal/strong border |
| `palette.text` | `#e8edf4` | Primary text |
| `palette.muted` | `#8e9aac` | Secondary text |
| `palette.faint` | `#667286` | Tertiary text and metadata |
| `palette.accent` | `#f0a35e` | Brand/accent |
| `palette.success` | `#5fd39a` | Success/positive |
| `palette.warning` | `#f4c566` | Warning/caution |
| `palette.danger` | `#ff7a85` | Error/destructive |
| `palette.info` | `#73b7ff` | Informational |
| `palette.highlight` | `#b79cff` | Highlight/pipeline |

Surfaces that are not simple palette roles:

- `sidebar_background`
- `topbar_background`
- `hero_border`
- `row_header_background`
- `job_background`
- `job_border`

## Semantic tones

The contract carries semantic tones, not colors. The UI maps them in
`theme::tone_color`:

| Contract tone | Token |
|---|---|
| `Neutral` | `muted` |
| `Accent` | `accent` |
| `Success` | `success` |
| `Warning` | `warning` |
| `Danger` | `danger` |
| `Info` | `info` |
| `Highlight` | `highlight` |

A designer can change the mapping without touching controller code.

## Spacing, radii, type

- `space`: page/content/card/row gaps.
- `radius`: card, hero, modal, row, notice, badge, nav.
- `font`: eyebrow, page title, subtitle, card/section title, body, small, badge,
  monospace.

The current pages still contain some one-off sizes for dense table cells. When
implementing the new Figma design, prefer moving those values into `font` and
`space` rather than adding new literals.

## Text scaling

The Settings screen persists a text scale (100/110/125/150%). The UI applies it
through `ui::text` and `ui::font_size`. New components should use those helpers
for any explicit text size so accessibility scaling keeps working.

## Fonts and assets

The current build uses the system font. If the Figma design specifies a custom
font, add the font files under `crates/unity-hub-ui/assets/fonts/` and load them
in `crates/unity-hub/src/main.rs` before building the application. Keep the
license file next to the font.
