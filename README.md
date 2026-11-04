# Unity Rust Hub

A native desktop GUI for the [Unity CLI](https://docs.unity.com/en-us/unity-cli/unity-cli),
written in Rust with [iced](https://iced.rs). It wraps the official `unity` binary so
you can inspect the local toolchain, manage Editors and modules, change project
versions, and read release notes without keeping a terminal open.

No webview, no Node toolchain, no HTML/JS layer. The release build is a single Rust
binary that renders through iced's software renderer.

The Unity CLI is in beta (`1.0.0-beta.16` at the time of writing). This app is built
against its JSON envelope and treats the CLI as the source of truth for every mutation.

## Features

- **CLI lifecycle** — detect the executable, show version/channel/platform, check for
  updates, install the CLI with Unity's official per-platform script, self-update, and
  sign in to Unity with `unity auth login`.
- **Editors** — installed versions with modules, Linux/macOS/Windows architecture,
  default-Editor state, and the install location.
- **Running Editors** — read-only status from `unity editors running`, enriched with
  `unity pipeline editors` phase, Pipeline version, update availability, Safe Mode, and a
  blocking dialog.
- **Releases** — newest releases with LTS/beta/alpha and installed badges.
- **Editor install** — pick a version or alias (`lts`, `latest`, `6000.3`, exact), load
  the module catalog, preview the download with `--dry-run`, accept module terms
  explicitly, then install with live NDJSON progress and a cancellable raw log.
- **Modules** — add modules to an installed Editor with `install-modules`.
- **Projects** — every project in the Hub registry, its Editor version, render pipeline,
  build target, cloud/VCS badges, an Open action, and a guarded upgrade dialog.
- **Changelog** — `unity changelog` rendered as sections with readable bullet points and
  a focused heuristic for **possible breaking changes**, plus a raw Markdown view.
- **Provider-ready changelog** — `ChangelogProvider` is a trait; v1 uses
  `CliChangelogProvider`. A remote JSON/docs feed can be added without changing the UI.
- **Settings** — explicit CLI path override with auto-detection fallback and a persisted
  text-size setting (100%, 110%, 125%, 150%).

## Stack

| Layer | Choice |
|---|---|
| Desktop shell | iced 0.14 (winit window, no webview) |
| Renderer | `tiny-skia` software rendering (`wgpu` disabled) |
| CLI adapter | `crates/unity-core` (synchronous, UI-agnostic) |
| Contract | `crates/unity-hub-model` (view models + `UiAction`, no dependencies) |
| Controller | `crates/unity-hub-controller` (state, jobs, settings, view-model mapping) |
| Visuals | `crates/unity-hub-ui` (design tokens, components, pages, modals, gallery) |
| Composition root | `crates/unity-hub` (iced application, window, runtime wiring) |
| CLI integration | `unity --format json` / `--format ndjson` |
| Concurrency | one serialized CLI reader worker; `JobManager` threads for streaming jobs |

## Layout

```
crates/unity-core/          Pure CLI adapter: command specs, envelope parsing, DTOs,
                            errors, validation, changelog parsing/provider, bootstrap.
crates/unity-hub-model/     Stable presentation contract: view models, UiAction,
                            LoadState, semantic tones. No dependencies.
crates/unity-hub-controller/ State, messages, update loop, jobs, settings, and
  src/app.rs                domain/CLI → view-model mapping.
  src/jobs.rs               Child processes, NDJSON progress, cancellation.
  src/settings.rs           Persisted CLI path and text-size preference.
  src/util.rs               Serialized reader worker and display formatting.
crates/unity-hub-ui/        Visual layer; depends only on unity-hub-model + iced.
  src/tokens.rs             Figma-mappable palette, spacing, radii, type tokens.
  src/theme.rs              iced theme/styles built from tokens.
  src/components.rs         Shared cards, badges, tables, headers.
  src/pages/                overview, editors, projects, changelog, settings.
  src/shell.rs              Sidebar, top bar, notice and job cards.
  src/modals.rs             Full-screen dialog screens.
  src/fixtures.rs           Deterministic content for every screen/state.
  src/bin/gallery.rs        CLI-free design gallery.
crates/unity-hub/           Thin binary: iced application, window/icon, runtime.
Icon/                       app icon source and hicolor PNG set (16–256 px).
packaging/                  desktop entry and Arch Linux PKGBUILD.
scripts/                    Arch/CachyOS install and uninstall helper.
docs/design/                Designer/controller contract and token mapping.
docs/superpowers/           Historical iced design spec and implementation plan.
```

Dependency direction is one-way: `unity-hub-ui` → `unity-hub-model`; the controller
depends on `unity-core` and the model; the binary wires controller + UI together. The
UI never builds a `unity` argument, parses a raw CLI envelope, or sees a `UnityError`.

## Prerequisites

- **Rust** stable (tested with 1.99).
- **Unity CLI** — optional at first launch. The Overview screen installs it.
- **Linux** — X11 or Wayland client libraries. A typical desktop already has them;
  for a minimal build host install the development packages for
  `libxkbcommon`, `libwayland-client`, and `libX11`/`libxcb`. `tiny-skia` means no
  Vulkan/OpenGL driver or WebKitGTK is required.
- **macOS** — Xcode Command Line Tools (`xcode-select --install`).
- **Windows** — the MSVC Rust toolchain. No WebView2 runtime is needed.

## Run

```bash
cargo run -p unity-hub
```

Build a release binary:

```bash
cargo build --release -p unity-hub
./target/release/unity-rust-hub
```

The workspace default member is `unity-hub`, so `cargo run` and `cargo build --release`
also work from the repository root.

## CachyOS / Arch Linux

### Pacman package (system-wide)

`packaging/arch/PKGBUILD` builds the current checkout and installs the binary, desktop
entry, MIT license, and all eight icon sizes under `/usr`:

```bash
cd packaging/arch
makepkg -si
```

Remove it with:

```bash
sudo pacman -Rns unity-rust-hub
```

The PKGBUILD uses this working tree instead of a remote tarball, so the first build
needs network access for crates.io. Bump `pkgver` when the workspace version changes.
Build dependencies: `base-devel` and a Rust/cargo toolchain (the `rust` or `rustup`
package).

### Per-user install script (no sudo)

```bash
./scripts/install-arch.sh                  # ~/.local
./scripts/install-arch.sh uninstall

./scripts/install-arch.sh --system         # /usr/local via sudo
./scripts/install-arch.sh --system uninstall
```

The script builds the release binary, installs `unity-rust-hub.desktop`, the license,
and the 16–256 px icons, then refreshes the icon and desktop caches. It also supports
`--prefix PATH`, `--no-build`, `--binary PATH`, and `--purge` (which removes
`~/.config/unity-rust-hub`). If you use the per-user install, make sure `~/.local/bin`
is on your `PATH`.

The launcher appears in the application menu as **Unity Rust Hub**. Do not mix a
pacman package with a script install; uninstall using the same method you used to
install.

## Verify

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release -p unity-hub
```

Render every screen/modal/job fixture without a Unity CLI:

```bash
cargo run -p unity-hub-ui --bin unity-hub-design-gallery
```

Live adapter smoke test against a real Unity CLI (ignored by default):

```bash
cargo test -p unity-core --test live -- --ignored --nocapture
```

This only reads data: version, Editors, modules, releases, projects, auth/license,
update check, and the installed changelog. It never installs an Editor, changes a
project, or updates the CLI.

## Performance

Measured on a Linux/Wayland release build (1280×840, Unity CLI `1.0.0-beta.15`):

| Metric | Result |
|---|---|
| Idle app RSS | 23–27 MB |
| Peak RSS during boot (app + CLI child) | ~55–60 MB |
| Concurrent Unity CLI processes | 1 |
| Time until the last boot read finishes | ~0.4 s |
| Idle CPU | 0% |

The app serializes reads through one worker, sets `DO_NOT_TRACK=1` on every CLI
invocation, and gives each read a 30-second deadline. `scripts/measure-memory.py`
reproduces the RSS measurement on Linux.

## How the CLI is invoked

Every read uses:

```
unity <command> --format json --no-pager --no-banner --non-interactive
```

Long operations use `--format ndjson` and are streamed by `JobManager` into the active
job card. `projects upgrade` is run **without** `--wait` so the job ends when the
upgrade does, not when the Editor is closed.

CLI beta.15 removed the `list` subcommands and deprecated a few flags, so the adapter
uses the current spellings: `unity projects --all`, `unity modules <version>`,
`unity releases --offset`, and `unity install-modules --editor-version`.

The adapter is aligned to CLI beta.16:

- Exit codes `17` (`UNAVAILABLE`) and `18` (`AWAITING_INPUT`) are mapped and treated as
  retryable.
- `unity install-modules` exiting `0` when every named module is already installed counts
  as success, even when no result frame is written.
- `unity projects upgrade` refuses a project that is open in an Editor; the upgrade job
  surfaces the CLI error, and the app also checks the running-Editor list before starting.
- The Overview running-Editor card merges `unity editors running` with
  `unity pipeline editors` to show phase, Pipeline version, update availability, Safe Mode,
  and a blocking dialog. It stays read-only; dialogs are not answered from the app.

Resolution order for the binary:

1. Path saved in Settings (`~/.config/unity-rust-hub/settings.json` on Linux).
2. `unityhub/cli-install.json` or `UnityHub/cli-install.json` install receipt.
3. `UNITY_CLI_HOME/bin/unity`.
4. Platform default (`~/.local/bin`, `~/.unity/bin`, `%LOCALAPPDATA%\Unity\bin`).
5. `PATH`.

## UI notes

- The shell is a fixed sidebar plus a scrollable content area and top bar.
- Tables are built from header and row containers rather than `iced::widget::table`;
  the table widget caused a rendering stall with live data in iced 0.14.
- Dialogs replace the shell with a full-screen view instead of using `stack!`. Stacked
  layers over the scrollable shell miscomposite in iced's tiny-skia backend, and
  dialog drop shadows are omitted because tiny-skia repaints them over the card on
  partial redraws.
- Job polling is event-driven: `JobManager` wakes a subscription only when it has
  an event, coalescing bursts into one UI poll every 100 ms. An idle window does not
  wake the update loop, and the queued event/log buffers are bounded.
- CLI reads run through one serialized worker with a 30 s timeout, so only one
  `unity` process is alive at a time. Every invocation sets `DO_NOT_TRACK=1`; this
  avoids the CLI's analytics call, which otherwise adds roughly one second and extra
  memory to most commands.
- The window icon is compiled in as raw RGBA, which lets the build drop iced's
  `image` feature and its image codecs. The custom theme means Linux theme detection
  is not linked either.
- The theme is built from the design tokens in
  `crates/unity-hub-ui/src/tokens.rs`; component styles live in
  `crates/unity-hub-ui/src/theme.rs`.
- `cargo run -p unity-hub-ui --bin unity-hub-design-gallery` renders every page,
  modal, job state, loading state, and error state with fixture view models, so
  design review does not depend on a Unity installation.
- Settings persist the optional CLI path and a text-size preference (100–150%). The
  scale applies to text only, so controls and layout geometry stay fixed.

## Safety

- The CLI installer is Unity's official script, shown in full in the consent dialog and
  executed only after the checkbox is confirmed. The app sets `UNITY_CLI_SKIP_SETUP=1`
  so the installer's interactive setup does not hang under the GUI.
- Module license acceptance is never implied. `--accept-eula` is passed only after the
  matching checkbox is checked.
- Project upgrades require a second confirmation and warn that the project on disk can
  change. Commit or back up first.
- No credentials are read, stored, or forwarded. Changelog Markdown is displayed as
  plain text, never interpreted.

## Known limitations (v1)

- No Editor uninstall, module removal, or disk-usage tools.
- No running-Editor control beyond a read-only list; the Pipeline package is not required.
- Project upgrades are restricted to installed Editors.
- Changelog breaking flags are heuristics over English release notes, not a guarantee.
- Jobs are not persisted across app restarts; closing the window cancels them.
- Only one job may run at a time.
- Linux is the tested platform. macOS/Windows code paths are implemented but not CI-built.
- The raw changelog is shown as unformatted text.

## Documentation

- Designer/controller contract: [`docs/design/contract.md`](docs/design/contract.md)
- Design tokens: [`docs/design/tokens.md`](docs/design/tokens.md)
- Historical iced design spec: [`docs/superpowers/specs/2026-10-31-unity-rust-hub-iced-design.md`](docs/superpowers/specs/2026-10-31-unity-rust-hub-iced-design.md)
- Historical implementation plan: [`docs/superpowers/plans/2026-10-31-unity-rust-hub-iced.md`](docs/superpowers/plans/2026-10-31-unity-rust-hub-iced.md)
- Unity CLI reference: <https://docs.unity.com/en-us/unity-cli/unity-cli-reference>
- Unity CLI release notes: <https://docs.unity.com/en-us/unity-cli/release-notes>
