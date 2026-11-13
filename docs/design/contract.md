# Unity Rust Hub — design/controller contract

This document is the handoff surface for a visual redesign. The designer supplies
Figma screens and tokens; implementation happens in `unity-hub-ui` against the
view models in `unity-hub-model`.

## Layer map

```
unity-core              CLI adapter: commands, envelopes, DTOs, errors
        ↑
unity-hub-controller    state, actions, jobs, settings, domain → view-model mapping
        ↓
unity-hub-model         pure contract: view models + UiAction
        ↑
unity-hub-ui            design tokens, components, shell, pages, modals, gallery
        ↑
unity-hub               binary: iced application, window/icon, runtime wiring
```

Dependency rules, enforced by Cargo:

- `unity-hub-ui` depends only on `unity-hub-model` and iced. It must never
  depend on `unity-core`, `unity-hub-controller`, or a CLI error type.
- `unity-hub-controller` contains no widget or screen code. It may use iced
  `Task`/`Subscription` to schedule work, but it does not build `Element`s.
- `unity-hub` has no business logic. It boots the controller, caches the view
  model, maps `UiAction` to a controller `Message`, and calls the UI entry
  point.
- `unity-hub-model` has no dependencies. It is the stable boundary.

## How a screen is wired

1. Controller state changes in `unity-hub-controller/src/app.rs`.
2. `App::view_model()` maps domain/CLI data into `unity_hub_model::ViewModel`.
3. The binary stores that snapshot and passes `&ViewModel` to
   `unity_hub_ui::view`.
4. The UI draws the snapshot and emits `UiAction`.
5. The binary converts the action with `Message::from(action)` and calls
   `App::update`.

The UI never reads `Loadable`, `UnityError`, `UnityCli`, `JobManager`, or
controller fields.

## Contract files

- View models: `crates/unity-hub-model/src/view.rs`
- User actions: `crates/unity-hub-model/src/action.rs`
- UI entry point: `crates/unity-hub-ui/src/lib.rs`
- Placeholder content for every screen: `crates/unity-hub-ui/src/fixtures.rs`
- Design primitives: `crates/unity-hub-ui/src/tokens.rs`

## Screen and state inventory

The redesign needs a visual treatment for each state below. The gallery covers
every screen plus representative loading, error, modal, and job states; add a
fixture when a new state needs review.

### Shell

- Sidebar navigation (5 destinations), active state.
- Sidebar footer: CLI status (checking / connected with version / missing).
- Page header: title, subtitle, and actions (Refresh on Overview, Editors,
  Projects).
- Notice banner: info, success, error.
- Job card: running with progress, succeeded, failed with error detail,
  cancelled, with collapsible-looking log area.
- Minimum window 1040×680; default 1280×840.

### Overview

- Hero.
- CLI card: resolving, missing CLI with install callout, installed details,
  update available, update-check failure, CLI read error, re-check/settings
  actions.
- Account card: checking sign-in, signed in, signed out, expired session,
  sign-in error; license active/inactive, no licenses, license read error.
- Running Editors: loading, none, one/many, read error.

### Editors

- Installed Editors: loading, error, empty, one/many, default badge, long module
  list.
- Install a new Editor: version input, module catalog loading/error/empty/loaded,
  select all/clear, EULA, preview loading/ready/raw fallback, install enabled
  and disabled.
- Available releases: loading, error, empty, installed badge, use action,
  stream tones.

### Projects

- Search field and count, plus an Add project action that opens the native
  folder picker and registers the chosen folder with `unity projects add`.
- Loading, error, no projects registered, no matching projects, populated table.
- Badges: Unity Cloud, VCS, Editor not installed, local project; favorites are
  the star toggle beside the title (CLI pin/unpin), not a badge.
- Pipeline badge or empty, long title, long path, modified date.
- Row actions: favorite star, Open, Change version, Remove (confirmation modal;
  files on disk are untouched).

### Changelog

- Not loaded, loading, error, no data.
- Header with version/source/entry count.
- No breaking changes panel; breaking changes list with section/reason badges.
- Section cards for Removed/Changed/Added/Fixed/Security/Other.
- Long section with truncation note.
- Raw Markdown toggle: idle, loading, loaded, error.

### Settings

- Resolved CLI executable/source, path input, save/clear, re-check.
- Config directory and settings file.
- Font picker fed by the installed PC font families (default Tahoma).
- Text-scale picker with preview.
- About/engine/version badges.

### Modals

- Install CLI: channel picker, installer command preview, consent checkbox,
  disabled/enabled run action.
- Editor modules: installed module list, available module loading/error/empty/
  loaded, selection count, EULA, disabled/enabled install.
- Project upgrade: current version/path, target picker, no target installed
  warning, confirmation, disabled/enabled upgrade.
- Remove project: path, "files on disk are not deleted" warning, cancel/remove.

## Platform constraints to design around

- Dialogs replace the shell as full-screen views. Do not design stacked overlays
  above the scrollable shell: iced 0.14 with the tiny-skia fallback miscomposites
  them.
- Tables are built from header/row containers, not `iced::widget::table`; the
  table widget stalled rendering with live data.
- Text scaling is a user preference. Components must tolerate 100–150% text
  without clipping critical actions.
- Changelog Markdown is displayed as plain text.
- Windows: the release binary is a GUI-subsystem app with no console, so every
  CLI child must be spawned through `unity_core::process::hidden`. Otherwise
  Windows creates a console window per invocation, and a page's read batch
  flashes a terminal on each tab open/switch.
- Linux: CLI commands read the stored sign-in through the CLI's resident auth
  broker; without it each command reads the Secret Service keyring itself and a
  locked keyring asks to unlock once per command. The app runs
  `unity auth broker ensure` before its first credential read and before
  `unity open`. `UNITY_NO_AUTH_BROKER` disables that sharing, so it is surfaced
  as a notice.

## Design gallery

```bash
cargo run -p unity-hub-ui --bin unity-hub-design-gallery
```

The gallery renders every fixture without a Unity CLI and is the fastest way to
review loading/error/modal/job states against Figma. `cargo test -p unity-hub-ui`
also constructs every fixture, so a broken screen fails CI without opening a
window.

Environment variables for review:

- `GALLERY_FIXTURE=<index>` opens a specific fixture.
- `GALLERY_FONT=Tahoma` renders every fixture with a system font family.
- `GALLERY_TEXT_SCALE=1.5` renders every fixture at a larger text size.
- `GALLERY_HEIGHT=<px>` opens a taller window.
- `GALLERY_SCREENSHOTS=<dir>` captures every fixture from inside the gallery
  window as raw RGBA (`NN-WxH.rgba`) and exits. Convert one with
  `magick -size WxH -depth 8 rgba:<file> out.png`.
