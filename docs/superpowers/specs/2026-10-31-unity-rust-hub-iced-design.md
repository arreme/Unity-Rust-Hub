# Unity Rust Hub — iced design spec

Date: 2026-10-31
Branch: `feat/iced-native-ui`
Status: implemented

## Goal

Replace the previous webview-based shell with a fully native Rust desktop application built on
[iced 0.14](https://iced.rs). The Unity CLI adapter (`crates/unity-core`) stays the
single source of truth for flags, envelopes, validation, and job specs. The rewrite
changes only the presentation and process-driving layer.

Success means the app starts on a Linux desktop with no webview, no Node toolchain,
and no WebKitGTK dependency, while keeping the existing feature set:

- detect, install, update, and inspect the Unity CLI;
- list installed Editors and their modules;
- load release notes, module catalogs, and install a new Editor;
- add modules to an installed Editor;
- list Hub-registered projects and change their Editor version;
- show the CLI changelog with possible breaking changes;
- persist an optional CLI path override.

## Non-goals

- Editor or module uninstall.
- Project creation or batch migration.
- Background services, tray integration, or auto-update of the GUI itself.
- Bundling the Unity CLI.
- A remote changelog editor or account management.

## Why iced

| Requirement | iced result |
|---|---|
| Native, no webview | winit window plus a Rust renderer; no HTML/JS runtime |
| Small host dependency set | Linux needs X11/Wayland client libraries, not WebKitGTK |
| Elm-style state flow | `boot` / `update` / `view` maps directly onto the existing job model |
| Single binary | `cargo build --release` produces one executable |
| Cross-platform | winit and iced run on Linux, macOS, and Windows |

The chosen build uses iced with `default-features = false` and:

- `tiny-skia` — software rendering; avoids requiring a working Vulkan/GL driver;
- `crisp` — sharper text and pixel snapping;
- `image` — window icon decoding;
- `thread-pool` — executor for futures;
- `x11`, `wayland` — Linux window backends;
- `linux-theme-detection` — keeps native font/theme behavior reasonable.

`wgpu` is deliberately disabled. The app is mostly panels and text; tiny-skia keeps
the dependency tree and driver requirements smaller. A GPU build can be restored by
swapping the feature list if profiling ever demands it.

## Workspace layout

```
crates/unity-core/     UI-agnostic CLI adapter
  bootstrap.rs         official installer command
  changelog.rs         changelog parser, breaking heuristics, provider trait, ChangelogView
  envelope.rs          JSON envelope types
  errors.rs            structured UnityError
  models.rs            DTOs (CliStatus, Editor, Project, InstallPreview, …)
  runner.rs            resolution, command specs, read methods, spec builders
  versions.rs          version/alias validation

crates/unity-hub/      iced application
  src/main.rs          iced application builder, window settings, icon
  src/app.rs           state, messages, update loop, shell, modals, job polling
  src/jobs.rs          child processes, NDJSON parsing, cancellation, event queue
  src/settings.rs      settings.json load/save
  src/theme.rs         palette, widget styles, badges
  src/util.rs          async bridge, formatting, error text
  src/views/           overview, editors, projects, changelog, settings
  assets/icon.png      window icon

docs/superpowers/      this spec and the implementation plan
```

Dependency direction is one-way: `unity-hub` depends on `unity-core`; core knows
nothing about iced or any UI toolkit.

## Runtime architecture

### Application loop

`main` builds an `iced::application` with `App::boot`, `App::update`, and `App::view`.
The window is 1280×840, minimum 1040×680, centered, with the bundled icon. The theme
is a custom dark palette.

`App` holds:

- a cloned `UnityCli` adapter (resolution is cheap; the optional override path lives
  in an `Arc`);
- a `JobManager` for child processes;
- `Loadable<T>` wrappers for each remote dataset (`data`, `loading`, `error`);
- page-local form state (install version, selected modules, EULA flags, search text,
  upgrade target, changelog target);
- the active job view model, notice banner, modal enum, and persisted settings.

All mutations go through messages. `update` returns `Task<Message>`; no widget owns
business state.

### Blocking CLI work

`unity-core` is synchronous by design. Every read is wrapped by
`util::run_blocking`, which moves the closure to a dedicated OS thread and returns a
`futures::oneshot` future. The iced executor awaits that future with `Task::perform`,
so the UI thread never blocks on process spawning or JSON parsing.

### Jobs and streaming

`JobManager::start`:

1. builds a `std::process::Command` from a core `CommandSpec` (program, args, env,
   cwd, stdin null);
2. pipes stdout and stderr;
3. registers the child and a cancellation flag;
4. spawns a reader thread for each stream and a supervisor thread.

The supervisor parses each stdout line as JSON, extracts progress (`progress`,
`percent`, byte ratios, `current/total`), a status message, and any result envelope.
Events are pushed onto a mutex-protected queue. On exit, the supervisor classifies the
job as succeeded/failed/cancelled, attaches a structured `UnityError` from the result
envelope when present, and emits a terminal event.

`App::poll_jobs` drains the queue, appends log lines (bounded to 600 with 100-line
trimming), updates progress, and on a terminal event refreshes the affected page
(status after CLI install, Editors after Editor/module install, Projects after an
upgrade).

iced 0.14 no longer exposes `iced::time::every`, so the app runs a small thread that
sends `Message::PollJobs` through a `futures::channel::mpsc` subscription every
120 ms. The subscription is only active while a job is running, so the app does not
wake its update loop when idle. This keeps polling independent of the executor
implementation and replaces the former UI event channel.

A single job may run at a time. Starting another is rejected with a notice so two
installers cannot race the same CLI state.

### Cancellation

`JobManager::cancel` sets the flag and kills the child. The supervisor sees a
non-success exit with the flag set and reports `Cancelled`, which is surfaced as an
amber badge and a dismissible notice rather than an error.

### Error handling

`UnityError` carries a code, message, optional detail, and optional exit code.
`util::error_text` joins message and detail for display. Read failures populate the
`Loadable` error and render inline, leaving the rest of the page usable. Mutation
failures surface in the notice banner or dialog. A missing CLI is not a fatal boot
error: `CliStatus { installed: false, .. }` drives the install callout.

## State and pages

### Shell

- Sidebar: brand, five navigation buttons, CLI connection footer.
- Top bar: page title, CLI version badge (or "checking"/"CLI not found"), Refresh.
- Content: notice banner, current page, active job card, all inside a scrollable.
- Dialog: modals replace the shell with a full-screen view. `stack!` was removed
  because layering the scrollable shell under a dialog miscomposites in iced's
  tiny-skia backend (base widgets draw over the dialog).

Navigation lazily loads data the first time a page needs it. Refresh re-fetches only
the current page.

### Overview

Hero strip, CLI card (version, channel, platform, runtime, executable, resolution
source, install path, build date, update availability, update action), account card
(sign-in state, license summary), and running Editors.

### Editors

- Installed Editors: version, architecture, default badge, module list, Manage action.
- Install a new Editor: version/alias input, module catalog with checkboxes,
  select-all, EULA checkbox, dry-run preview, install action.
- Available releases: newest 14 releases with stream badge and Use/installed state.

Tables are manual header rows plus per-item rows (see decision record).

### Projects

Filter input, count, and a manual table: project name + badges (cloud, VCS, favorite,
"Editor not installed"), version, render pipeline, build target, shortened path,
modified date, and a Change version action.

The upgrade modal defaults to an installed Editor different from the project's
current version. It shows the exact command, requires an explicit confirmation, and
runs `projects upgrade` without `--wait` so the job ends with the upgrade rather than
with the Editor session.

### Changelog

Controls select an installed/upgrade version or the installed CLI's own notes. The
first visit auto-loads the changelog. Rendering:

- version header, provider, entry count;
- possible breaking changes panel (or a green "none detected" panel);
- one card per section with counts and tone by section name;
- optional raw Markdown viewer.

Parsing and heuristics live in `unity-core::changelog`. `ChangelogView::load` adapts a
`ChangelogProvider` into a UI-ready shape so a remote provider can be added later.

### Settings

Resolved executable and source, manual path override with save/clear, config
directory and settings file, engine information, and UI/CLI versions.

## Theming

`theme.rs` defines the palette (near-black background, raised panels, warm orange
accent, green/red/amber/blue/violet tones) and style functions for the sidebar,
top bar, cards, hero, modal, modal screen, notice cards, badges, navigation buttons,
and manual table rows. Text styles cover muted, faint, accent, success, warning, and
danger. Widgets use these functions instead of ad-hoc colors so the app stays
visually consistent.

## Persistence

Only one value is persisted: `settings.json` under the platform config directory
(`~/.config/unity-rust-hub` on Linux) with an optional `cliPath`. Load failures fall
back to defaults; save failures surface as a notice. No credentials, tokens, or
account data are stored by the GUI.

## Security and safety

- The CLI installer command is displayed in full and runs only after an explicit
  consent checkbox. `UNITY_CLI_SKIP_SETUP=1` prevents interactive setup from hanging.
- Module EULA acceptance is never implied; `--accept-eula` requires the matching
  checkbox.
- Project upgrades require a second confirmation and warn about on-disk changes.
- All mutations are delegated to the official CLI; the GUI never edits project files
  or the Hub database directly.
- Changelog Markdown is displayed as plain text, never interpreted.

## Distribution on Arch Linux

Two install paths are supported for Arch-based distributions, including CachyOS:

- `packaging/arch/PKGBUILD` builds the current checkout with cargo and installs the
  binary, desktop entry, license, and eight icon sizes under `/usr` via `makepkg -si`.
- `scripts/install-arch.sh` builds the release binary and installs per-user under
  `~/.local` by default, with `--system` for `/usr/local`, `--prefix` for custom
  locations, and an `uninstall` command. It removes only the files it installed and
  refreshes icon/desktop caches.

Both use `packaging/unity-rust-hub.desktop` and the `Icon/Icon-png-icons` hicolor set.
The desktop entry uses `Icon=unity-rust-hub` and `StartupWMClass=unity-rust-hub`, so
launchers group the window correctly. Do not mix the pacman package with a script
install; the uninstall method must match the install method.

## Testing and verification

- `unity-core` unit tests cover envelope parsing, version validation, changelog
  parsing/heuristics, and error mapping.
- `unity-core/tests/live.rs` is ignored by default and exercises reads against a real
  CLI (version, Editors, modules, releases, projects, auth/license, update check,
  changelog) without mutating anything.
- `cargo fmt --all`, `cargo test -p unity-core`, and
  `cargo clippy -p unity-hub --all-targets -- -D warnings` are the required checks.
- Manual smoke tests launch the binary against CLI `1.0.0-beta.15` and verify each page
  renders and loads live data.

## Decision record

1. **Keep `unity-core`.** The adapter was already UI-agnostic and tested; rewriting it
   would add risk without changing behaviour.
2. **Use tiny-skia instead of wgpu.** The UI is text/panel heavy and the test machine
   has no reliable GPU stack. Software rendering removes a class of driver failures.
3. **Manual table rows instead of `iced::widget::table`.** With iced 0.14 the table
   widget caused the Editors page to stop presenting new frames after data arrived.
   Header/row containers with `Length::FillPortion` are predictable and fast enough
   for the dataset sizes involved.
4. **Thread timer instead of `iced::time::every`.** The 0.14 API no longer exposes the
   helper; a channel-backed subscription is stable across executors.
5. **One job at a time.** CLI state is global; serializing installers avoids
   interleaved writes and confusing refresh races.
6. **Changelog provider trait retained.** The current provider shells out to the CLI;
   a remote JSON source can be substituted behind `ChangelogView::load`.
7. **Dialog screens instead of `stack!`.** In iced 0.14 with tiny-skia, stacking a
   dialog over the scrollable shell draws base widgets over the dialog and turns the
   window progressively darker. Full-screen dialogs are deterministic and cheaper.
   Dialog cards also skip drop shadows because tiny-skia's damage tracking repaints
   the shadow over the card on partial redraws.

## Known limitations

- No Editor uninstall or module removal.
- Linux is the tested platform; macOS/Windows code paths compile from the same source
  but are not CI-built.
- Changelog breaking-change flags are English-text heuristics.
- Jobs are in-memory; closing the window ends them.
- The raw changelog is shown as unformatted text.
- Screenshot automation on KDE Wayland can be unreliable because focus is controlled
  by the compositor; this affects verification only, not the app.
