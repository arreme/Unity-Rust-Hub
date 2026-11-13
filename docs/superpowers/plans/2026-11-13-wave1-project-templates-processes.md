# Wave 1: New Project, Templates, and Processes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the first Wave 1 slice — a Templates page (catalog plus create/pack), a New Project wizard (local create plus preflight/failure states), and a read-only Processes page (merged list plus optional metrics).

**Architecture:** The existing one-way contract is preserved: `unity-core` gains typed template/project/status DTOs and command specs; `unity-hub-model` gains the page view models and actions; `unity-hub-controller` owns state, reads, jobs, validation, and mapping; `unity-hub-ui` draws the snapshot and emits actions. Streaming writes use `JobManager`; reads use the serialized `run_blocking` worker. Processes sampling is a page-scoped subscription that stops when the page is left.

**Tech Stack:** Rust 2021, iced 0.14, serde/serde_json, `sysinfo` for opt-in process metrics.

**Spec:** GitHub issues #11, #15, #16 and their tickets #20–#25. Live CLI 1.0.0 shapes were verified on 2026-11-13.

## Global Constraints

- The app never passes `--git-token`; only `--git-token-from-stdin` may be used, from a later VCS slice.
- `projects create` always receives `--no-cloud` and `--output-dir` (never `--path`).
- `--open` is passed only when the user asked to open.
- Built-in `com.unity.template.3d` / `com.unity.template.2d` templates are hidden unless the user enables "Show built-in templates".
- No mutation before the final confirm. Create/pack are streamed jobs.
- Token/binary discipline: no new dependency in `unity-core`, `unity-hub-model`, or `unity-hub-ui`. `sysinfo` goes in the controller only.
- Every fixture renders in `cargo test -p unity-hub-ui`; the gallery has no CLI dependency.
- All commands keep `--format`, `--no-pager`, `--no-banner`, `--non-interactive`, and `DO_NOT_TRACK=1`.
- Standard gates: `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`; the slim build (`--no-default-features`) still compiles.

## Review Focus

- A template list with a missing `renderPipeline` (Universal 2D) must not be dropped or shown as an error.
- A `projects create` result with a failed `steps[]` entry must keep the created path visible and name the failed step.
- A `status` read that exits 12 with `STATUS_NO_INSTANCES` is the normal "no Editor" state, not a page failure.
- Leaving the Processes page must stop sampling; the first CPU sample must be a placeholder, never 0%.
- A pack destination inside the source project, or an existing archive without confirmation, must be refused before the CLI runs.

---

## Task 1: Core template and project-creation types

**Files:**
- Modify: `crates/unity-core/src/models.rs`
- Modify: `crates/unity-core/src/versions.rs`
- Test: inline modules in both files

**Interfaces:**
- Produces: `TemplateBrief`, `TemplateInfo`, `TemplatePackage`, `TemplateVersion`, `CreateProjectResult`, `CreateProjectStep`, `TemplateFilter`, `TemplateCreateOptions`, `TemplatePackOptions`, `CreateProjectOptions`.
- Produces: `validate_template_id(&str)`, `validate_project_name(&str)`, `validate_package_id(&str)`, `validate_archive_path(&Path)`.

- [ ] Add serde structs matching the live CLI: brief rows (`name`, `displayName`, `type`, `version`, `status`), info (`description`, `renderPipeline?`, `packages[]`, `previousVersions[]`, `isUserGenerated`, `status`), and the create result (`path`, `version`, `architecture`, `template`, `opened`, `steps[]`).
- [ ] Add validators that reject path traversal, separators, flag-like values, and empty names.
- [ ] Add tests for blank/missing `renderPipeline` and for each validator.

## Task 2: Core template, status, and create command specs

**Files:**
- Modify: `crates/unity-core/src/runner.rs`
- Modify: `crates/unity-core/src/models.rs` (`RunningEditor` gains `state`, `frame_count`, `player_loop_ticking`)
- Test: inline `runner::tests`

**Interfaces:**
- Produces: `UnityCli::templates(version, filter, installed_only) -> Result<Vec<TemplateBrief>>`
- Produces: `UnityCli::template_info(version, name) -> Result<TemplateInfo>`
- Produces: `UnityCli::templates_create_spec(options) -> Result<CommandSpec>`
- Produces: `UnityCli::templates_pack_spec(options) -> Result<CommandSpec>`
- Produces: `UnityCli::create_project_spec(options) -> Result<CommandSpec>`
- Produces: `UnityCli::status_instances() -> Result<Vec<StatusInstance>>`
- Produces: `UnityCli::editors_running()` now merges `unity status` by PID then project path.

- [ ] Add `templates` and `template_info` reads using `parse_items` / `parse_data`.
- [ ] Add specs: `templates create <path> --name … --display-name … [--description] [--version] [--output-dir] [--overwrite]`, `templates pack <path> --output <file> …`, `projects create <name> --output-dir … --editor-version … --template … --architecture … --no-cloud [--open]`.
- [ ] Parse `status` with `execute_envelope` so exit 12 + `STATUS_NO_INSTANCES` becomes an empty list.
- [ ] Merge status rows into running Editors by PID first, path second; keep stale `isRunning == false` rows out.
- [ ] Tests: exact argv for each spec, `--no-cloud` always present, `--open` only when asked, status-exit-12 empty, merge precedence.

## Task 3: Model contract for the three pages

**Files:**
- Modify: `crates/unity-hub-model/src/view.rs`
- Modify: `crates/unity-hub-model/src/action.rs`

**Interfaces:**
- Produces: `Page::{Templates, Processes}`; `PageVm::{Templates(TemplatesVm), Processes(ProcessesVm)}`.
- Produces: `TemplatesVm`, `TemplateRowVm`, `TemplateDetailVm`, `TemplateAuthoringVm`, `TemplateFormVm`.
- Produces: `ProcessesVm`, `ProcessRowVm`, `ProcessMetricsVm`.
- Produces: `NewProjectVm`, `NewProjectStep`, `NewProjectFormVm`, `NewProjectResultVm`.
- Produces: `ModalVm::{NewProject, TemplateAuthoring}`.
- Produces: actions for navigation already covered, plus filter/editor/detail/authoring/wizard/process-metrics/copy actions.

- [ ] Add the page variants and sidebar `Page::ALL`.
- [ ] Add view models with `LoadState` for each independently loaded section.
- [ ] Add actions carrying stable ids (template name, project path) not indices.
- [ ] Add model tests for `Page::ALL` and action cloning.

## Task 4: Controller reads, jobs, validation, and mapping

**Files:**
- Modify: `crates/unity-hub-controller/src/app.rs`
- Modify: `crates/unity-hub-controller/src/jobs.rs` (`ActiveJob.result: Option<Value>`)
- Create: `crates/unity-hub-controller/src/metrics.rs`
- Modify: `crates/unity-hub-controller/src/settings.rs` (`process_metrics: bool`)
- Modify: `crates/unity-hub-controller/Cargo.toml`

**Interfaces:**
- Consumes: Task 2 specs and Task 3 view models.
- Produces: `Message::{TemplatesLoaded, TemplateInfoLoaded, TemplatesEditorChanged, TemplateKindChanged, TemplateInstalledToggled, OpenTemplateDetail, OpenTemplateAuthoring, TemplateFormChanged, StartTemplateJob, ...}`.
- Produces: `Message::{OpenNewProject, WizardFieldChanged, WizardStepChanged, WizardTemplateSelected, WizardPreflightLoaded, StartCreateProject, ...}`.
- Produces: `Message::{ProcessesLoaded, ProcessTick, ProcessMetricsToggled, CopyProcessValue}`.
- Produces: `metrics::Sampler` with `sample(&mut self, pids) -> Vec<ProcessMetrics>`; first sample CPU is `None`.

- [ ] Add page fields, loaders, `ensure_page`/`refresh_page` arms.
- [ ] Add template catalog/detail reads and the create/pack job specs with pre-CLI validation and overwrite confirmation.
- [ ] Add the wizard state machine: fields → preflight (license + editors + name/folder) → template → review → job → result; preserve fields on failure.
- [ ] Retain the terminal job `result` and parse the created path / failed `steps[]` / template archive path.
- [ ] Add the Processes merged read and page-scoped sampling. Use `sysinfo` only in the controller.
- [ ] Settings: persist `processMetrics`, default false.
- [ ] Tests: validation, spec args, partial success, status empty state, sampler start/stop + two-sample CPU.

## Task 5: UI pages, wizard, template authoring, and gallery

**Files:**
- Create: `crates/unity-hub-ui/src/pages/templates.rs`
- Create: `crates/unity-hub-ui/src/pages/processes.rs`
- Modify: `crates/unity-hub-ui/src/pages/mod.rs`
- Modify: `crates/unity-hub-ui/src/shell.rs` (labels, icons, nav order)
- Modify: `crates/unity-hub-ui/src/modals.rs` (wizard + authoring)
- Modify: `crates/unity-hub-ui/src/fixtures.rs`
- Modify: `crates/unity-hub-ui/src/icons.rs` only if a new glyph is already in the subset

**Interfaces:**
- Consumes: Task 3 view models/actions.
- Produces: Template catalog with editor picker, core/custom/all + installed-only filters, detail pane, authoring buttons; Processes table with state/phase/play/frame/safe-mode/dialog, copy/open actions, metrics toggle; New Project full-screen wizard; Template authoring full-screen form with overwrite confirmation.

- [ ] Build the Templates page with loading/empty/error/CLI-missing states.
- [ ] Build the Processes page with running/empty/pipeline-missing/safe-mode/dialog/unreachable states.
- [ ] Build the wizard steps and review command preview.
- [ ] Build template create/pack form and overwrite confirmation.
- [ ] Add fixtures for every new state and keep `every_fixture_renders` green.

## Task 6: Integration and verification

- [ ] Update the design contract page inventory and README feature list.
- [ ] `cargo fmt --all --check`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo build -p unity-hub --no-default-features`
- [ ] Live, non-mutating smoke: templates list/info + status; create/pack verified against a temp probe project.
- [ ] Record the metrics binary-size delta.

## Scope note

The GitHub dependency graph marks #20/#24 as blocked by the notification centre (#18) and #22 by #19. This slice deliberately keeps the existing notice banner and job card so the three feature pages can land independently; migrating notices into the notification rail is the follow-up #18/#19 slice. No acceptance criterion in #20–#25 requires the rail.

## Verification record (2026-11-13)

- `cargo fmt --all --check`: pass.
- `cargo test --workspace`: 67 core + 36 controller + 3 UI tests pass; the ignored live test passes with the real CLI 1.0.0.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass.
- `cargo build -p unity-hub --no-default-features`: pass (debug).
- Live reads: `templates --brief` (8 core templates), `templates info` (URP pipeline; Universal 2D reports no `renderPipeline`), and `status` (exit 12 + `STATUS_NO_INSTANCES` becomes empty).
- Live writes: `projects create` and `templates pack` were exercised against a temp probe project. The pack result frame is `data.path`; the create result frame is `data.path` + `data.steps[]`.
- Release binary sizes on Windows x64: GPU build 14,394,880 bytes; slim (`--no-default-features`) build 8,138,240 bytes. `sysinfo` 0.33.1 is linked only by `unity-hub-controller`; these totals include it.
- Gallery: all 29 fixtures rendered and captured without a CLI.

