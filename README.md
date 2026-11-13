# Unity Rust Hub

A native desktop GUI for the [Unity CLI](https://docs.unity.com/en-us/unity-cli/unity-cli),
written in Rust with [iced](https://iced.rs). It wraps the official `unity` binary so
you can inspect the local toolchain, manage Editors and modules, change project
versions, and read release notes without keeping a terminal open.

No webview, no Node toolchain, no HTML/JS layer. The release build is a single Rust
binary that renders through iced's `wgpu` backend, with iced's `tiny-skia` software
renderer kept as an automatic fallback when no usable GPU adapter is available.

The Unity CLI is at `1.0.0` (stable). This app is built against its JSON envelope and
treats the CLI as the source of truth for every mutation.

## Download

Prebuilt binaries and installers are published on the
[Releases](https://github.com/arreme/Unity-Rust-Hub/releases) page:

- **Linux** — AppImage, Debian package, pacman package, Flatpak bundle, or a raw binary.
  On Arch-based systems, Shelly can install the pacman package or the Flatpak bundle.
- **Windows** — `*-windows-x64-setup.exe` installer, or the raw `*-portable.exe`.
- **macOS** — DMG for Apple Silicon (`arm64`) or Intel (`x86_64`), plus a zipped `.app`.

The app does not bundle the Unity CLI. Install it from the Overview screen on first
launch, or point Settings at an existing `unity` binary.

### Updating with Shelly (Arch-based systems)

The AppImage embeds its version so
[Shelly](https://github.com/Seafoam-Labs/Shelly-ALPM) can track updates.
Install it through Shelly instead of copying it into place:

```bash
mv UnityRustHub-0.2.1-linux-x86_64.AppImage UnityRustHub.AppImage
shelly install appimage ./UnityRustHub.AppImage
shelly sync appimage UnityRustHub arreme/Unity-Rust-Hub GitHub
```

Then check for and apply updates with:

```bash
shelly list-updates appimage
shelly upgrade appimage
```

Shelly only tracks AppImages it installed; a manually copied file has no update
entry until you import and configure it.

In the Shelly GUI, open **App Images → Unity Rust Hub → Update source**,
choose **GitHub**, and enter `arreme/Unity-Rust-Hub`. Prereleases are skipped
by default; use `--prerelease` on `shelly sync appimage` or the GUI toggle to
track them.

Use the stable `UnityRustHub.AppImage` filename.

Do not rename or move the file after installing it, or Shelly will lose the
update target.

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
  build target, cloud/VCS badges, an Open action, a guarded upgrade dialog, an Add project
  folder picker, a Remove confirmation, and a favorite star backed by `projects pin`.
- **New Project** — a local-first wizard over `projects create`: name and parent folder,
  installed Editor and architecture, a template chosen by render pipeline (built-in
  templates hidden by default), a review with the exact command, then a streamed create.
  It always passes `--no-cloud`, and a partial success keeps the created path and names
  the failed setup step.
- **Templates** — browse `templates --brief` per installed Editor, filter core/custom and
  installed-only, inspect a template's render pipeline and packages, create a registered
  template from a project, or pack a portable `.tgz`. Create and pack stream progress and
  report the resulting path.
- **Processes** — one read-only row per running Editor, merging `status`,
  `editors running`, and `pipeline editors`: project, version, PID, port, state, phase,
  Play mode, frame count, Safe Mode, and blocking dialog, with copy and open-folder
  actions. Optional CPU/memory/uptime sampling is off by default and only runs while the
  page is visible.
- **Changelog** — `unity changelog` rendered as sections with readable bullet points and
  a focused heuristic for **possible breaking changes**, plus a raw Markdown view.
- **Provider-ready changelog** — `ChangelogProvider` is a trait; v1 uses
  `CliChangelogProvider`. A remote JSON/docs feed can be added without changing the UI.
- **Settings** — explicit CLI path override with auto-detection fallback, a persisted
  text-size setting (100%, 110%, 125%, 150%), and a persisted font family chosen from the
  fonts installed on the PC (default Tahoma).

## Stack

| Layer | Choice |
|---|---|
| Desktop shell | iced 0.14 (winit window, no webview) |
| Renderer | iced 0.14 `wgpu` (DX12/Vulkan/Metal) with a `tiny-skia` software fallback |
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
  src/settings.rs           Persisted CLI path, text-size, and font preferences.
  src/util.rs               Serialized reader worker and display formatting.
crates/unity-hub-ui/        Visual layer; depends only on unity-hub-model + iced.
  src/tokens.rs             Figma-mappable palette, spacing, radii, type tokens.
  src/theme.rs              iced theme/styles built from tokens.
  src/components.rs         Shared cards, badges, tables, headers.
  src/pages/                overview, editors, projects, changelog, settings.
  src/shell.rs              Sidebar, notice and job cards.
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
  `libxkbcommon`, `libwayland-client`, and `libX11`/`libxcb`. A working Vulkan or
  OpenGL driver gets the GPU renderer; without one the app falls back to the
  `tiny-skia` software renderer. No WebKitGTK is required.
- **macOS** — Xcode Command Line Tools (`xcode-select --install`).
- **Windows** — the MSVC Rust toolchain. No WebView2 runtime is needed. A DX12
  driver gets the GPU renderer; otherwise the app falls back to software.

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

### Two builds: GPU (default) and slim

Rendering is a compile-time choice, because the resident cost of a GPU context is
fixed and cannot be trimmed from the UI:

```bash
# Default: GPU via wgpu. Smooth scrolling, ~100 MB resident.
cargo build --release -p unity-hub

# Slim: software rendering only. ~26 MB resident, ~7 MB binary, no GPU driver.
cargo build --release -p unity-hub --no-default-features
```

Both builds are the same UI and the same behaviour; only the renderer differs. See
[Performance](#performance) for the measured cost of each. The packaging config and
the GitHub release workflow build the default GPU variant.

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
needs network access for crates.io. `scripts/set-version.sh` keeps `pkgver` in sync
with the workspace version. Build dependencies: `base-devel` and a Rust/cargo
toolchain (the `rust` or `rustup` package).

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

### Version bumps and releases

Use the version helper instead of editing version fields by hand:

```bash
./scripts/set-version.sh 0.3.0          # update files only
./scripts/set-version.sh 0.3.0 --commit # update and commit
./scripts/set-version.sh 0.3.0 --tag    # commit and tag v0.3.0
./scripts/set-version.sh 0.3.0 --release --yes
```

The script updates `Cargo.toml`, `Packager.toml`, `packaging/arch/PKGBUILD`,
`Cargo.lock`, and the AppImage example in this README. `--release` pushes the
branch and tag, which starts the GitHub release workflow. It requires a clean
worktree and runs the workspace tests. `--no-verify` is update-only; it cannot
be combined with the git flags because a stale `Cargo.lock` would fail the
release workflow.

### Packaging a release locally

```bash
cargo install cargo-packager --version 0.11.8 --locked

# Linux: AppImage, Debian package, pacman package.
# VERSION makes appimagetool embed X-AppImage-Version for Shelly.
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Packager.toml | head -n1)" \
  cargo packager --config Packager.toml --formats appimage,deb,pacman

# Windows: NSIS installer.
cargo packager --config Packager.toml --formats nsis

# macOS: .app bundle and DMG.
cargo packager --config Packager.toml --formats app,dmg
```

Artifacts are written to `dist/`. The GitHub release workflow runs the same
commands on native runners and attaches the results to a tagged release.

Live adapter smoke test against a real Unity CLI (ignored by default):

```bash
cargo test -p unity-core --test live -- --ignored --nocapture
```

This only reads data: version, Editors, modules, releases, projects, auth/license,
update check, and the installed changelog. It never installs an Editor, changes a
project, or updates the CLI.

## Performance

Scroll cost is dominated by the renderer, not by the CLI or the view model. Measured
on Windows 11 with an RTX 4060 Ti, release build, 1280×840 window, Unity CLI `1.0.0`:

| Metric | GPU build (default) | Slim build (`--no-default-features`) |
|---|---|---|
| CPU per painted scroll frame | 1–2 ms | 12–13 ms |
| Sustained CPU while scrolling | 1–6% of one core | 40–82% of one core |
| Idle CPU | 0% | 0% |
| Idle RSS | 103–120 MB | 24–26 MB |
| Threads | ~22 | ~11 |
| Binary | 13.1 MB | 7.1 MB |
| Needs a GPU driver | yes (DX12/Vulkan/Metal) | no |

The GPU path is roughly an order of magnitude cheaper per frame, which is what keeps
scrolling smooth; the software path saturates most of a core once wheel events arrive
faster than it can paint. The GPU path's memory is almost entirely graphics-driver
overhead: an empty iced window on the same backend already costs 108 MB, and the full
UI costs nothing on top of that. Shrinking the window or minimizing it does not change
the figure, so the UI itself is already as slim as it can be.

The GPU build prefers DX12 on Windows (`WGPU_BACKEND` overrides it, e.g. `vulkan`,
`gl`; `WGPU_POWER_PREF` selects a different adapter). If no adapter can be created it
falls back to `tiny-skia` at runtime, so the app still starts without a GPU driver.
The design gallery is software-rendered by default; add `--features gpu` for the GPU
path.

Independent of the renderer: the app serializes reads through one worker, sets
`DO_NOT_TRACK=1` on every CLI invocation, and gives each read a 30-second deadline.
`scripts/measure-memory.py` reproduces the RSS measurement on Linux.

## How the CLI is invoked

Every read uses:

```
unity <command> --format json --no-pager --no-banner --non-interactive
```

Long operations use `--format ndjson` and are streamed by `JobManager` into the active
job card. `projects upgrade` is run **without** `--wait` so the job ends when the
upgrade does, not when the Editor is closed.

On Linux, the CLI reads the stored sign-in through its resident auth broker. When that
broker is not running, every command reads the Secret Service keyring itself, and a
locked keyring shows its unlock prompt once per command. Before the first credential
read (and before `unity open`), the app runs `unity auth broker ensure` so the whole
batch shares one session. `UNITY_NO_AUTH_BROKER` defeats this, so the app shows a notice
when it is set.

Set `UNITY_HUB_CLI_TRACE=1` to append every CLI invocation (`argv`, start time, exit code,
elapsed time) to `cli-trace.log` next to `cli-install.json`. It is the fastest way to
match a credential prompt to the command that caused it. `UNITY_HUB_CLI_TRACE_FILE`
overrides the destination.

CLI beta.15 removed the `list` subcommands and deprecated a few flags, so the adapter
uses the current spellings: `unity projects --all`, `unity modules <version>`,
`unity releases --offset`, and `unity install-modules --editor-version`.

The adapter is aligned to CLI 1.0.0:

- Exit codes `17` (`UNAVAILABLE`) and `18` (`AWAITING_INPUT`) are still part of the 1.0.0
  table (verified with `unity commands --format json`), mapped, and treated as retryable.
- `unity install-modules` exiting `0` when every named module is already installed counts
  as success, even when no result frame is written.
- `unity projects upgrade` refuses a project that is open in an Editor; the upgrade job
  surfaces the CLI error, and the app also checks the running-Editor list before starting.
- The Overview running-Editor card merges `unity editors running` with
  `unity pipeline editors` to show phase, Pipeline version, update availability, Safe Mode,
  and a blocking dialog. It stays read-only; dialogs are not answered from the app.
- `self-update` does not emit NDJSON in 1.0.0 (it prints a two-column result line), so the
  update job asks for human output and the app refreshes `unity version` afterwards.
- 1.0.0 adds `unity close --if-saved` and `unity install --headless`; neither is wired into
  the UI yet. See [`docs/unity-cli-1.0-upgrade.md`](docs/unity-cli-1.0-upgrade.md).

Resolution order for the binary:

1. Path saved in Settings (`~/.config/unity-rust-hub/settings.json` on Linux).
2. `unityhub/cli-install.json` or `UnityHub/cli-install.json` install receipt.
3. `UNITY_CLI_HOME/bin/unity`.
4. Platform default (`~/.local/bin`, `~/.unity/bin`, `%LOCALAPPDATA%\Unity\bin`).
5. `PATH`.

## UI notes

- The shell is a fixed sidebar plus a scrollable content area; each page draws its
  own title and actions, so there is no top bar.
- Tables are built from header and row containers rather than `iced::widget::table`;
  the table widget caused a rendering stall with live data in iced 0.14.
- Dialogs replace the shell with a full-screen view instead of using `stack!`. Stacked
  layers over the scrollable shell miscomposite in iced's tiny-skia fallback, and
  dialog drop shadows are omitted because tiny-skia repaints them over the card on
  partial redraws.
- The renderer is iced's `wgpu` backend by default, with `tiny-skia` kept as the
  runtime fallback. `gpu` is a Cargo feature (`--no-default-features` builds the
  software-only binary), so a single UI has two build profiles.
- Job polling is event-driven: `JobManager` wakes a subscription only when it has
  an event, coalescing bursts into one UI poll every 100 ms. An idle window does not
  wake the update loop, and the queued event/log buffers are bounded. The log tail is
  rendered once per line and shared as an `Arc<str>`, so a progress-only update neither
  copies nor reshapes the scrollback.
- CLI reads run through one serialized worker with a 30 s timeout, so only one
  `unity` process is alive at a time. Every invocation sets `DO_NOT_TRACK=1`; this
  avoids the CLI's analytics call, which otherwise adds roughly one second and extra
  memory to most commands.
- The window icon is compiled in as raw RGBA, which lets the build drop iced's
  `image` feature and its image codecs. The custom theme means Linux theme detection
  is not linked either.
- The UI bundles Inter, JetBrains Mono, and Lucide icon subsets (`scripts/subset-fonts.sh`).
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
- No credentials are read, stored, or forwarded. The app never passes
  `--insecure-storage` to `unity auth login`, so a CLI 1.0.0 sign-in either uses the
  system credential store or reports that no store is available. Changelog Markdown is
  displayed as plain text, never interpreted.

## Known limitations (v1)

- No Editor uninstall, module removal, or disk-usage tools.
- No running-Editor control beyond a read-only list; the Pipeline package is not required.
- Project upgrades are restricted to installed Editors.
- Changelog breaking flags are heuristics over English release notes, not a guarantee.
- Jobs are not persisted across app restarts; closing the window cancels them.
- Only one job may run at a time.
- Linux is the most exercised platform. macOS/Windows builds run in CI, but the runtime
  behavior there is less tested than on Linux.
- The raw changelog is shown as unformatted text.

## Documentation

- Designer/controller contract: [`docs/design/contract.md`](docs/design/contract.md)
- Design tokens: [`docs/design/tokens.md`](docs/design/tokens.md)
- Historical iced design spec: [`docs/superpowers/specs/2026-10-31-unity-rust-hub-iced-design.md`](docs/superpowers/specs/2026-10-31-unity-rust-hub-iced-design.md)
- Historical implementation plan: [`docs/superpowers/plans/2026-10-31-unity-rust-hub-iced.md`](docs/superpowers/plans/2026-10-31-unity-rust-hub-iced.md)
- Unity CLI reference: <https://docs.unity.com/en-us/unity-cli/unity-cli-reference>
- Unity CLI release notes: <https://docs.unity.com/en-us/unity-cli/release-notes>
