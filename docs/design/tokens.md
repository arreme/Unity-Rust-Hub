# Design tokens

All visual primitives live in `crates/unity-hub-ui/src/tokens.rs`. Change
values in that one file where possible.

## Palette

Clean graphite: grey surfaces one step apart, hairline borders, and white as
the only accent. Status colors are muted and used for small marks (dots,
chips, icons), never for large fills.

| Token | Current value | Role |
|---|---|---|
| `palette.background` | `#1f2225` | Page background |
| `palette.panel` | `#26292d` | Card surface |
| `palette.panel_raised` | `#2f3337` | Tiles, chips, raised blocks inside cards, active nav |
| `palette.sunken` | `#1f2225` | Inputs, logs, previews |
| `palette.border` | `#34383d` | Hairlines and card borders |
| `palette.border_strong` | `#43484e` | Control borders |
| `palette.text` | `#f1f2f3` | Primary text |
| `palette.muted` | `#b4b8bd` | Secondary text |
| `palette.faint` | `#8e9399` | Labels, captions, metadata |
| `palette.accent` | `#f1f2f3` | Primary button fill, checked boxes, progress |
| `palette.accent_hover` / `accent_pressed` | `#ffffff` / `#d6d8db` | Primary button states |
| `palette.on_accent` | `#1a1c1f` | Text on the accent fill |
| `palette.success` | `#86cfa6` | Success/positive |
| `palette.warning` | `#e6c27c` | Warning/caution |
| `palette.danger` | `#ec8f97` | Error/destructive |
| `palette.info` | `#94bce8` | Informational |
| `palette.highlight` | `#bcaeea` | Highlight/pipeline |

Other surfaces: `sidebar_background` (`#1a1c1f`), `backdrop` (dialog screen,
`#16181a`), and `hover` / `pressed` (white washes for ghost and nav controls).

## Semantic tones

The contract carries semantic tones, not colors. The UI maps them in
`theme::tone_color`:

| Contract tone | Token |
|---|---|
| `Neutral` | `muted` (grey chip) |
| `Accent` | `text` (grey chip) |
| `Success` | `success` |
| `Warning` | `warning` |
| `Danger` | `danger` |
| `Info` | `info` |
| `Highlight` | `highlight` |

## Spacing, radii, type

- `spacing`: page padding (`PAGE_X`, `PAGE_TOP`), section gap, card padding and
  gap, list-row padding, small, xs.
- `radius`: card 16, modal 20, control 12, tile 12, notice 14, nav 10, and
  `PILL` for buttons, chips, and segmented controls.
- `font`: caption 11 (uppercase labels), small 12, body 13, body large 14,
  card title 16, row title 18, modal title 20, stat 26, page title 28,
  display 36, plus button, badge, mono, and icon sizes.

## Layout

- No top bar: every page starts with `components::page_header` (title,
  subtitle, optional actions such as `refresh_button`).
- Lists are rows separated by hairlines (`components::divided`, `list_row`)
  inside one card, not boxed rows.
- Summary numbers use `components::stat_strip`; settings use `group` and
  `setting_row`.

## Controls

Every interactive widget is styled in `theme.rs`; pages use the helpers in
`components.rs` rather than iced's built-in styles:

- `components::button` / `icon_button` with `ButtonKind::{Primary, Secondary,
  Ghost, Danger}`: white pill, outlined pill, text pill, danger outline. Use at
  most one primary action per card or dialog. Pass `components::COMPACT`
  padding inside list rows. `components::icon_only` is the square icon action
  used by table rows (favorite star, remove).
- `components::segmented` for small exclusive choices.
- `components::text_input`, `search_input`, `checkbox`, `scroll`.
- Pick lists take `theme::pick_list`, `theme::menu`,
  `icons::pick_list_handle`, and `components::PICK_LIST_PADDING`.
- Status helpers: `notice`, `error_text`, `loading`, `hint`, `badge`,
  `icon_badge`, `status_dot`, `status_label`, `empty`, `icon_tile`,
  `initial_tile`.

## Text scaling

The Settings screen persists a text scale (100/110/125/150%). The UI applies it
through `ui::text` and `ui::font_size`. New components should use those helpers
for any explicit text size so accessibility scaling keeps working. Icons are
text glyphs, so they scale with it; fixed widths that hold text use
`ui::scaled_pixels`.

## Fonts and icons

The Settings screen also persists one font family from the fonts installed on
the PC (default `Tahoma`). `ui::text` puts every regular label in that family,
and `fonts::regular` / `medium` / `semibold` only change the weight, so selecting
a nav item, button, or row never swaps typefaces. Paths and versions use the
same family through `fonts::mono`.

The UI still bundles Inter (Regular, Medium, SemiBold) as the iced startup
fallback and the Lucide icon font under `crates/unity-hub-ui/assets/fonts/`,
with their licenses next to them. `fonts::ALL` is loaded by both the app and the
gallery, and `fonts::ICONS` stays fixed for glyphs.

To add an icon, add its Lucide name to `ICONS` in `scripts/subset-fonts.sh`,
run the script, and add the codepoint constant to `icons.rs`.
