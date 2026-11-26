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

- Sidebar navigation (9 destinations: Overview, Editors, Projects, Templates,
  Processes, Skills, Diagnostics, Changelog, Settings), active state.
- Sidebar footer: CLI status (checking / connected with version / missing).
- Page header: title, subtitle, and actions (Refresh on Overview, Editors,
  Projects; Refresh plus Copy report on Diagnostics).
- Sidebar notifications affordance: unread badge, collapsed by default.
- Notification rail: unread, empty, overflow, long message, dismissed, and one
  item per action state (action available, confirming, running, failed). A
  dismissed entry disappears from the rail, and the gallery covers that
  post-dismiss state. The shared scroll reserves the scrollbar's painted lane,
  so cards never run under the thumb.
  Advisory entries come from the CLI's `notifications[]` and `warnings[]` on any
  read or streamed job frame: a CLI update notice keeps its code as the label
  and shows the exact remediation command with Copy, a plugin beta warning shows
  the CLI's text verbatim, and unknown codes keep the CLI's own message.
- Toast: inline above the page, auto-expires after about five seconds,
  dismissible. A notice (a copy confirmation, a settings error) borrows the same
  slot, so there is exactly one transient surface.
- Job card: running with progress, succeeded, failed with the shared error
  rendering (stable code, CLI message, detail, and a non-retry action when the
  mapping has one), cancelled, with collapsible-looking log area. A 1.1.0
  parallel install reports `downloads[]`; the job keeps one percentage per
  `sha256` and shows their mean, clamped to a monotonic high-water mark, so a
  frame for the second file never pushes the bar back to 0.
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
- CLI update flow (inside the CLI card): channel picker (stable/beta), target
  version input (empty = newest on the channel), **Load release notes**,
  **Preview (--dry-run)**, **Apply update**, and **Roll back…**. The target
  release notes are shown before Apply is enabled. The dry-run preview is a
  read. A failed preview or changelog read keeps its own Retry.
- Account card: checking sign-in, signed in, signed out, expired session,
  sign-in error; license active/inactive, no licenses, license read error.
- Running Editors: loading, none, one/many, read error.

### Editors

- Installed Editors: loading, error, empty, one/many, default badge, long module
  list. One row is one installed build; two builds of the same version show as
  two rows with their architecture. Each row offers Manage modules and Set
  default (or Clear default when it is the stored default).
- The architecture picker samples: Host default, `x86_64`, `arm64`. Host default
  is the only preselection and keeps `--architecture` out of the command.
- Install a new Editor: version input, architecture picker next to it, module
  catalog loading/error/empty/loaded for the chosen build, select all/clear,
  headless option on Linux with the CLI's availability note, EULA, preview
  loading/ready/raw fallback with the resolved architecture, second-build
  warning, install enabled and disabled.
- Advanced install area (collapsed by default): `--resume`, `--download-only`,
  `--no-wait`, `--timeout <seconds>`, `--set-default`, plus the
  `UNITY_INSTALL_RETRIES` count. The exact `unity install` command is shown
  below. No-wait and timeout clear each other with an explanation of the lock
  conflict; resume, download-only, and set-default clear the combinations the
  CLI refuses with `BAD_ARGS`. Invalid timeout/retry text disables install and
  shows why.
- Resume offer: after a failed/interrupted install whose CLI result reports a
  cache or an interruption class, a warning card names the version and build,
  explains that the cache is reused, and offers **Resume install**. It is
  consumed when clicked; the resumed command keeps the original build, modules,
  and EULA but never grows download-only, set-default, no-wait, or timeout.
- Download-only report: a success card names the folder the CLI reported under
  `data.path`, says nothing was installed or registered, and does not add an
  installed row.
- Available releases: loading, error, empty, installed badge (matched on
  version and architecture), use action that selects the build, stream tones.
- Maintenance card: last prune report summary (unused count and reclaimable
  size from the CLI) and a **Review unused Editors** action that opens the scan
  dialog. It never scans or removes on its own.
- Editor verify: each row has a Verify action for exactly that
  `(version, architecture)` build. The dialog shows pass/warn/fail rows, the
  failing path, the CLI's repair command, and the raw log when the verify
  fails. A stale row action for a build that is gone does nothing.

### Projects

- Search field and count, plus an Add project action that opens the native
  folder picker and registers the chosen folder with `unity projects add`.
- Loading, error, no projects registered, no matching projects, populated table.
- Each row names its Editor version and architecture (when the CLI reports one).
- Badges: Unity Cloud, VCS, Editor not installed, local project; favorites are
  the star toggle beside the title (CLI pin/unpin), not a badge.
  "Editor not installed" matches both version and architecture; an older row
  with no architecture falls back to a version match.
- Pipeline badge or empty, long title, long path, modified date.
- Row actions: favorite star, Open, Change version, Remove (confirmation modal;
  files on disk are untouched).

### Templates

- Editor picker, All/Core/Custom filters, installed-only toggle.
- Loading, empty filter, error-with-retry, no-Editor, and CLI-missing states.
- Row: display name, package id, render pipeline, version, status.
- Detail: description, versions, installed state, packages.

### Processes

- Merged read-only list: project name/path, Editor version, PID, port, state,
  phase, Play mode, frame count, Safe Mode, and blocking dialog.
- No Editor running, Pipeline missing, unreachable instance, Safe Mode, and
  blocking-dialog states.
- Optional CPU/memory/uptime columns; the first CPU sample is unknown.
- Copy PID/port/path and Open folder actions.

### Skills

- Page header: title/subtitle and Reload (re-reads both lists).
- Supported clients card (`unity skill install --list`): one row per AI client
  with its display label, client id, install path, installed/absent badge, and
  an Install button for that client. Loading, error + Retry, empty, and loaded.
- Tracked installs card (`unity skills`): one row per CLI-tracked copy with the
  skill name, client badge, path, and the CLI's status. Loading, error + Retry,
  empty, and loaded.
- Install options card: `--local` (only when a project context is known, with an
  explanation when it is not), `--yes` (replace an outdated copy), `--dry-run`,
  and the optional `--skill <name>` field with its validation line. The exact
  `unity skill install --all` command is shown above the Install to all clients
  button.
- Refresh card: the dry-run default is stated, the exact `unity skill refresh`
  command is shown, and the primary button is "Preview refresh (dry run)" until
  a preview lands, then "Refresh now". Preview again stays available.
- Last run card: one row per tracked install, in CLI order, with the CLI's
  outcome word (`refreshed`, `up-to-date`, `pruned`, `skipped`, `conflict`, or
  `would-update`/`would-skip`/`would-prune` under `--dry-run`) and the
  `skipReason` when present. An unknown outcome word is shown verbatim.
- Conflict offer: after a `conflict` row, a danger callout explains that the CLI
  did not write that copy and shows the exact `--force` command. The Take
  ownership button exists only after that explanation.
- A running skill job disables every install/refresh/force control; the shell
  job card owns progress, the raw NDJSON log, and Cancel.

### Diagnostics

- Page header: title/subtitle, Refresh, and Copy report. Copy is disabled until
  at least one of doctor, environment, or proxy has data. Refresh re-runs the
  automatic section reads but never `unity doctor`.
- Every section has its own load state, so one slow or failed read never gates
  the others. Each failure renders the shared error treatment and its own Retry;
  Retry re-runs only that section.
- Doctor card (`unity doctor`): **opt-in**, because doctor's credential probe
  can trigger a polkit password prompt. The idle card explains that and offers
  Run doctor; a loaded card offers Refresh doctor. Platform/architecture,
  runtime, Node, CLI version, install path, log directory, auth/license
  summaries, and the check list. Checks use pass/warn/fail/info/unknown
  styling, a humanized label from the check `id` (CLI message, then
  `messageKey`, then a generic label as fallbacks), and a detail line built
  from the CLI's `params`. A support-safe raw report is visible in a scrollable
  inset: known credential fields are replaced with `***` and proxy URLs lose
  their user-info. Loading, error + Retry, empty, and loaded.
- Environment card (`unity env`): user data path, Editor install path, download
  cache, config path, CLI and Hub versions, plus the resolved proxy URL, source,
  auth source, and no-proxy list. A missing proxy section is stated explicitly.
  Loading, error + Retry, and loaded.
- Proxy, Accelerator, and Update cards (`unity diagnose <topic>`): the CLI's
  redacted report as preformatted, scrollable text with a per-report Copy.
  Loading, error + Retry, empty, and loaded states.
- Copy report writes the combined plain-text doctor + environment + proxy
  report. A failed section is named as unavailable rather than omitted. The
  copied doctor document is the same sanitized raw report the card shows. An
  empty report or a failed clipboard write (the OS clipboard reports a real
  result) becomes an error notice; success is shown only after the write
  succeeds.
- No mutation controls appear on this screen.

### Changelog

- Not loaded, loading, error, no data.
- Header with version/source/entry count.
- Filter bar: "Affects this app (N)" off by default; the count is exactly the
  entries the filter keeps. Docs scope picker (Automatic, script reference,
  manual, CLI, Hub, Cloud, package) with a package-name field for the package
  scope.
- No breaking changes panel; breaking changes list with section/reason badges
  (unchanged generic heuristic).
- Section cards for Removed/Changed/Added/Fixed/Security/Other. When the filter
  is on, only impactful entries render and an empty filter shows a quiet notice.
- Per-entry docs line: lookup action, "Finding docs…", resolved URL with an
  Open docs action (search results carry a Search badge), "No docs page found",
  and a quiet failure message. The entry text always stays visible.
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
- Project upgrade: current version/path, target picker, no target installed
  warning, confirmation, disabled/enabled upgrade; a target with two installed
  builds is explained, not silently resolved.
- Remove project: path, "files on disk are not deleted" warning, cancel/remove.
- New project wizard: details, Editor/architecture (including no-Editor state),
  template (with built-ins hidden by default), review with command preview,
  license preflight, and partial-success result.
- Template authoring: create and pack forms, validation issues, overwrite
  confirmation, command preview, job card, and result path.
- Editor modules: installed module list for one build (version + architecture),
  available module loading/error/empty/loaded, selection count, EULA,
  `--retries <n>` (0 disables), optional portable `--download-directory`, the
  exact command, disabled/enabled install.
- CLI rollback: names the kept version being restored (or "the most recent kept
  CLI version" when the field is empty), the exact rollback command, and a
  confirmation checkbox. The command contains only `--rollback [version]`
  plus `--yes`; no check/changelog/target/channel/dry-run flag is ever added.
  A `BAD_ARGS` job failure keeps the dialog open and renders the code plus its
  copy-the-command action.
- Project upgrade: current version/path with its architecture, target version
  picker (one entry per installed version, because the CLI upgrades by version),
  a note naming any target version with two installed builds, no target
  installed warning, confirmation, disabled/enabled upgrade.
- Prune review: the report-only scan command, unused candidates (version,
  architecture, size when the CLI reports one, CLI reason, path), and the
  reclaimable total. A confirmation checkbox enumerates every build to delete
  before the primary danger button enables. `--force` appears only after a
  remove run failed because the running-Editor check could not complete.
  While the removal job runs, the dialog shows a "Removing…" line and points at
  the job card for the raw log and Cancel. Cancelling removes nothing.
- Prune missing paths: a separate outlined block with an info notice stating
  that no files are deleted, its own candidate list, confirmation, and registry
  only button. It never implies `--remove`.
- Verify results: one row per checked component with pass/warn/fail/unknown
  badges, the exact failing path, a per-component repair command, the CLI
  verdict summary, and a raw-log toggle whenever the verify failed.

## Changelog docs and app-impact contract

`ChangelogSectionVm.entries` is `Vec<ChangelogEntryVm>`:

- `ChangelogEntryVm { text, app_impact, docs: EntryDocsVm }`.
- `EntryDocsVm` is `Unavailable`, `Idle { topic, scope }`,
  `Loading { topic }`, `Ready { topic, url, search }`,
  `Missing { topic }`, or `Failed { topic, message }`. Every non-`Unavailable`
  state keeps the entry text rendered; `Missing` says "No docs page found" and
  `Failed` is a quiet inline line.
- `ChangelogDataVm::app_impact_count` is computed from the same
  `ChangelogEntryVm::app_impact` flags the cards filter on, so the checkbox
  count and the visible list cannot disagree.
- `ChangelogVm` adds `app_impact_only: bool` (default false),
  `docs_scope: DocsScopeVm` (default `Automatic`), `docs_package: String`, and
  `docs_scope_note: Option<String>`.

Actions: `ToggleChangelogImpact(bool)`, `ChangelogDocsScopeChanged(DocsScopeVm)`,
`ChangelogDocsPackageChanged(String)`,
`LookupChangelogDocs { topic, scope }`, and `OpenExternalUrl(String)`.

The adapter side is `unity-core::changelog` plus `unity-core::docs`:

- Entry parsing keeps the raw Markdown just long enough to extract backticked
  candidates. A `unity <command> [<subcommand>]` sequence wins; otherwise a
  class-like identifier (`MonoBehaviour.Start`) is the topic. Flags and
  `<placeholders>` are stripped.
- `unity-core::surface` is the single command/flag/error-code registry. Command
  builders take their tokens from it, and `app_impact` is true only when an
  entry names one of those commands, a flag the app passes, or a mapped
  error/exit code. Do not add a second list in the controller or UI.
- `surface::EXIT_CODES` is the one exit-code table (`name`, `message`,
  `retryable`, `remediation_code`). `code_for_exit_code`,
  `message_for_exit_code`, `is_retryable_exit_code` and `is_mapped_exit_code`
  project it, so adding a code is one row. Exit `11` stays unmapped because
  1.1.0 moved several different not-found failures onto it.
- `command_path` resolves a backticked `unity <command> [<subcommand>]`. A
  second token outside the registered subcommands is an argument only when the
  command declares one (`docs <topic>`) or has no subcommands; otherwise it is
  an unknown subtree and returns `None`, so `pipeline automation` never counts
  as the app's `pipeline editors` / `cloud-build` surface. `CLI_CODES` lists
  recognised codes that have no dedicated remediation; `ERROR_CODES` is the
  remediated subset.
- `UnityCli::docs_spec` always passes `--url`, so the CLI never opens a
  browser. `--manual`, `--cli`, `--hub`, `--cloud`, and `--package <name>` are
  optional scopes; no scope means the script reference. A search URL is a
  normal `Ready` result with `search: true`.
- Resolved URLs are cached for the session by topic + loaded changelog version
  + resolved scope. A `DOCS_NOT_FOUND` answer maps to `Missing`; any other error
  maps to `Failed`. Lookups run on the serialized reader worker and never block
  a frame.
- `OpenExternalUrl` is handled only by the controller, validates HTTP(S), and
  uses a platform opener (`cmd /C start`, `open`, or `xdg-open`/`gio`/
  `sensible-browser`). No lookup ever launches a browser.

Gallery fixtures: `Changelog`, `Changelog · filter on`,
`Changelog · docs action`, and `Changelog · docs failure`.

## Editor maintenance contract

`EditorsVm::prune: LoadState<PruneReportVm>` is the page's last report.
`PruneReportVm { unused: Vec<PruneCandidateVm>, missing: Vec<PruneCandidateVm>,
reclaimable, removed }`; `PruneCandidateVm { version, architecture, path,
size: Option<String>, reason }`. `size` and `reclaimable` are formatted from
the CLI's `bytes`/`total`; `reason` is the CLI's `reason`/non-status text, or
the command's own classification when the CLI omits it. Nothing is computed
from the project registry locally.

`ModalVm::PruneEditors(PruneEditorsVm)` carries the report plus
`confirmed`, `missing_confirmed`, `force`, `force_available`, the three exact
command previews (`scan_command`, `remove_command`, `missing_command`), labels
that name the count and total, confirmation labels that enumerate every
candidate, `can_remove`, `can_remove_missing`, the failed-run `error`, and
`job_running`.

Counts, labels, and confirmations are derived only from
`LoadState::Ready`. A loading or failed report keeps its previous data for
display but renders no confirmation, missing section, or danger button; the
controller also rejects confirmations and both remove actions until the report
is Ready again. `job_running` renders an in-dialog "Removing…" line that points
at the job card, and both removal buttons stay disabled.

`ModalVm::VerifyEditor(VerifyEditorVm)` carries the build, the exact
`unity editors verify` `command`, and `LoadState<VerifyResultVm>`;
`VerifyResultVm { version, architecture, ok, checks, failed_count,
repair_command, raw }` and `VerifyCheckVm { component, kind, status:
CheckStatus, path, repair_command }`. `ok` is the CLI's own verdict; `raw` is
its full JSON envelope.

Actions: `OpenPrune` (open + report scan), `PruneScan` (report scan only),
`PruneConfirmed(bool)`, `PruneMissingConfirmed(bool)`, `PruneForceToggled(bool)`,
`StartPruneRemove` (streamed `--remove --yes [--force]`),
`StartPruneRemoveMissing` (streamed `--remove-missing --yes`, registry only),
`OpenVerify(version, architecture)`, and `ToggleVerifyRaw`.

Both remove commands are streamed jobs; their raw NDJSON log is the job card.
A job failure keeps the dialog open and only
`EDITORS_PRUNE_EDITOR_CHECK_FAILED` / `_UNAVAILABLE` enables `--force`.
Verify is a read with its own `read:editor-verify` retry target that repeats
the exact build.

Gallery fixtures: `Editors · default cleared`, `Modal · prune candidates`,
`Modal · prune loading`, `Modal · prune rescanning`, `Modal · prune failed`,
`Modal · prune empty`, `Modal · prune confirm`, `Modal · prune remove missing`,
`Modal · prune running`, `Modal · prune force offer`, `Modal · verify results`,
`Modal · verify loading`, `Modal · verify all passed`,
`Modal · verify failed read`.

## Install resilience and CLI rollback contract

`InstallEditorVm` carries `advanced: InstallAdvancedVm`, the exact
`command_preview`, `resume_offer: Option<ResumeInstallVm>`, and
`download_report: Option<String>`.

- `InstallAdvancedVm { expanded, resume, download_only, no_wait,
  timeout_input, set_default, retries_input, conflict_note, timeout_error,
  retries_error, can_install }`. All flags default off so the untouched command
  stays `unity install <version> --yes` plus the standard machine flags.
- `ResumeInstallVm { version, architecture, reason, can_resume }`. The
  controller only creates it after a failed/cancelled `editor-install` whose
  error code is an interruption/timeout/network class or whose terminal result
  reports `resumable`, `resumeAvailable`, `interrupted`, or a cache path.
  Download-only failures never offer it: `editors-install.md:363` documents
  that `--download-only` cannot be combined with `--resume`, and resuming as a
  normal install would break the "installs nothing" promise. The Advanced
  resume control still lets the user ask for a normal cached resume.
- `EditorModulesVm` adds `retries_input`, `download_directory`,
  `retries_error`, and `command_preview`; the adapter appends `--retries <n>`
  and `--download-directory <dir>` only when set.
- `OverviewVm::update_flow: CliUpdateFlowVm` carries the channel, target input,
  `LoadState<CliChangelogVm>`, `LoadState<String>` dry-run preview, the three
  exact command previews, `can_apply`, and `apply_blocked_reason`. `can_apply`
  is false until the target changelog is `Ready`; changing target or channel
  clears both reads.
- `ModalVm::RollbackCli(RollbackCliVm)` carries `version_input`,
  `restore_label`, `confirmed`, `can_rollback`, `command`, the failed-run
  `error`, and `job_running`.

Actions: `InstallAdvancedToggled`, `InstallResumeToggled`,
`InstallDownloadOnlyToggled`, `InstallNoWaitToggled`, `InstallTimeoutChanged`,
`InstallSetDefaultToggled`, `InstallRetriesChanged`, `ResumeInstallEditor`,
`DialogRetriesChanged`, `DialogDownloadDirectoryChanged`,
`CliUpdateChannelChanged`, `CliUpdateTargetChanged`, `LoadCliUpdateChangelog`,
`PreviewCliUpdate`, `StartCliUpdate`, `OpenCliRollback`,
`CliRollbackVersionChanged`, `CliRollbackConfirmed`, `StartCliRollback`.

The CLI builders live in `unity-core::runner`:
`install_editor_spec(..., &InstallOptions)`,
`install_modules_spec(..., &ModuleInstallOptions)`,
`self_update_check_on`, `self_update_changelog(_spec)`,
`self_update_dry_run(_spec)`, `self_update_apply_spec`, and
`self_update_rollback_spec`. `unity install` has no `--retries`, so the retry
count is passed only as the `UNITY_INSTALL_RETRIES` child environment;
`install-modules` uses the flag. Secrets never enter argv.

Gallery fixtures: `Editors · advanced install options`,
`Editors · resume offered`, `Editors · download-only finished`,
`Overview · CLI update flow`, `Modal · CLI rollback`, and
`Modal · CLI rollback BAD_ARGS`.

## Advisory contract

`ShellVm::advisories: Vec<AdvisoryVm>` is the single surface every CLI advisory
uses; later pages (#9 plugins) reuse it instead of drawing their own banners.

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

## Diagnostics contract

`DiagnosticsVm` carries one independent `LoadState` per section:

- `doctor: LoadState<DoctorVm>` — `DoctorVm { platform, arch, runtime_version,
  node_version, cli_version, install_path, log_directory, checks, auth, license,
  raw }`. `auth`, `license`, and `raw` are sanitized at the mapping boundary:
  credential-bearing keys become `***` and URL user-info is redacted before the
  document reaches the view model or the combined copy.
- `environment: LoadState<EnvironmentVm>` — `EnvironmentVm { user_data_path,
  editor_install_path, download_cache_path, config_path, cli_version,
  hub_version, proxy: Option<ProxyVm> }`.
- `proxy_report`, `accelerator_report`, `update_report:
  LoadState<DiagnoseReportVm>` — `DiagnoseReportVm { topic, text }`.
- `can_copy: bool` — true when doctor, environment, or proxy has data.

`DoctorCheckVm { id, label, status: CheckStatus, detail }`. `CheckStatus` is
`Pass | Warn | Fail | Info | Unknown`; unknown CLI statuses are shown neutrally
instead of guessed. `label` is never a raw localization key unless the key's
last segment is the only available text. `detail` renders `params` as
`Humanized key: value` pairs, preserving the CLI's values verbatim.

Actions: `UiAction::CopyDiagnosticsReport` (combined plain text),
`UiAction::CopyDiagnoseReport(DiagnoseTopic)` (one redacted report),
`UiAction::LoadDoctor` (explicitly runs or refreshes `unity doctor`), and
`UiAction::Refresh` (re-runs the automatic section reads; it does not run
doctor). Retry uses the same
`ErrorActionKind::Retry("read:doctor" | "read:environment" |
"read:diagnose-proxy" | "read:diagnose-accelerator" | "read:diagnose-update")`
targets as every other read.

Gallery fixtures: `Diagnostics`, `Diagnostics · not loaded`,
`Diagnostics · doctor not run`, `Diagnostics · loading`,
`Diagnostics · partial failure`, `Diagnostics · fully failed`.

## Skills contract

`SkillsVm` is the read-first surface for the Unity agent skill:

- `clients: LoadState<Vec<SkillClientVm>>` — `SkillClientVm { client, label,
  path, installed }` from `unity skill install --list`. `installed` accepts the
  CLI's boolean or its documented status words; an unrecognized status stays
  absent.
- `installs: LoadState<Vec<SkillInstallVm>>` — `SkillInstallVm { path, client,
  skill, status }` from `unity skills`, shown verbatim.
- `install: SkillInstallOptionsVm` — `local`, `overwrite` (`--yes`),
  `dry_run`, `skill_input` (`--skill`), `skill_input_error`, and the exact
  `all_command`.
- `result: Option<SkillRunVm>` — `SkillRunVm { dry_run, rows, force }`;
  `SkillOutcomeRowVm { path, client, skill, outcome, skip_reason }`.
  `SkillOutcomeVm` is `Refreshed | UpToDate | Pruned | Skipped | Conflict |
  WouldUpdate | WouldSkip | WouldPrune | Unknown(String)`. `Skipped` and
  `Conflict` are distinct tones and are never collapsed into success.
- `SkillForceVm { explanation, command, scope }` exists only when a result has a
  `conflict` row. The explanation states that the CLI did not write that copy
  before the `--force` button is drawn. `force_skill_last_run` repeats the check
  and refuses a stray or replayed `--force` action when the current result has no
  conflict row.
- `refresh_label`, `refresh_previewed`, `refresh_hint`, and `refresh_command`
  describe the next refresh click. The first click after opening the page is a
  `--dry-run`; entering the page resets the cycle, and after a preview lands the
  next click applies. `PreviewSkillRefresh` re-runs a preview at any time.
- A supported-client row whose CLI id is empty renders no Install button: there
  is no valid client token to send.
- `project_context`, `local_available`, `local_disabled_reason` gate `--local`.
  The context is the process working directory when it (or an ancestor) holds
  `ProjectSettings/ProjectVersion.txt`; without it the checkbox is replaced by
  an explanation.
- `job_running` disables every mutation control; the controller refuses a
  concurrent job.

Adapter surface (`unity-core::runner`):

- `skills()` reads `unity skills --format json` and parses `data.items[]`.
- `skill_clients()` reads `unity skill install --list --format json`.
- `skill_install_spec(&SkillInstallOptions)` builds
  `unity skill install [client|--all] [--local] [--skill <name>] [--yes]
  [--dry-run] [--force]` as an NDJSON job.
- `skill_refresh_spec(&SkillRefreshOptions)` builds
  `unity skill refresh [--yes] [--dry-run] [--force]` as an NDJSON job. The
  controller always passes `--yes` because the click is the confirmation and a
  child has no terminal.
- Every token is validated in `unity-core::versions` before it reaches argv:
  `validate_skill_client` accepts the documented client-id shape and
  `validate_skill_name` accepts a single skill token. The new `skills` command,
  `--list`, `--local`, and `--skill` are registered in
  `unity-core::surface`, so the changelog app-impact filter stays derived from
  the adapter.

Actions: `InstallSkill(client)`, `InstallAllSkills`, `SkillLocalToggled(bool)`,
`SkillOverwriteToggled(bool)`, `SkillDryRunToggled(bool)`,
`SkillNameChanged(String)`, `StartSkillRefresh`, `PreviewSkillRefresh`, and
`ForceSkillLastRun`. The controller keeps the exact request the last job ran, so
Force repeats that client/scope rather than whatever the form contains later.

Both reads use `read:skill-clients` and `read:skill-installs` retry targets. A
finished `skill-install`/`skill-refresh` job parses its terminal frame's
`data.items[]` (or the single-object `skipped: true` answer) and reloads both
reads, so the new status is visible without an app restart.

Gallery fixtures: `Skills`, `Skills · empty`, `Skills · installed`,
`Skills · dry run`, `Skills · conflict`, `Skills · loading`, `Skills · error`.

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
2. Exit `18`: the CLI's dialog signal wins over a streamed `EDITOR_TIMEOUT` code
   (1.1.0), so a blocked command or close always offers the dialog action.
3. The stable `code` table: auth codes → Sign in; `NETWORK_UNREACHABLE` and
   `VCS_PROVIDER_UNREACHABLE` → Retry; `INSTALL_LOCKED`, `TIMEOUT`,
   `EDITOR_TIMEOUT` → Retry; `RELEASE_NOT_FOUND` → Pick another version;
   `EDITOR_UNREACHABLE` → Open a project; `UNAVAILABLE` / `STATUS_SAFE_MODE` →
   Show status; `AWAITING_INPUT` / `EDITOR_BLOCKED_BY_DIALOG` /
   `PROJECTS_CLOSE_BLOCKED_BY_DIALOG` → Answer the Editor dialog;
   `INVALID_COMMAND_ARGS` → Show the command and name the proxy/PAC variable;
   `EDITOR_NOT_INSTALLED` → Install that version;
   `NOT_A_PROJECT` → Choose a project;
   `PROJECTS_REMOVE_NOT_FOUND` → Refresh the project list;
   `VCS_TOKEN_REQUIRED` → Enter a token (env var or `--git-token-from-stdin`,
   explicitly not a Unity sign-in); `GIT_START_FAILED` → Install git;
   `LOCAL_STORE_UNWRITABLE` → Fix permissions;
   `LINK_VCS_DELETE_REMOTE_REQUIRES_YES` → Confirm explicitly;
   `INSTALL_HEADLESS_UNAVAILABLE` → Pick another version or clear headless;
   `BAD_ARGS` → Show the command.
4. Exit-code semantics for unknown codes (`2`, `3`, `7`, `9`, `12`, `14`, `17`,
   `18`). Exit `11` is deliberately absent: 1.1.0 moved several different
   not-found failures onto it, so the code must name the target. Exit codes
   outside that table stay generic.

Retry is a controller-side target, not a closure in the view model:

- `UnityError::is_blocked_by_dialog()` keys on exit `18` or the three dialog
  codes. `close_editor_spec` / `close_project_spec` exist in the adapter but no
  Processes flow wires them yet; when it lands, `--force` is a separate explicit
  confirmation and a dialog-blocked close or command never offers Retry.

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
  as a notification.

## Design gallery

```bash
cargo run -p unity-hub-ui --bin unity-hub-design-gallery
```

The gallery renders every fixture without a Unity CLI and is the fastest way to
review loading/error/modal/job states against Figma. It covers the notification rail,
the inline toast, and the four notification action states without a CLI installed.
`cargo test -p unity-hub-ui` also constructs every fixture, so a broken screen fails
CI without opening a window.

Environment variables for review:

- `GALLERY_FIXTURE=<index>` opens a specific fixture.
- `GALLERY_FONT=Tahoma` renders every fixture with a system font family.
- `GALLERY_TEXT_SCALE=1.5` renders every fixture at a larger text size.
- `GALLERY_HEIGHT=<px>` opens a taller window.
- `GALLERY_SCREENSHOTS=<dir>` captures every fixture from inside the gallery
  window as raw RGBA (`NN-WxH.rgba`) and exits. Convert one with
  `magick -size WxH -depth 8 rgba:<file> out.png`.
