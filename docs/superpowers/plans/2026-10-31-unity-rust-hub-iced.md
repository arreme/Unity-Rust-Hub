# Unity Rust Hub — iced implementation plan

Date: 2026-10-31
Branch: `feat/iced-native-ui`
Base: `06c65da` on `main` (Tauri/React implementation, preserved)
Status: complete

## Objective

Replace the Tauri/React UI with a native iced 0.14 application while keeping the
`unity-core` adapter and the full feature set. Work happens only on the feature
branch; `main` remains a working Tauri implementation.

## Phase 0 — Preserve the baseline

- [x] Commit the Tauri implementation on `main` as `06c65da`.
- [x] Create `feat/iced-native-ui` from that commit.
- [x] Confirm a clean starting tree and branch name.

Verification: `git log --oneline -1` showed the Tauri commit; `git branch --show-current`
reported `feat/iced-native-ui`.

## Phase 1 — Workspace and UI-agnostic core

- [x] Change the workspace to `crates/unity-core` + `crates/unity-hub`, with the hub
      as the default member.
- [x] Remove the Tauri member from the workspace.
- [x] Add `UnityCli::status()` so boot can resolve path, version, and install path in
      one typed value; a missing CLI is reported as `installed: false`, not a failure.
- [x] Replace raw preview JSON with typed `InstallPreview` / `PreviewEditor` /
      `PreviewModule` models while keeping the raw value for fallback display.
- [x] Add `ChangelogView::load` and keep the `ChangelogProvider` trait so the UI never
      parses a provider response directly.
- [x] Update the core crate docs to state that it has no UI toolkit dependency.

Verification: `cargo test -p unity-core` — 17 unit tests pass; the ignored live test
remains read-only.

## Phase 2 — iced application skeleton

- [x] Add `crates/unity-hub` with the `unity-rust-hub` binary.
- [x] Configure iced 0.14 with `default-features = false` and `crisp`, `image`,
      `linux-theme-detection`, `thread-pool`, `tiny-skia`, `wayland`, `x11`.
- [x] Build `App::boot` / `App::update` / `App::view` with typed messages and
      `Loadable<T>` per dataset.
- [x] Add a custom dark theme, window size/minimum, centering, and icon.
- [x] Add `settings.rs` for the optional CLI path override.

Verification: `cargo build -p unity-hub` succeeded; the window opened with the custom
theme on Linux.

## Phase 3 — Async reads and jobs

- [x] Add `util::run_blocking`: a dedicated thread plus `futures::oneshot`, consumed by
      `Task::perform`, so synchronous CLI calls never block the UI thread.
- [x] Add `jobs.rs`: spawn a `CommandSpec`, pipe stdout/stderr, parse NDJSON progress,
      retain the last result envelope, map failures to `UnityError`, and support
      cancellation by killing the child.
- [x] Add a 120 ms channel-backed subscription to poll job events, because iced 0.14
      no longer exposes `iced::time::every`.
- [x] Serialize mutations: reject a second job while one is running.
- [x] Refresh the affected dataset after each terminal job.

Verification: the app stayed responsive while loading Editors, releases, projects, and
changelog data from CLI `1.0.0-beta.14`; compiled with clippy `-D warnings`.

## Phase 4 — Shell and pages

- [x] Shell: sidebar navigation, CLI status footer, top bar with version badge and
      Refresh, notice banner, active job card, stacked modal overlay.
- [x] Overview: CLI detail card, update action, account/license card, running Editors,
      quick links.
- [x] Editors: installed list, module management dialog, install form with catalog,
      select-all, EULA, dry-run preview, release list.
- [x] Projects: filterable table, badges, guarded upgrade dialog restricted to
      installed Editors.
- [x] Changelog: target picker, lazy auto-load on first visit, breaking-change panel,
      sections, raw Markdown toggle.
- [x] Settings: resolved path/source, override save/clear, config paths, about.
- [x] Dialogs: install CLI consent, Editor modules, project upgrade, delete/dismiss
      interactions.

Verification: screenshots captured for Overview, Editors, Projects, Changelog, and
Settings; each showed live data (3 Editors, 40 releases, 6 projects, 6 changelog
sections / 344 entries / 22 breaking candidates).

## Phase 5 — Replace the Tauri tree

- [x] Delete `src-tauri/`, `src/`, `public/`, `index.html`, `package.json`,
      `pnpm-lock.yaml`, Vite/TypeScript config, and `node_modules`.
- [x] Copy a window icon into `crates/unity-hub/assets/` before removing the Tauri tree.
- [x] Regenerate `Cargo.lock`; confirm it contains no Tauri dependencies.
- [x] Replace the README and docs with iced instructions.

Verification: `grep -c tauri Cargo.lock` returned 0; `git status` shows only the
intended deletions and the new `crates/unity-hub/` tree.

## Phase 6 — Final verification

- [x] `cargo fmt --all`
- [x] `cargo test -p unity-core` (17 passed, 1 ignored live test)
- [x] `cargo clippy -p unity-hub --all-targets -- -D warnings`
- [x] `cargo build --release -p unity-hub`
- [x] `grep -R "eprintln\|UNITY_HUB_PAGE\|dbg!" crates` returns nothing.
- [x] Smoke-test the release/debug binary against the real Unity CLI.
- [ ] Commit the rewrite on `feat/iced-native-ui`.

## Phase 7 — Arch/CachyOS packaging

- [x] Add `packaging/unity-rust-hub.desktop` with the launcher name, icon, categories,
      and window class.
- [x] Add `packaging/arch/PKGBUILD` that compiles the current checkout, installs the
      binary, desktop entry, license, and 16–256 px hicolor icons, and declares the
      X11/Wayland runtime dependencies.
- [x] Add `scripts/install-arch.sh` with `install` (default per-user `~/.local`),
      `uninstall`, `--system` (`/usr/local`), `--prefix`, `--no-build`, `--binary`,
      and `--purge`.
- [x] Use `Icon/Icon-png-icons/Icon-256x256.png` as the embedded window icon so the
      launcher and window icons match.
- [x] Document both paths in the README.

Verification: `shellcheck` and `bash -n` pass; `desktop-file-validate` passes;
`makepkg -f` produced a package containing the expected binary, desktop entry, license,
and eight icons; the script installed and cleanly uninstalled a temp prefix with no
leftover files; the per-user install launched successfully.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Blocking CLI calls freeze the UI | All reads run on threads via `run_blocking` |
| Long installs look hung | NDJSON progress, log view, cancellable child |
| Two installs race the CLI | Single active job enforced in `update` |
| `table` widget rendering stall | Manual header/row containers with fill portions |
| Software renderer too slow | Data is bounded; releases capped at 14 rows; logs trimmed |
| Missing CLI on first run | `CliStatus` reports absence; install flow remains available |
| KDE focus blocks screenshot checks | Verified functionality through data/state and partial screenshots; not a product issue |

## Follow-ups (not in this branch)

- Editor uninstall and module removal.
- CI builds for macOS and Windows.
- Optional wgpu feature flag for machines with reliable GPU drivers.
- Remote changelog provider behind the existing trait.
- Settings for window size and persisted job history.
