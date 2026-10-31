# Unity Rust Hub — Implementation Plan

Date: 2026-10-31
Spec: `docs/superpowers/specs/2026-10-31-unity-rust-hub-design.md`
Execution: autonomous (user approved design and left)

## Checklist

- [x] 1. Scaffold Tauri v2 + React + TypeScript + Vite + pnpm project at repo root.
- [x] 2. Configure Tauri window, capabilities, app metadata, icons.
- [x] 3. Implement Rust CLI floor (`crates/unity-core`):
  - [x] `errors.rs` — typed error, exit-code mapping.
  - [x] `versions.rs` — version/module/path validation.
  - [x] `envelope.rs` — envelope + flexible list extraction.
  - [x] `runner.rs` — binary resolution + process runner + command specs.
  - [x] `models.rs` — DTOs for status, editors, releases, modules, projects, changelog.
  - [x] `changelog.rs` — Markdown sections + breaking heuristics + provider trait.
  - [x] `bootstrap.rs` — per-OS CLI install spec.
- [x] 4. Implement jobs:
  - [x] `jobs/mod.rs` — registry, spawn, NDJSON parsing, cancellation.
  - [x] event payloads + `job:update` emission.
- [x] 5. Implement Tauri commands for every feature and register them.
- [x] 6. Rust unit tests for parsing/validation/heuristics; `cargo test` passes.
- [x] 7. Frontend foundation:
  - [x] Vite config, TS config, entrypoint, app shell + styles.
  - [x] typed API wrappers, `useAsync`, `useJob`, toast/error handling.
  - [x] shared components (Card, Button, Badge, Modal, Progress, LogPanel, JobPanel).
- [x] 8. Feature views:
  - [x] Overview: CLI status, install/update, auth/license, running Editors.
  - [x] Editors: installed + modules, releases, install wizard with dry run.
  - [x] Projects: table/search, change-version dialog + job.
  - [x] Changelog: fetch, sections, breaking panel, raw markdown/copy.
  - [x] Settings: CLI path override, paths, about.
- [x] 9. Connect job events to progress bars + log panes; cancel buttons.
- [x] 10. TypeScript/Vite build passes (`pnpm check`, `pnpm build`).
- [x] 11. `cargo fmt`, `cargo clippy`, `cargo test` pass.
- [x] 12. `pnpm tauri build --no-bundle` succeeds.
- [x] 13. Smoke test CLI-facing code against real `unity` data (live test).
- [x] 14. Write README (run/build, architecture, known limitations).
- [x] 15. Commit the work.

## Verification results

- `pnpm check` — pass.
- `pnpm build` — pass (Vite 8.3.5).
- `cargo fmt --all --check` — pass.
- `cargo test -p unity-core` — 17 unit tests pass.
- `cargo test -p unity-core --test live -- --ignored --nocapture` — pass against
  Unity CLI 1.0.0-beta.14 (3 Editors, 6 projects, 6 changelog sections / 344 entries /
  22 possible-breaking candidates).
- `cargo clippy -p unity-core --all-targets -- -D warnings` — pass.
- `cargo clippy -p unity-rust-hub --all-targets -- -D warnings` — pass.
- `pnpm tauri build --no-bundle` — pass; release binary produced.
- Runtime smoke test: the release binary started under a local WebKitGTK 4.1 sandbox
  and the frontend invoked `get_cli_status` against the real CLI. This machine does not
  have the system `webkit2gtk-4.1` package; see README for the prerequisite.

## Constraints honored

- No Editor install, uninstall, project upgrade, or CLI self-update was executed.
- All read-only CLI calls are safe.
- Every CLI flag lives in Rust; the frontend never builds a command string.
