# Unity Rust Hub — CLI extension proposals

Investigation of the Unity CLI `1.0.0` command surface against the app's current
coverage, with a proposed catalogue of extensions, tools, and follow-ups.

The app today exposes five pages — Overview, Editors, Projects, Changelog, Settings —
and deliberately treats the CLI as the source of truth for every mutation. Most of the
CLI is therefore unused. This document lists what is worth adding, why, and what it
costs.

Verification: command names and flags below were checked against
`unity commands --format json` and `--help` on Unity CLI `1.0.0` (Windows). One drift
was already found: the CLI reference describes `projects create --path`, while `1.0.0`
exposes `--output-dir`. That is a good argument for the schema-drift test in
[Internal engineering tools](#internal-engineering-tools).

Effort: **S** = a few lines / one read and a card; **M** = a full slice through
`unity-core` → model → controller → UI; **L** = new page/flow, new job kinds, multiple
CLI reads, or long-running work.

---

## Design constraints that shape every proposal

1. **One serialized reader, 30 s deadline.** Every read goes through a single worker with
   a 30-second timeout. `doctor`, `projects size --all`, `projects verify`, `status
   --until-ready`, and all cloud inventory reads can exceed that. They need either a
   longer per-call timeout or a `JobManager` job.
2. **One job at a time.** Installs, upgrades, sign-in, and self-update already share one
   job slot. Builds, tests, template packing, and Editor installs are long enough that a
   queue or at least a "replace active job?" policy becomes necessary.
3. **NDJSON is not uniform.** `install` / `install-modules` stream progress frames on
   stdout; `build` puts the result document on stdout and the streamed log/progress on
   stderr; `self-update` emits human text. `JobManager` currently assumes one shape, so
   each new streaming command needs an explicit stream contract.
4. **No credentials in the app.** Git tokens, service-account secrets, license serials,
   and Editor passwords stay in the user's credential store / CI secrets. The app may
   name the secret to configure; it must not collect or forward it.
5. **Mutations are deliberate.** The app's current posture is read-only unless the user
   starts a confirmed flow. Every destructive CLI call gets a preview, a second
   confirmation, and a copyable command line.
6. **The UI contract is one-way.** New features still land as `unity-core` DTO + command
   spec → `unity-hub-model` view model/action → controller mapping → `unity-hub-ui`
   page. No raw JSON or `UnityError` reaches the UI.

---

## Proposal catalogue

### A. Projects, templates, and creation

| # | Proposal | CLI surface | Value | Effort | Risk |
|---|---|---|---|---|---|
| A1 | **New Project wizard** | `projects create` (`--output-dir`, `--editor-version`, `--template`, `--install-editor`, `--module`, `--accept-eula`, `--bare`, `--no-cloud`, `--vcs`, `--open`) | Closes the biggest gap: the app can open and upgrade projects but cannot create them | L | Medium |
| A2 | **Template library** | `templates --brief --type core|learning|sample|custom|new`, `templates info`, `templates location`, `templates create`, `templates pack` | Standardize team starters; create a portable `.tgz` from any project | M–L | Medium |
| A3 | **Template maintenance** | `templates edit`, `templates delete --yes`, `templates location --set/--reset` | Keep a curated template catalog without a terminal | M | Destructive delete |
| A4 | **Clone project** | `projects clone` (`--vcs`, `--vcs-namespace`, `--vcs-repo`, `--ref`, `--output-dir`, `--subpath`, `--no-lfs`) | Onboard an existing repo, including monorepo subpaths and UVCS changesets | M | Medium |
| A5 | **Project details drawer** | `projects info`, `projects size`, `projects verify`, `projects report`, `projects pin/unpin` | Turns the Projects page into a maintenance console; accurate Cloud badge | M | Low |
| A6 | **Install required Editor** | `projects require <path> --yes --timeout` | One action from "this project needs 6000.3.12f1" to a working Editor | M | Medium |
| A7 | **Fleet command runner** | `projects exec --filter ... --parallel N --dry-run` | Batch `git status`, custom scripts, or upgrades across the registry | L | High (arbitrary command execution) |

**A1 notes.** `projects create` links to Unity Cloud **by default**. The app must pass
`--no-cloud` unless the user explicitly opts in, or a GUI user who is signed out gets
`NOT_SIGNED_IN` (exit 3) before scaffolding. Pass every option (name, `--output-dir`,
`--editor-version`, `--template`) so `--non-interactive` cannot fall through to a prompt.
Read the result's `steps[]` (`done`, `present`, `skipped`, `failed`) rather than the
individual `pipeline` / `playerAuth` / `cinemachine` blocks. Handle
`TEMPLATE_NOT_FOUND` (`data.offered`), `ENGINE_NOT_INSTALLED` (offer
`--install-editor`), `LICENSE_CHOICE_REQUIRED`, and `CLOUD_CHOICE_REQUIRED`.
Never pass `--git-token`/`--git-token-from-stdin` from the GUI: with no token, a
non-interactive create either uses the user's credential helper or fails with exit 4 and
a message naming the credential — show that message.

**A2 notes.** `templates create` installs into the Hub-configured user templates
directory and registers the template; `templates pack` writes a standalone `.tgz` and
registers nothing. `pack` is the right one for a release artifact or a checked-in
starter. Use `--output-dir` for the user-template directory, `--output <file>` for
`pack` (it is a file, not a directory, and may not be inside the packed project).
Exclude generated folders and respect `.gitignore` expectations; warn that a template
can capture `ProjectSettings` and embedded packages only if asked
(`--keep-project-settings`, `--keep-embedded-packages`).

**A5 notes.** `projects size --all` and `projects verify` are the two genuinely useful
maintenance reads. `verify` is Editor-free, license-free, network-free, and CI-safe;
`--strict` promotes warnings, `--expect-editor` adds version drift, and `--check` narrows
the checks. `size` reports raw bytes under `--json`, human units otherwise. `info` is one
call per project and should be lazy (expanded row only), because the registry list is
already one call.

---

### B. Build, test, and CI

| # | Proposal | CLI surface | Value | Effort | Risk |
|---|---|---|---|---|---|
| B1 | **Local build page** | `build --list-targets`, `--list-profiles`, `--create-profile`, `--profile/--target`, `--output-path`, `--versioning-strategy`, `--provenance-path`, `--timeout`, `build run` | Build and launch players without leaving the Hub | L | Medium |
| B2 | **Test runner + report viewer** | `test --mode`, `--filter`, `--report-format junit`, `--coverage`, `--shard`, `--retries`, `--affected`, `--rerun-failed` | The most-used day-2 workflow after project creation | L | Medium |
| B3 | **CI scaffolding** | `ci init --provider github|gitlab --target --shards --dry-run --overwrite` | Generates a correct, secret-free workflow from the project | M | Writes to repo |
| B4 | **CI readiness card** | `doctor --ci`, `cache key --target`, `projects verify --strict --expect-editor` | Fails in seconds instead of 20 minutes into a build | M | Low |
| B5 | **Cloud Build dashboard** | `pipeline cloud-build targets/builds/project/tooling`, `builds steps|failures|artifacts|log|download` | See CI builds without the web dashboard | L | Auth + context |
| B6 | **Pipeline Automation** | `pipeline automation pipelines/jobs/jobs stats/jobs watch/logs/approve/reject` | Trigger and supervise automation jobs | L | Remote mutations |

**B1 notes.** The build log is streamed to stdout in human mode but to **stderr** under
`--format json`; stdout is then the single result document. `JobManager` must route
streams per command. The outcome is decided by Unity's terminal log verdict, not the
process exit code alone, so a "successful" process can still be a failed build (exit 6).
Show `data.outputPath`, `data.provenance`, `editorErrors`, and a "Launch build"
(`build run`) action. Provenance manifests are redacted and safe to attach to a release;
surface a summary rather than the raw file.

**B2 notes.** Exit codes matter: `0` pass, `8` tests ran and failed, `6` infrastructure
failure, `14` timeout, `2` usage error. `--report-format junit` is what CI consumes;
`--format github` emits workflow annotations. The editor writes JUnit only via conversion,
and `--shard` requires an NUnit inventory, not JUnit. `unity test` refuses a project that
is already open in an Editor. Do **not** close the user's Editor to make it run: if the
Pipeline package is present, run tests through the live Editor's command instead, and only
when it is not in Play mode; otherwise explain and let the user close it.

**B3 notes.** `ci init --dry-run` prints the workflow and writes nothing, which is exactly
the GUI preview step; then write with `--overwrite` after confirmation. It never reads a
credential and only names `UNITY_SERVICE_ACCOUNT_ID`, `UNITY_SERVICE_ACCOUNT_SECRET`, and
`UNITY_LICENSE_SERIAL` as secrets to add. Show that list. `--shards 2..64`; `1` is
rejected. The generated workflow already includes `doctor --ci`, tests, build, artifact
upload, and license teardown — do not reimplement it by hand.

**B4 notes.** `doctor --ci` exits `0` (pass), `6` (definitive blocker), `7` (service
unreachable, retryable), and `6` outranks `7`. Accelerator problems never block.
`cache key` is deterministic across OSes and is the correct CI cache key; `--component
editor` gives a fallback level.

---

### C. Editors, processes, and live control

| # | Proposal | CLI surface | Value | Effort | Risk |
|---|---|---|---|---|---|
| C1 | **Process manager page** | `status`, `editors running`, `pipeline editors` | One place to see every Editor: project, PID, port, state, play mode, Safe Mode, dialogs | M | Low (read-only) |
| C2 | **Safe close** | `close <project> --if-saved --timeout` | Free a project for upgrade/build without losing work | M | Unsaved work |
| C3 | **Force close** | `close <project> --force` | Recover a hung Editor | S | High: data loss |
| C4 | **Live Editor console** | `logs --console --tail --level --follow --format ndjson` | Read compile errors and runtime logs without opening the Editor | M | Stream volume |
| C5 | **Play mode control** | `command editor.playmode start|stop`, `status --until-ready` | Verify a game actually advances, not just that it launched | M | Needs Pipeline |
| C6 | **Dialog handling** | `command editor.dialog list|answer|dismiss` | Unblock an Editor stuck on "Save changes?" / upgrade prompt | M | Clicking for the user |
| C7 | **Detached job tracking** | `command --detach`, `job status|wait|cancel` | Long work survives a page change; reattach after restart | M | Job lifetime |
| C8 | **Editor upgrade** | `editors upgrade --check|--dry-run|--replace|--remove-old|--architecture|--module` | Fix the "no Editor upgrade" limitation | M | Large download |
| C9 | **Uninstall / prune / module removal** | `uninstall`, `editors prune --remove`, `editors module remove` | Reclaim disk | M | Destructive |
| C10 | **Process hygiene** | app-side process groups / job objects | No orphaned `unity`/Editor processes when the app exits | M | Platform-specific |

**C1 notes.** `unity status` is the authoritative read: it reports `state`
(`ready`/`starting`/`unreachable`), `projectPath`, `editorVersion`, `pid`, `port`,
`frameCount`, `playerLoopTicking`, and `blockedBy.dialogs` (Unity 6000.7+ with the right
Pipeline version). Merge it with `editors running` and `pipeline editors` for phase,
Pipeline version, update availability, Safe Mode, and the blocking dialog. Poll only while
the page is visible (2–5 s), and stop when the window is hidden. CPU/RSS would need a new
dependency (`sysinfo`) and conflicts with the app's slim-binary discipline: ship PID/state/
port first, add metrics only if the size budget allows. On macOS, dialog titles in the
window list need Screen Recording permission; do not treat an invisible dialog as "ready".

**C2/C3 notes.** `close` exits **without saving** by default; `--if-saved` is the safe
form and should be the only one exposed on the normal row. `--force` needs a separate
two-step confirmation that names the project and says unsaved work will be lost. Handle
`PROJECTS_CLOSE_PIPELINE_MISSING` (a windowed Editor without the Pipeline package offers
no graceful channel) and exit `11` for a non-project path.

**C5 notes.** `editor.playmode start` drops the Pipeline listener for about 8 seconds;
use `status --until-ready`, not a polling loop. A frozen Editor can still answer `status`
as `playing`; check `frameCount` advances and `playerLoopTicking` is true before claiming
success. This is the foundation for a "smoke test" action, not a general Play-mode remote
control.

**C6 notes.** Requires Unity 6000.7+ and a Pipeline package with the dialog commands.
Answering can stack a new dialog (`pending`); iterate, innermost last. Never auto-answer
a save prompt: present the exact title/buttons and require a click that maps to a button
label. `force close` remains the fallback.

**C8/C9 notes.** `editors upgrade --dry-run` is a preview, not a build; `--replace` and
`--remove-old` are separate destructive decisions. Prune before uninstall to see what no
registered project needs (`projects report`). Module removal is `--yes` and can break an
install; confirm with the module names.

**C10 notes.** The CLI spawns the Editor, so killing the CLI's direct child may orphan
the Editor. Use a process group on Unix and a Job Object on Windows, and prefer
`unity close` for graceful shutdown. This also protects `build`/`test`, which spawn a
batch Editor.

---

### D. Diagnostics and health (Unity Doctor)

| # | Proposal | CLI surface | Value | Effort | Risk |
|---|---|---|---|---|---|
| D1 | **Doctor page** | `doctor --format json`, `doctor --ci`, `doctor --tail`, `--skip-license` | One screen for "why is this machine not working?" | M | Slow read |
| D2 | **Environment and cache** | `env`, `cache info`, `cache clean --yes`, `cache key` | Paths, cache size, safe cleanup, CI key | S–M | Low |
| D3 | **Proxy and Accelerator** | `config proxy`, `diagnose proxy`, `config accelerator`, `diagnose accelerator`, `doctor --ci` | Fix the two most common "slow/unreachable" causes | M | Low |
| D4 | **CLI log viewer** | `logs --tail --level --follow` | Readable CLI activity, copyable for support | S | Low |
| D5 | **Update diagnosis** | `diagnose update` | Explain why `self-update` says "up to date" | S | Low |
| D6 | **Report a bug** | `bug --area ...` | One-click issue with logs — **only after explicit consent** | S | Uploads logs |
| D7 | **Copy diagnostics** | `doctor` + `env` + `version` | Paste-safe, redacted support bundle | S | Redaction |

**D1 notes.** `doctor` covers platform, auth, installed Editors, post-install PATH
checks (binary directory on `PATH`, shadowed binaries, Windows long paths), git credential
helper, proxy, Accelerator, and third-party components. It is redacted and paste-safe.
`doctor --ci` is a different shape: per-check pass/warn with codes and remediation hints,
and exit `6` vs `7`. A doctor read can be slow; run it as a short job with a 60–90 s
budget, not on page entry through the 30 s worker.

**D3 notes.** `diagnose proxy` and `diagnose accelerator` always exit `0` even when the
endpoint is broken — they report a row, so the UI must render the row, not infer failure
from the exit code. `config accelerator` persists an endpoint that `run`/`test`/`build`
inject automatically; do not expose it on `open` (the CLI doesn't inject there).

**D6 notes.** `unity bug` sends logs to Unity. It must stay behind a confirmation that
states what is uploaded, and it must never run on app startup or on a crash report.

---

### E. Account, license, cloud, and VCS

| # | Proposal | CLI surface | Value | Effort | Risk |
|---|---|---|---|---|---|
| E1 | **License management** | `license status`, `license activate --serial|--floating|--file|--personal|--generate-request`, `license return --yes`, `license server` | The account card currently stops at status | M | Secrets / seats |
| E2 | **Cloud context** | `cloud status`, `cloud org`, `cloud project` | Pick the org/project that cloud features need | S–M | Low |
| E3 | **Source-control card** | `vcs status`, `vcs sync`, `vcs doctor`, `vcs conflicts/resolve/explain`, `vcs uvcs locks`, `vcs affected` | Surfaces merge damage and lock collisions before a build | M | Provider-specific |
| E4 | **Git LFS / worktrees** | `vcs git migrate-lfs`, `vcs git worktree add` | Correct asset-heavy onboarding and isolated work | M | Repo mutations |

**E1 notes.** Prefer the CLI's secret-safe path: a bare `license activate` with
`UNITY_LICENSE_SERIAL` in the environment (the generated CI workflow does this) rather
than `--serial`, which lands in argv. `license return` frees a seat — confirm and state
that. `--personal` is rejected for service accounts. Never store a serial in settings.

**E3 notes.** `vcs uvcs locks` is a joined read (who holds a lock and what it collides
with your pending changes) and is explicitly racy; never cache it, read again before
acting. `vcs doctor` anchors findings to file/line and is safe to paste. `resolve`,
`switch`, and `sync` are mutations; preview with `vcs diff`/`conflicts` first.

---

### F. Integrations and in-app tools

| # | Proposal | CLI surface | Value | Effort | Risk |
|---|---|---|---|---|---|
| F1 | **MCP / AI client setup** | `mcp configure`, `setup <client>`, `skill install/refresh` | Makes the Hub the on-ramp for agent-driven Unity work | M | Config writes |
| F2 | **Plugin manager** | `plugin list`, `plugin install --dry-run`, `plugin upgrade --changelog`, `plugin remove`, `plugin changelog` | Discover optional tooling without a terminal | M | EULA / downloads |
| F3 | **Version-matched docs** | `docs toc`, `docs --content`, `docs cache` | "Docs" for the installed Editor, offline-capable | M | Low |
| F4 | **Command palette** | curated `unity ...` invocations, copy-only by default | Power-user escape hatch without a shell | M | Arbitrary execution |
| F5 | **Saved contexts** | `context save/use/list/current/delete` | Named project/editor/org combinations | M | Low |

**F1 notes.** `unity mcp configure` registers the CLI's MCP server in an AI client;
`setup` installs the plugin for Claude Code/Codex/Copilot/etc. These write to the user's
client config, so show the exact target and ask first. `plugin install ai` is alpha and
gated to Unity staff; a refusal is a normal answer, not a bug.

**F2 notes.** `plugin install`/`upgrade` show license terms and wait; `--dry-run` prints
the terms and changes without accepting. Never pass `--accept-eula` on the user's behalf.

**F3 notes.** Docs are keyed to an Editor version (`--editor-version`), with `docs cache`
for offline use. A "Docs" action on an Editor row, a project, or an error message is the
highest-value placement. Render Markdown as plain text, as the changelog already does.

**F4 notes.** If built at all, keep it to a curated allow-list of read commands plus
"copy command", with mutations requiring the same confirmation dialogs as the typed UI.
Do not expose a general shell.

---

## Internal engineering tools

These are for the repo, not the user, and they protect the CLI-as-source-of-truth bet.

| Tool | What it does | Effort |
|---|---|---|
| **CLI schema snapshot test** | Run `unity commands --format json` in CI, diff the subset of commands/flags the app uses, fail on removal or rename. Would have caught `projects create --path` → `--output-dir`. | M |
| **Envelope fixture recorder** | Capture real, redacted `--format json`/`ndjson` envelopes from a live CLI into `unity-core/tests/fixtures`, so parser tests do not hand-write shapes that drift. | M |
| **Command-spec tests** | One test per new command: exact argv, `--format`, `--non-interactive`, timeout, and env (`DO_NOT_TRACK=1`). | S per command |
| **Stream-contract tests** | Assert where each command writes frames, log, and result (`install` stdout, `build` stderr, `self-update` human). | M |
| **Job lifecycle tests** | Cancel kills the process tree; closing the window leaves no orphan; a detached job can be reattached. | M |
| **Gallery coverage** | Add every new page/modal/job/error state to `unity-hub-design-gallery` so design review stays CLI-free. | S per state |
| **Live compatibility suite** | Extend `crates/unity-core/tests/live.rs` with read-only checks for the new commands, and record the tested CLI version in docs. | M |

---

## Deliberate non-goals

- `--insecure-storage` for `auth login`.
- Implicit `--accept-upgrades` on open.
- `--force` close without a separate, explicit confirmation.
- Implicit `--cloud` on `projects create`; the default must be **not linked** in the GUI.
- `unity bug` upload without consent.
- Collecting Git tokens, service-account secrets, license serials, or keystore passwords.
- A general shell / arbitrary command runner in v1.
- Auto-answering "Save changes?" dialogs.

---

## Suggested roadmap

**Wave 1 — fill the obvious gaps (highest value / lowest risk).**
A1 New Project, A2 template list + pack, C1 read-only Process Manager, D1 Doctor page,
plus the small follow-ups already listed in `docs/unity-cli-1.0-upgrade.md` (warnings on
success, `self-update` details, module notices on stderr).

Why first: the app can create nothing today, and "why is my machine broken / what is
running" is the other daily question. All are mostly reads or one guided mutation.

**Wave 2 — the build loop.**
B1 local build, B2 test runner, B3 `ci init`, B4 preflight, C2 safe close, C4 console.

Why second: this is what turns the Hub from an installer into a tool used every day.
It also forces the job/stream work that every later feature needs.

**Wave 3 — lifecycle and disk.**
A5 project details/verify/size, C8/C9 Editor upgrade and uninstall, E1 license, D2 cache,
C5/C6 live Editor controls.

**Wave 4 — remote and integrations.**
B5/B6 cloud builds and automation (read-only first), E3/E4 VCS, F1/F2/F3 MCP, plugins,
docs, F5 contexts.

---

## Recommendation

Start with A1 + A2 + C1 + D1. They are independent, each can ship behind its own page or
card, and together they cover the three questions a Unity developer asks the Hub:
"What do I start from?", "What is running?", and "Why is this machine unhappy?".

Before Wave 2, land the CLI schema snapshot test and the stream-contract work; build and
test streaming will otherwise be built twice.
