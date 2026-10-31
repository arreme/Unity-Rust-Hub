# Unity Rust Hub

A native desktop GUI for the [Unity CLI](https://docs.unity.com/en-us/unity-cli/unity-cli),
written in Rust with [iced](https://iced.rs). It wraps the official `unity` binary so
you can inspect the local toolchain, manage Editors and modules, change project
versions, and read release notes without keeping a terminal open.

No webview, no Node toolchain, no HTML/JS layer. The release build is a single Rust
binary that renders through iced's software renderer.

The Unity CLI is in beta (`1.0.0-beta.14` at the time of writing). This app is built
against its JSON envelope and treats the CLI as the source of truth for every mutation.

## Features

- **CLI lifecycle** — detect the executable, show version/channel/platform, check for
  updates, install the CLI with Unity's official per-platform script, and self-update.
- **Editors** — installed versions with modules, Linux/macOS/Windows architecture,
  default-Editor state, and the install location.
- **Releases** — newest releases with LTS/beta/alpha and installed badges.
- **Editor install** — pick a version or alias (`lts`, `latest`, `6000.3`, exact), load
  the module catalog, preview the download with `--dry-run`, accept module terms
  explicitly, then install with live NDJSON progress and a cancellable raw log.
- **Modules** — add modules to an installed Editor with `install-modules`.
- **Projects** — every project in the Hub registry, its Editor version, render pipeline,
  build target, cloud/VCS badges, and a guarded upgrade dialog.
- **Changelog** — `unity changelog` rendered as sections, with a focused heuristic for
  **possible breaking changes** and a raw Markdown view.
- **Provider-ready changelog** — `ChangelogProvider` is a trait; v1 uses
  `CliChangelogProvider`. A remote JSON/docs feed can be added without changing the UI.
- **Settings** — explicit CLI path override with auto-detection fallback.

## Stack

| Layer | Choice |
|---|---|
| Desktop shell | iced 0.14 (winit window, no webview) |
| Renderer | `tiny-skia` software rendering (`wgpu` disabled) |
| Core adapter | `crates/unity-core` (synchronous, UI-agnostic) |
| UI crate | `crates/unity-hub` (Elm-style boot/update/view) |
| CLI integration | `unity --format json` / `--format ndjson` |
| Concurrency | thread + `futures::oneshot` for reads; `JobManager` threads for jobs |

## Layout

```
crates/unity-core/     Pure CLI adapter: command specs, envelope parsing, DTOs,
                       errors, validation, changelog parsing/provider, bootstrap.
crates/unity-hub/      iced application: state, views, theme, jobs, settings.
  src/app.rs           state, messages, update loop, shell, dialogs, job polling.
  src/jobs.rs          child processes, NDJSON progress, cancellation, event queue.
  src/views/           overview, editors, projects, changelog, settings.
  assets/icon.png      window icon.
Icon/                  app icon source and hicolor PNG set (16–256 px).
packaging/             desktop entry and Arch Linux PKGBUILD.
scripts/               Arch/CachyOS install and uninstall helper.
docs/superpowers/      iced design spec and implementation plan.
```

The UI never builds a `unity` argument or parses a raw CLI envelope. `unity-core` owns
flag construction, process spawning, JSON normalization, exit-code mapping, and job
specs.

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
cargo test -p unity-core           # parser/validation/heuristics unit tests
cargo clippy -p unity-hub --all-targets -- -D warnings
cargo build --release -p unity-hub
```

Live adapter smoke test against a real Unity CLI (ignored by default):

```bash
cargo test -p unity-core --test live -- --ignored --nocapture
```

This only reads data: version, Editors, modules, releases, projects, auth/license,
update check, and the installed changelog. It never installs an Editor, changes a
project, or updates the CLI.

## How the CLI is invoked

Every read uses:

```
unity <command> --format json --no-pager --no-banner --non-interactive
```

Long operations use `--format ndjson` and are streamed by `JobManager` into the active
job card. `projects upgrade` is run **without** `--wait` so the job ends when the
upgrade does, not when the Editor is closed.

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
- Job polling uses a channel-backed subscription on a 120 ms thread because iced 0.14
  no longer exposes `iced::time::every`.
- The theme is a custom dark palette defined in `src/theme.rs`.

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

- Design spec: [`docs/superpowers/specs/2026-10-31-unity-rust-hub-iced-design.md`](docs/superpowers/specs/2026-10-31-unity-rust-hub-iced-design.md)
- Implementation plan: [`docs/superpowers/plans/2026-10-31-unity-rust-hub-iced.md`](docs/superpowers/plans/2026-10-31-unity-rust-hub-iced.md)
- Unity CLI reference: <https://docs.unity.com/en-us/unity-cli/unity-cli-reference>
- Unity CLI release notes: <https://docs.unity.com/en-us/unity-cli/release-notes>

The previous Tauri/React implementation is preserved on `main` at commit `06c65da`.
