# Unity Rust Hub — Design Spec

Date: 2026-10-31
Status: Approved for implementation (user approved design and asked for autonomous completion)

## 1. Context

The Unity CLI (`unity`, currently `1.0.0-beta.14` on this machine) is the standalone
replacement for the Unity Hub CLI. It already exposes everything needed to manage
Editors, modules, projects, releases, changelogs, and the CLI's own lifecycle, with a
stable JSON/NDJSON envelope.

This project is a small desktop GUI over that CLI. It is a convenience layer, not a
reimplementation: every mutation is performed by the CLI.

## 2. Goals

1. Detect the Unity CLI, its version, and update availability; install it on a machine
   with the official per-platform script; self-update it.
2. List installed Editors with their modules, and list available releases.
3. Install a new Editor version with selected modules, with live progress.
4. List registered Unity projects, their Editor versions, and change a project's
   version through `unity projects upgrade`.
5. Show the CLI changelog and highlight likely breaking changes. The changelog layer
   must be provider-based so a remote JSON API can replace or augment it later.
6. Cross-platform code paths (Linux, macOS, Windows); Linux is the tested platform in v1.

## 3. Non-goals (v1)

- Unity account/license management UI (only surfaced as diagnostics).
- Cloud Build, Pipeline Automation, VCS, collaboration.
- Creating new Unity projects.
- Editing project files directly.
- Background auto-updating or auto-installing Editors.
- Bundling the Unity CLI binary (licensing/size); the app installs it on request.

## 4. Architecture

```
React 19 + TypeScript (Vite)  ──invoke/events──  Rust (Tauri v2)
        view layer                                 commands ── UnityCli ── `unity` process
                                                   jobs     ── JobManager (NDJSON stream)
```

- **Rust owns all CLI knowledge.** Argument construction, machine flags, JSON envelope
  parsing, data-shape normalization, error mapping, and exit-code semantics live in one
  module. The frontend receives typed DTOs only.
- **Tauri commands are thin.** They validate input, call `UnityCli` or `JobManager`, and
  return serializable results.
- **Long-running operations are jobs.** `install`, `install-modules`, `projects upgrade`,
  and CLI self-update run under `JobManager`. Progress is streamed as `job:update`
  events; every job keeps a raw log and can be cancelled.
- **No caching in v1.** Views fetch on mount and after mutations.

### Project layout

```
src/                         React
  api/                       invoke/listen wrappers + DTO types
  components/                Button, Card, Badge, Modal, Table, Toast, LogPanel...
  features/overview|editors|projects|changelog|settings
  hooks/
crates/unity-core/src/       Pure, testable CLI adapter (no Tauri dependency)
  runner.rs                  CLI facade, command specs, process execution
  envelope.rs                JSON envelope + flexible list extraction
  models.rs                  DTOs
  errors.rs                  exit-code mapping
  versions.rs                input validation
  changelog.rs               Markdown parser, breaking heuristics, provider trait
  bootstrap.rs               per-OS CLI installer specs
src-tauri/src/
  commands/                  Thin Tauri commands
  jobs/                      job registry, NDJSON streaming, cancellation
  settings.rs                persisted CLI path override
docs/                        this spec + implementation plan
```

## 5. CLI adapter contract

Every invocation uses:

```
unity <args...> --format <json|ndjson> --no-pager --no-banner [--non-interactive]
```

- Arguments are built as `Vec<String>`; no shell interpolation for user input.
- Version-like inputs are validated before use:
  - exact: `^\d{4}\.\d+\.\d+[abfp]\d+$`
  - stream: `^\d+(\.\d+)?$`
  - aliases: `latest`, `lts`, `default`
  - CLI versions: `^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$`
- Envelope: `{ success, command, data, errors[], warnings[], notifications[] }`.
- Listing shapes are normalized: `data` may be an array, or an object with `items`,
  `instances`, or `results`. Actual beta.14 listings use plain arrays; `status` and
  `editors running` use `{count, instances}`.
- Failure precedence: `errors[0].code` from stdout, then stderr, then process status.
- `retryable` follows exit codes `7, 9, 10, 12, 14, 130, 143`.
- `STATUS_NO_INSTANCES` (exit 12) is a normal "no Editor running" state, not a UI error.

### Resolving the binary

Order:
1. User override saved in the app config directory.
2. Unity install receipt:
   - Linux: `$XDG_CONFIG_HOME/unityhub/cli-install.json` or `~/.config/unityhub/...`
   - macOS: `~/Library/Application Support/UnityHub/cli-install.json`
   - Windows: `%APPDATA%\UnityHub\cli-install.json`
3. `$UNITY_CLI_HOME/bin/unity[.exe]`.
4. Platform defaults: `~/.local/bin/unity` (Linux), `~/.unity/bin/unity` (macOS),
   `%LOCALAPPDATA%\Unity\bin\unity.exe` (Windows).
5. `PATH` search.
6. Bare `unity` (so a clear spawn error can be shown).

## 6. Data model (normalized)

- `CliStatus { installed, path, version, channel, platform, arch, runtime, commit,
  buildDate }`
- `CliUpdateInfo { currentVersion, latestVersion, platform, updateAvailable }`
- `Editor { version, alias, architecture, location, moduleNames[], default }`
- `Release { version, architecture, stream, lts, installed, revisionHash }`
- `Module { id, name, category, status, downloadBytes, installedBytes, isInstalled,
  aliases }`
- `Project { title, path, version, architecture, changeset, isFavorite, cloudEnabled,
  vcsProvider, renderPipeline, buildTarget, lastModified }`
- `EditorInfo { version, changeset, stream, releaseDate, releaseNotesUrl, downloadUrl,
  modules[] }`
- `Changelog { version, sections[], breakingChanges[], markdown, source, generatedAt }`
  - `ChangelogSection { title, entries[] }`
  - `BreakingChange { text, reason, section }`

## 7. Screens and flows

**Shell:** sidebar navigation (Overview, Editors, Projects, Changelog, Settings),
header with CLI version badge and update indicator.

**Overview**
- CLI status card: installed/not installed, version, channel, platform, path.
- Update card: `self-update --check`; "Update CLI" starts a job.
- No CLI: explain and offer "Install Unity CLI" (explicit consent modal, then job).
- Auth/license diagnostic line (`auth status`, `license status`), read-only.
- Running Editors count (`editors running`).

**Editors**
- Table of installed Editors: version, architecture, default, modules, path.
- Detail panel: modules from `editors --installed --long`; "Manage modules" opens the
  module picker for that Editor (`modules list <version>`, `install-modules`).
- Releases table (`releases`) with stream/LTS/installed badges.
- "Install Editor" wizard: version input with alias suggestions (`lts`, `latest`, exact),
  module checkboxes loaded from `modules list <version>`, EULA checkbox, dry-run preview,
  then install job.
- Uninstall is intentionally omitted from v1 (requires confirmation and can be
  destructive); the CLI command remains available in docs.

**Projects**
- Table: title, Editor version, path, render pipeline, build target, cloud/VCS badges,
  last modified.
- Search filter.
- "Change version" dialog: target Editor dropdown (installed Editors only), warning that
  the CLI will upgrade the project and may open the Editor, explicit confirm. Runs
  `projects upgrade <path> --to <version> -y` as a job. `--wait` is deliberately
  omitted so the job ends when the upgrade does, not when the Editor is closed.
- Refresh button; `projects info` is not required in v1.

**Changelog**
- Button "Show changelog" loads `unity changelog --format json`.
- Version selector: installed CLI version and (if an update exists) the target version
  via `changelog --target`.
- Rendered Markdown-ish sections with badges: Added / Changed / Removed / Fixed /
  Security.
- "Possible breaking changes" panel: entries flagged by heuristics, with reason.
- Source label (`unity changelog`, installed version) and a note that the view is
  provider-based and will accept a remote API later.
- Raw Markdown toggle + copy button.

**Settings**
- CLI path override (manual), reveal config directory, "Re-check CLI".
- About: app version, architecture, links to CLI docs/release notes.

## 8. Jobs

- `JobManager` keeps `jobId -> { pid, kind, startedAt }` in a mutex.
- Start commands return `{ jobId }` immediately; UI subscribes to `job:update`.
- Frames:
  ```
  { jobId, kind, state, stream?, line?, progress?, message?, result?, error?, at }
  ```
  `state`: `started | running | succeeded | failed | cancelled`.
- stdout lines are parsed as JSON when possible:
  - `type: "result"` → final result.
  - progress fields (`progress`, `percent`, `percentage`, `current/total`,
    `downloadedBytes/totalBytes`) → progress 0–100.
  - `message`/`stage`/`status` → status text.
  - Unknown frames are preserved as raw lines.
- stderr is streamed as raw lines and included in failure detail.
- Cancel sends `SIGTERM` then `SIGKILL` on Unix; `taskkill /T /F` on Windows. The UI warns
  that an interrupted install may leave a partial download and suggests `--resume` via a
  retry.
- Jobs are not persisted across app restarts. Closing the window cancels running jobs.

## 9. Changelog and breaking-change design

`ChangelogProvider` trait:

```rust
trait ChangelogProvider {
    fn source_id(&self) -> &'static str;
    fn fetch(&self, req: ChangelogRequest) -> Result<Changelog, UnityError>;
}
```

v1 provider: `CliChangelogProvider` using `unity changelog --format json [--target]`.

Parsing:
- Markdown `### Added/Changed/Removed/Fixed/Security/Deprecated/Known Issues` headings
  become sections. Top-level `##` version headings become release boundaries (the CLI
  returns one version's notes today, but newer output may include more).
- Entries are top-level `-` list items; wrapped continuation lines are joined.
- Breaking heuristics (tuned against beta.14: 22 candidates out of 344 entries):
  - Section `Removed` → breaking (reason: "removed functionality").
  - Section `Deprecated` → breaking (reason: "deprecated functionality").
  - Other sections → only wording that signals removal, rename, incompatibility, or
    dropped/unsupported behavior. Ordinary `Changed` entries are not blanket-flagged.
  - `Fixed` and `Security` entries only flag explicit removal/rename wording, so a
    bug fix that says a problem "no longer" happens is not mislabeled.
- Each flag records section, text, and reason; duplicates are collapsed.
- `ChangelogProvider` is a real trait with `CliChangelogProvider` as the v1
  implementation. A future `RemoteChangelogProvider` (HTTP JSON or docs feed) can be
  selected without touching UI code. The frontend shows the active source id.

## 10. CLI bootstrap and self-update

- Install commands exactly as requested:
  - Linux/macOS: `bash -lc "curl -fsSL https://unity.com/install.sh | bash"`
  - Windows: `powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://unity.com/install.ps1 | iex"`
- Environment for bootstrap: `UNITY_CLI_SKIP_SETUP=1` to prevent the installer's
  interactive agent setup from hanging under the GUI; `UNITY_CLI_CHANNEL` passes through
  if set.
- The exact command is shown in a consent modal before running.
- After install, the backend re-resolves the binary (receipt/default paths) without
  requiring an app restart.
- Self-update uses `unity self-update --check --format json` and
  `unity self-update --yes --format ndjson` (job).

## 11. Security and safety

- No secrets are collected or stored.
- No remote content is loaded into the webview; the changelog is plain text rendered as
  React elements, never `dangerouslySetInnerHTML`.
- User-provided versions are regex-validated; paths are passed as single arguments after
  canonicalization where they exist.
- EULA acceptance is never implied: `--accept-eula` is passed only when the user checks
  the box in the install wizard. The modal spells out what it means.
- Destructive actions (project upgrade, CLI install) require explicit confirmation.
- The app inherits the CLI's proxy settings; no proxy credentials are handled.

## 12. Testing and verification

Rust:
- Unit tests for envelope parsing (array vs `{items}` vs `{instances}`), error mapping,
  version validation, changelog section parsing, and breaking-change heuristics.
- `cargo test` and `cargo clippy` (warnings allowed only if pre-existing/dependency).

Frontend:
- `pnpm build` (TypeScript + Vite) must pass.
- A small `vitest` suite for the changelog rendering helpers is optional; v1 relies on
  type-checking and manual UI smoke tests.

Integration:
- `pnpm tauri build --no-bundle` must succeed on Linux.
- Manual smoke test against the real CLI: Overview, Editors, Projects, Changelog load
  with real data; no install/upgrade is executed during verification.

## 13. Future work

- `RemoteChangelogProvider` over a JSON endpoint, with ETag caching and offline fallback.
- Editor uninstall + disk usage (`editors prune`, `projects size`).
- Running Editor control (`status`, open/close) and live logs.
- Job history and resumable installs.
- Windows/macOS CI packaging and installer signing.
