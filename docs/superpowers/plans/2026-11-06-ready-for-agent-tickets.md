# Plan — implement the ready-for-agent tickets

Date: 2026-11-06
Spec: the GitHub issues on `arreme/Unity-Rust-Hub` labelled `ready-for-agent`
(#2–#13). Each issue body is the spec for its task; the brief in
`.superpowers/sdd/2026-11-06-ready-for-agent-tickets/task-N-brief.md` carries the
issue text verbatim.

## Goal

Ship every open ticket behind one reviewable branch (`feat/ready-for-agent-tickets`),
one commit (or a small commit series) per ticket, with the repository's full
verification suite green after each ticket.

## Global constraints

- `AGENTS.md` is binding: layer rules, command construction, secret handling,
  controller conventions, tests, docs, commits.
- The CLI is the source of truth. Never compute locally what the CLI reports,
  never invent a mutation, never auto-run a command the user has not seen.
- No secrets in argv, logs, settings, or command previews.
- Every ticket adds gallery fixtures for its new states and updates
  `docs/design/contract.md` and the README feature list.
- No new dependencies unless the ticket requires one; prefer std/existing crates.
- Windows/macOS must still compile: no `cfg(unix)`-only API without a fallback
  (clipboard, file openers, path casing).
- Keep unrelated code untouched; no opportunistic refactors.
- Tests: unit tests for command construction and mapping, controller tests for
  view-model behavior, fixture-render coverage for every new screen state.

## Tasks

### Task 1 — #2 Surface CLI advisories in the app shell

Foundation for #7 and #9. Stop skipping `warnings`/`notifications` in the
envelope; model them typed; carry them from every read and every streamed job
result frame; dedupe; persist dismissal per CLI version; render a dismissible
advisory banner with visible/copyable remediation commands and an explicit Run
click; never a modal.

### Task 2 — #3 Turn stable CLI error codes into remediation actions

Error rendering gains code/message/detail plus a primary action from a
code → action table, CLI `remediation` first, exit code last. Retry only reads
and idempotent actions; installed/mutating jobs never offer retry. Unit tests
cover the table, unknown codes, and the non-idempotent rule.

### Task 3 — #4 Add a Diagnostics page

New `Diagnostics` page reading `doctor`, `env`, `diagnose proxy|accelerator|update`
in independent sections with per-section loading/error/Retry and one combined
Copy. No mutations. Reuses Task 2's retry action.

Depends on Task 2 for the retry action.

### Task 4 — #5 Make Editor architecture part of row identity

Row identity is `(version, architecture)` everywhere. Install card gains an
architecture picker (host default / x86_64 / arm64), reflected in the dry-run
preview; editors rows already carry architecture. Default set/clear is
architecture-aware and explains ambiguity; project upgrade must not silently
pick a build.

### Task 5 — #6 Add Editor maintenance: prune, verify and default

Prune (report-only by default, explicit confirm before `--remove`, separate
`--remove-missing`, `--force`), verify with a pass/warn/fail check list and raw
log, set/clear default from the list.

Depends on Task 4 for `(version, architecture)` targeting.

### Task 6 — #7 Add install resilience and CLI rollback

Advanced install options (`--resume`, `--download-only`, `--no-wait`,
`--timeout`, `--set-default`, `install-modules --retries`), resume affordance
after an interrupted install, CLI update card with changelog, target/channel,
dry-run, and confirmed rollback that never combines refused flags.

Depends on Task 1 for beta/update advisories.

### Task 7 — #13 Polish the changelog with docs links and an app-impact filter

`unity docs <topic> --url --format json` (never launch a browser), per-entry
docs action with search fallback and quiet failure, and an "Affects this app"
filter whose vocabulary comes from the adapter's command surface.

### Task 8 — #10 Add a Skills page

Supported clients with path/status, tracked installs, install/refresh actions,
verbatim per-install outcome rows (refreshed/up-to-date/pruned/skipped/conflict),
dry-run default, conflict → force only with explanation, `--local` gated on a
project.

### Task 9 — #9 Add a Plugins page with dependency-aware installs

Components grouped by kind with state badges; dependency-ordered install
confirm with per-dependency license steps; beta warning before download via the
Task 1 banner; CLI refusal on removing a required plugin; opt-in `--versions`
and `--refresh` with visible cost.

Depends on Task 1 for the beta advisory.

### Task 10 — #8 Add a Builds page for Unity Build Automation

Explicit org/project context (remembered, no fallback), paged targets/builds,
on-demand steps/failures/artifacts/log, timeline with an informative
"unavailable on this environment" panel. Read-only.

### Task 11 — #11 Add a New Project wizard

Name/parent → Editor version + architecture (install-if-missing, modules) →
template (render-pipeline aware) → optional Cloud → optional VCS → review with
redacted command. Token typed in the app and passed on stdin only. Partial
success keeps the local project and reports the link failure.

Depends on Task 4 for architecture.

### Task 12 — #12 Link and unlink projects with Unity Cloud and VCS

Cloud/VCS badge state and valid actions per row; link flows reuse the wizard's
credential and cloud-project picker; remote deletion is a separate, explicit,
unchecked confirm that passes `--yes`; unlink is confirmed and explains what is
kept.

Depends on Task 11 for the shared credential/cloud-project UI.

## Execution protocol

Per task:

1. Record `BASE=$(git rev-parse HEAD)`.
2. Dispatch one implementer subagent (deepseek-v4.1-flash) with the brief,
   `AGENTS.md`, and the interfaces from earlier tasks. It implements, tests,
   commits, and writes `task-N-report.md`.
3. Generate a review package (`git diff BASE..HEAD`) and review against the
   ticket's acceptance criteria and the global constraints. Large diffs get a
   reviewer subagent that writes `task-N-review.md`.
4. Fix loop on Critical/Important findings, then run the full verification
   suite in the integration worktree and update the ledger.

## Verification (after every task)

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Final: `cargo build -p unity-hub`, gallery fixture test, and a whole-branch
review against this plan and the twelve issues.
