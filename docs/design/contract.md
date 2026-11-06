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
- Advisory banners (above the notice), sourced from the CLI's `notifications[]`
  and `warnings[]` on any read or streamed job result frame:
  - CLI update available: neutral title, CLI message, `current → latest`, the
    exact `unity self-update` command, Copy, and a Run button (click only).
  - Plugin update available: same shape with `plugin · current → latest` and a
    copy-only command until the Plugins page supplies a builder.
  - Plugin beta / deprecated-flag warning: warning tone, CLI text verbatim, no
    invented action.
  - Unknown notification code or object-shaped warning: neutral title, CLI
    message verbatim.
  - Dismissed: hidden for the current CLI version (an info notice confirms).
  - Every banner is dismissible and never blocks the screen.
- Notice banner: info, success, error.
- Job card: running with progress, succeeded, failed with the shared error
  rendering (stable code, CLI message, detail, and a non-retry action when the
  mapping has one), cancelled, with collapsible-looking log area.
- Error rendering (`components::error_text`): the stable CLI code as a danger
  badge, the CLI's message verbatim, the CLI detail plus mapping guidance, and
  one primary action. Unknown codes render code/message/detail with no action.
  Used by failed reads on every page and by failed jobs.
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
  select all/clear, headless option on Linux with the CLI's availability note,
  EULA, preview loading/ready/raw fallback, install enabled and disabled.
- Available releases: loading, error, empty, installed badge, use action,
  stream tones.

### Projects

- Search field and count.
- Loading, error, no projects registered, no matching projects, populated table.
- Badges: Unity Cloud, VCS, favorite, Editor not installed, local project.
- Pipeline badge or empty, long title, long path, modified date.
- Row actions: Open, Change version.

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
- Text-scale picker with preview.
- About/engine/version badges.

### Modals

- Install CLI: channel picker, installer command preview, consent checkbox,
  disabled/enabled run action.
- Editor modules: installed module list for one build (version + architecture),
  available module loading/error/empty/loaded, selection count, EULA,
  disabled/enabled install.
- Project upgrade: current version/path, target picker, no target installed
  warning, confirmation, disabled/enabled upgrade.

## Advisory contract

`ShellVm::advisories: Vec<AdvisoryVm>` is the single surface every CLI advisory
uses; later pages (#7 install resilience, #9 plugins) reuse it instead of
drawing their own banners.

- `AdvisoryVm { id, tone, title, message, detail, command, run_label,
  requires_approval, dismissible }`.
- `message` is always the CLI's own (localized) text. The controller derives
  `title` and `detail` only for documented codes (`CLI_UPDATE_AVAILABLE`,
  `PLUGIN_UPDATE_AVAILABLE`, `FEX_EMU_EXPERIMENTAL`,
  `AGENT_PLUGIN_AVAILABLE`); unknown codes and warnings keep the CLI text and a
  neutral title.
- `id` is the adapter's stable identity: origin + `code` + the documented
  stable `data` fields (update/plugin versions, plugin id). Plain warnings use
  their text. The controller dedupes on it, so a polled refresh cannot stack
  identical banners.
- Dismissal is persisted in `settings.json` as
  `dismissedAdvisories: { "<cli version>": ["<id>", …] }`. A CLI version change
  drops the stored list and the old dismissals, so the new version shows its
  advisory once.
- `UiAction::DismissAdvisory(id)`, `UiAction::CopyAdvisoryCommand(id)` (clipboard
  only), and `UiAction::RunAdvisoryRemediation(id)`. Run exists only when the
  adapter has an explicit command builder; otherwise the command is
  visible/copyable and no button is drawn.

## Error contract

Every failed read and failed job reaches the UI as `UiError`:

- `code: String` — the CLI's stable machine code, passed through unchanged.
  Adapter failures use `CLI_NOT_FOUND`, `SPAWN_FAILED`, `UNEXPECTED_OUTPUT`,
  `INVALID_INPUT`, or `INTERNAL`.
- `message: String` — the CLI's own (localized) text, never rewritten.
- `detail: Option<String>` — the CLI's `detail` plus a code-specific guidance
  paragraph appended by the controller.
- `action: Option<ErrorActionVm>` where
  `ErrorActionVm { label, command, kind }`:
  - `label` is the button text.
  - `command` is the exact command the CLI named as `remediation`, when it
    named one. It is displayed and copied only; the app never executes a
    command from an error.
  - `kind` is one of `Retry(id)`, `SignIn`, `OpenSettings`, `PickVersion`,
    `OpenEditor`, `EnterToken`, `InstallGit`, `ShowCommand`, `Confirm`,
    `ShowBusy`, `AnswerDialog`, `FixPermissions`.

Resolution order in `unity-core::errors::remediation_for`:

1. The CLI's own `remediation` field (`UnityError.extra["remediation"]`), as a
   copy-only command (`ShowCommand`).
2. The stable `code` table: auth codes → Sign in; `NETWORK_UNREACHABLE` and
   `VCS_PROVIDER_UNREACHABLE` → Retry; `INSTALL_LOCKED`, `TIMEOUT`,
   `EDITOR_TIMEOUT` → Retry; `RELEASE_NOT_FOUND` → Pick another version;
   `EDITOR_UNREACHABLE` → Open a project; `UNAVAILABLE` / `STATUS_SAFE_MODE` →
   Show status; `AWAITING_INPUT` → Answer the Editor dialog;
   `VCS_TOKEN_REQUIRED` → Enter a token (env var or `--git-token-from-stdin`,
   explicitly not a Unity sign-in); `GIT_START_FAILED` → Install git;
   `LOCAL_STORE_UNWRITABLE` → Fix permissions;
   `LINK_VCS_DELETE_REMOTE_REQUIRES_YES` → Confirm explicitly;
   `INSTALL_HEADLESS_UNAVAILABLE` → Pick another version or clear headless;
   `BAD_ARGS` → Show the command.
3. Exit-code semantics for unknown codes (`2`, `3`, `7`, `9`, `11`, `12`, `14`,
   `17`, `18`). Exit codes outside that table stay generic.

Retry is a controller-side target, not a closure in the view model:

- The controller keeps `retry_targets` keyed by the stable read id carried in
  `ErrorActionKind::Retry("read:…")`. Parameterised reads (`modules`,
  `dialog-modules`, `preview-install`) keep the request they started with, so
  Retry repeats exactly what the user saw fail.
- No target means no Retry button; a successful retry clears the target.
- A job that already started a mutation is never offered Retry. Every job in
  this app (CLI install/update, Editor/module install, sign-in, project upgrade)
  is in that category. Jobs can still show non-retry actions (for example Enter
  a token) and always show their code.

Network errors name the proxy the CLI will use. The app has no proxy setting of
its own; the controller reports `UNITY_PROXY` (or `HTTPS_PROXY`, `HTTP_PROXY`,
`ALL_PROXY`) from the environment with credentials redacted. With no configured
proxy, the hint says the app has none and names the CLI's env vars and persisted
`unity config proxy` setting instead.

Gallery fixtures: `Error · actionable (retry)` and `Error · unknown code`, plus
the failed job fixture that has no Retry.

## Platform constraints to design around

- Dialogs replace the shell as full-screen views. Do not design stacked overlays
  above the scrollable shell: iced 0.14 with the tiny-skia backend miscomposites
  them.
- Tables are built from header/row containers, not `iced::widget::table`; the
  table widget stalled rendering with live data.
- Text scaling is a user preference. Components must tolerate 100–150% text
  without clipping critical actions.
- Changelog Markdown is displayed as plain text.

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
- `GALLERY_TEXT_SCALE=1.5` renders every fixture at a larger text size.
- `GALLERY_HEIGHT=<px>` opens a taller window.
- `GALLERY_SCREENSHOTS=<dir>` captures every fixture from inside the gallery
  window as raw RGBA (`NN-WxH.rgba`) and exits. Convert one with
  `magick -size WxH -depth 8 rgba:<file> out.png`.
