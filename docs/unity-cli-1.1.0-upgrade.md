# Unity CLI 1.1.0 upgrade audit

The app was aligned to Unity CLI `1.0.1` and shipped as `v0.4.0`. The installed
CLI is now `1.1.0`, so this document records the breaking changes, the
compatibility work, the deferred features, and the order to pick them up.

The tracking epic is **OPS-8 — CLI 1.1.0 compatibility and upgrade backlog** on
the Operations board, tagged `unity-rust-hub`.

## Verification

- `unity --version` reports `1.1.0`.
- The notes were read from `unity changelog --target 1.1.0`; no 1.1.x patch
  release exists on this machine.
- `cargo test -p unity-core --test live app_command_surface_is_still_exposed --
  --ignored --nocapture` passes: all 32 registered command paths still exist in
  `unity commands --format json`.
- Safe live reads still answer: `unity status --format json` (exit 12 with
  `STATUS_NO_INSTANCES` and an empty `data.items` is the normal "nothing
  running" result), `unity editors --installed --long --format json`, and
  `unity projects --all --format json`.
- `cargo fmt --all`, `cargo test --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo build -p unity-hub` pass.
- `unity doctor` was **not** run: its credential-store probe can trigger the
  Linux polkit prompt described in
  [`docs/investigations/2026-11-06-systemd-creds-polkit-prompts.md`](investigations/2026-11-06-systemd-creds-polkit-prompts.md).

## Breaking changes

| 1.1.0 change | App impact | Verdict |
|---|---|---|
| `unity close` / `unity projects close` exit `18` with `PROJECTS_CLOSE_BLOCKED_BY_DIALOG` instead of waiting out the timeout. | The app does not call either command yet. | Adapter specs added; the Processes flow will offer **Answer the dialog** and only pass `--force` after an explicit confirmation. Exit `18` already maps to `AnswerDialog`, never Retry. |
| A command refused because `UNITY_PROXY` / `HTTPS_PROXY` points to a PAC script now exits `2` with `INVALID_COMMAND_ARGS` instead of `1` with `UNSUPPORTED_STATE`. | Any command, including reads. | Exit `2` / `INVALID_COMMAND_ARGS` now says "show the command and check the proxy/PAC setting". The old bare exit `1` stays generic. |
| A `unity command` blocked by a modal dialog exits `18` with `EDITOR_BLOCKED_BY_DIALOG` instead of `14`. A streamed Editor envelope may still carry `EDITOR_TIMEOUT`, so the exit code is the contract. | The app does not call `unity command` yet. | Exit `18` overrides a streamed `EDITOR_TIMEOUT` in `remediation_for`. `UnityError::is_blocked_by_dialog()` keys on the exit code and the dialog codes. |

Unmarked but compatibility-relevant:

- **Exit `11` moved.** `uninstall`, `editors path`, `editors verify`, `editors
  modules`, `install-modules`, `modules`, `projects verify/open/remove`,
  `templates info/delete`, `assets import/inspect`, and `open` now report
  not-found as exit `11` with a specific code (`EDITOR_NOT_INSTALLED`,
  `NOT_A_PROJECT`, `PROJECTS_REMOVE_NOT_FOUND`, ...). A bare exit `11` is
  deliberately **not** mapped to "release not found" any more; the code names
  the target.
- **`unity command` reports `EDITOR_CONNECTION_LOST`** instead of
  `COMMAND_FAILED` when the Editor drops the connection. The exit code stays
  `6`, and the command may have run. The classifier recognises the code so a
  later `command` flow can offer "check the result", never Retry.
- **`unity build` refuses a second build of the same project.** The future Build
  page must show this as a busy state, not a retry.
- **`unity status` reports `blockedBy.dialogs` more reliably**, including
  Editors whose Pipeline package cannot report dialogs. The Overview and
  Processes pages now consume it.
- **`unity install` / `install-modules` download up to three files at once.**
  The job tracks one percentage per `sha256` and shows their mean, clamped
  monotonically.
- **`projects create` adds `templateVersion` and
  `TEMPLATE_OTHER_VERSION_ON_DISK`.**
- **`docs toc` adds `markdownUrl`** for the script reference and `--manual`.
- **`skill uninstall` and a broader `skill show`** are new.
- **Pipeline Automation and Cloud Build** accept a service account
  (`UNITY_SERVICE_ACCOUNT_ID` / `UNITY_SERVICE_ACCOUNT_SECRET`) and add the
  credential/app/pipeline lifecycle.
- **`build --android-debug-key`** signs an Android build with Unity's debug key
  and conflicts with the keystore flags.
- **`templates pack --version`** now rejects non-semver values such as `01.0.0`
  or `1.0`.
- **Linux glibc floor drops to 2.31**, so Ubuntu 20.04 and Debian 11 work again.
- **`unity pipeline install/upgrade` refuse Unity 6000.5.0–6000.5.2** with
  `PIPELINE_EDITOR_UNSUPPORTED`; `--force` proceeds with a warning.

## Compatibility summary

The adapter still uses `--format json` for reads and `--format ndjson` for
streamed jobs, and never relies on the piped `--format tsv` default. The
command surface is unchanged:

`version`, `install-path`, `editors --installed --long`, `editors running`,
`editors info`, `editors default`, `editors verify`, `editors prune`,
`pipeline editors`, `status`, `releases`, `modules`, `projects --all`,
`projects add` / `remove` / `pin` / `unpin`, `projects create`,
`projects upgrade`, `templates`, `templates info`, `templates create`,
`templates pack`, `open`, `changelog`, `auth status`, `auth login`,
`auth broker ensure`, `license status`, `install --dry-run`, `install`,
`install-modules`, `self-update --check`, `self-update`.

`crates/unity-core/src/surface.rs` is the single registry for those paths, the
flags they pass, the stable codes, and the exit-code table. `command_path` now
returns `None` for an unknown subtree (`pipeline automation` is not the
`pipeline editors` / `cloud-build` surface), and the live suite reads
`surface::REQUIRED_COMMAND_PATHS` instead of keeping its own copy.

The app does not call `close`, `projects close`, or `command` yet. Nothing in
1.1.0 removes a command the app builds.

## Fixed in this pass

| Issue | Change | Commit |
|---|---|---|
| OPS-9 | 1.1.0 codes in `surface.rs`; exit `18` wins over a streamed `EDITOR_TIMEOUT`; bare exit `11` is generic; PAC/`INVALID_COMMAND_ARGS` remediation. | `384f443` |
| OPS-14 | `validate_template_version` uses the SemVer grammar and rejects `01.0.0`, `1.0`, and leading-zero prerelease identifiers. | `daf11a7` |
| OPS-12 | `RunningEditor` carries `blockedBy`; both merges copy it; `dialog_text()` reads it, so the Overview cannot show "ready" while a dialog is present. | `ef7e87a` |
| OPS-20 | One `surface::EXIT_CODES` table; the four derived functions project it. | `c6e5e47` |
| OPS-10 | Remaining command/flag literals moved to the registry; `command`/`close`/`uninstall`/`skill show`/`skill uninstall`/`projects close` registered; code lists centralised; 1.1.0 changelog fixture added. The create-project architecture validator is in `versions.rs` (`aeed35e`). | `c302bc8` |
| OPS-11 | `close_editor_spec` / `close_project_spec` added; `--force` only when the caller asks; `is_blocked_by_dialog()`; tests pin "Answer dialog, never Retry". | `2e1e58a` |
| OPS-13 | Parallel install frames parsed (`pct`, `downloads[]`, per-file `pct`); overall progress tracks each `sha256` and only moves forward. | `f0fe201` |

## Workflows 1.1.0 enables

These are the deferred features, one OPS issue each:

1. **OPS-16 — `templateVersion` and newer-template notice on project create.**
   Small, user-visible, and it closes a current gap: the result today does not
   say which template version was used.
2. **OPS-17 — `docs toc` with `markdownUrl`.** The docs browser can render the
   Markdown page instead of fetching the full HTML page, and still offers "open
   in browser" with `url`.
3. **OPS-15 — `skill uninstall` and `skill show`.** A confirmed, per-skill
   removal with a dry-run preview, and a read-only "read before install" modal.
4. **OPS-18 — Build page.** `--list-targets`, `--list-profiles`, a streamed
   build log on stderr, `--android-debug-key`, and the concurrent-build busy
   state.
5. **OPS-19 — Pipeline Automation and Cloud Build surfaces.** The large remote
   mutation page: service-account auth, credentials, apps, pipelines, jobs.
   Reads first; every irreversible move behind a confirmation.

`OPS-22` tracks the upstream Linux keyring/polkit fix; 1.1.0 does not contain
it (see the investigation doc).

## Recommended order

1. Land this audit and the keyring tracker (OPS-21, OPS-22).
2. OPS-16, then OPS-17: both are small and improve screens that already exist.
3. OPS-15: the Skills page already has install/refresh, so uninstall/show fit
   next to it.
4. OPS-18: the first new mutation surface; keep logs and the result document
   separate, and treat a concurrent-build refusal as busy.
5. OPS-19 last: it is the largest credential and remote-mutation surface and
   depends on the registry and exit-code work that just landed.

## Known limitations after this pass

- The Processes page still cannot close an Editor or run `unity command`; only
  the adapter specs and the exit-18 policy exist (OPS-11).
- The Build and Pipeline screens do not exist yet (OPS-18, OPS-19).
- `blockedBy` is rendered as one dialog title. If 1.1.0 lists several dialogs,
  the last one is shown; a list view can come with the Processes flow.
- The parallel-progress aggregation assumes the frame's scalar belongs to the
  file marked `downloading` when it carries no per-file `pct`. It never moves
  backwards, but a CLI change to a global `pct` would make the bar finish
  earlier than a byte-accurate average.
