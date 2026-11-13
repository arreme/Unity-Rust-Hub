# Unity CLI 1.0.0 upgrade audit

The app was aligned to Unity CLI `1.0.0-beta.16`. The stable `1.0.0` release is
now installed, and this document records what still works, what needed a small
fix, and what 1.0.0 enables next.

## Verification

- `cargo test -p unity-core --test live -- --ignored --nocapture` passes against
  `unity 1.0.0` (Windows, CLI resolved from the Unity install receipt).
- `unity commands --format json` still reports exit codes `17` (`UNAVAILABLE`)
  and `18` (`AWAITING_INPUT`), both retryable. The app's exit-code mapping is
  therefore still correct.
- The 1.0.0 release notes were read from `unity changelog --target 1.0.0` and
  `unity changelog --target 1.0.0-beta.17`. Only those two releases fall between
  the old beta.16 baseline and 1.0.0.

## Compatibility summary

The command surface the app uses is unchanged:

`version`, `install-path`, `editors --installed --long`, `editors running`,
`pipeline editors`, `releases`, `modules`, `editors info`, `projects --all`,
`open`, `changelog`, `auth status`, `auth login`, `license status`,
`install --dry-run`, `install`, `install-modules`, `projects upgrade`,
`self-update --check`, `self-update`.

No 1.0.0 breaking change touches those commands. The only 1.0.0 breaking changes
are `unity recompile` moving from exit `7` to `12`, and `unity close` /
`unity projects close` returning exit `11` for a non-project path; the app uses
neither.

## Fixed in this pass

| Issue | Change |
|---|---|
| README still described the CLI as beta.16 | README now targets 1.0.0 and records the 1.0.0-specific behaviour. |
| Version validation error suggested `1.0.0-beta.16` | `crates/unity-core/src/versions.rs` now suggests `1.0.0`. |
| Exit-code docs/comments said "beta 16" | `crates/unity-core/src/errors.rs` now documents the 1.0.0 table; the test name is version-neutral. |
| Pipeline-status comment said "beta.16 addition" | `crates/unity-core/src/runner.rs` now describes it as optional rather than version-pinned. |
| `install-modules` comment pinned the behaviour to beta.16 | `crates/unity-hub-controller/src/jobs.rs` now states the behaviour without a version. |
| Changelog flagged `**Breaking:**` entries as "removal, rename, or deprecation wording" | `crates/unity-core/src/changelog.rs` now reports "explicitly marked breaking" for the CLI's own `Breaking:` marker, with a 1.0.0 regression test. |
| `self-update` was invoked with `--format ndjson`, but 1.0.0 prints a two-column line for that format | `UnityCli::self_update_spec` now uses human output, so the job log shows the CLI's readable message instead of `1.0.0<TAB>1.0.0`; the app already refreshes `unity version` after the job. |
| `unity auth status` can report `loggedIn: true` with `sessionState: "stale"` | The account card now shows a "session needs a refresh" hint and a Sign in action instead of presenting a stale session as fully healthy. |
| Sign-in security change was undocumented | README now states that the app never passes `--insecure-storage`; sign-in uses the system credential store or fails with the CLI's message. |

## Open issues and improvements

Effort is approximate: **S** = a few lines, **M** = one feature across core,
model, controller, and UI, **L** = new screen/flow or repeated CLI reads.

### Small UX fixes

1. **Surface CLI warnings on successful commands (S/M).**
   `CliEnvelope` ignores `warnings` and `notifications`, and `run_args` drops
   stderr on success. 1.0.0 emits useful warnings (for example `unity open`
   warns when `HUB_ENV=development` selects the staging cloud). Deserialize the
   warnings and show them in the existing notice area. Keep this bounded so a
   noisy command cannot fill the notice with a wall of text.

2. **Show `self-update` result details (S).**
   `unity self-update` now returns `installed` and `installedVersion` (and
   `appliesAfterExit` when the new binary takes over after exit). The app
   currently reports only "Job finished successfully" and refreshes `version`.
   A notice such as "Unity CLI updated to 1.0.0" would make the result explicit.
   The JSON output is pretty-printed, so this is easier as a post-job read than
   through the line-oriented job parser.

3. **Recognise module notices on stderr (S).**
   1.0.0 deliberately moves `install` / `install-modules` notices (added
   dependency, skipped unknown module) to stderr. The job card prefixes every
   stderr line with `!`, so a notice looks like an error. Consider a third log
   tone for "notice" or a small allow-list of known notice prefixes.

### 1.0.0 features worth wiring

4. **Close a running Editor with `unity close --if-saved` (M).**
   1.0.0 adds `--if-saved`, which refuses to close an Editor with unsaved
   scenes. Add a Close action to each running-Editor row behind a confirmation
   dialog, then refresh the running list. Use `--if-saved` by default; do not
   expose `--force` without a separate, explicit confirmation. Also handle the
   new exit `11` (`PROJECTS_CLOSE_NOT_A_PROJECT`) for a mistyped path.
   This changes the app's current read-only posture, so it should be a deliberate
   product decision.

5. **Install the headless Linux Editor with `unity install --headless` (M).**
   Add a "Headless Editor" checkbox to the install form, enabled only on Linux.
   It affects the dry-run preview as well as the real install. The CLI leaves out
   modules the headless Editor cannot use.

6. **Expose `--resume` and `--architecture` on install (M).**
   Both flags exist in the CLI and matter on interrupted downloads and on Apple
   silicon. They fit naturally in the same install form as the headless option.
   `install-modules` also respects `--architecture` as of beta.17, so the module
   dialog should pass the installed Editor's architecture.

7. **Add Editor upgrades with `unity editors upgrade` (M).**
   The app lists installed Editors but cannot upgrade them. A per-row action
   could call `unity editors upgrade <version> --yes` (optionally `--dry-run`
   first, `--replace` after confirmation). This would also address one of the
   known v1 limitations.

8. **Expose `self-update --rollback` / `--target` (M).**
   `self-update --target <version>` now keeps the chosen version instead of
   letting Unity Hub replace it. `--rollback` restores a kept binary. Both are
   useful recovery paths, but both need clear confirmation and a warning that
   the running binary changes.

9. **Use `unity projects info` for an accurate Cloud badge (M/L).**
    1.0.0 fixes `projects info` reporting `cloudEnabled: true` for an unlinked
    project. `projects --all` is still the right list source, but a per-project
    `projects info` read would make the badge authoritative. That is one CLI
    call per row, so it should be lazy (only when a row is expanded) rather than
    part of the initial project load.

### Larger / deliberately deferred

10. **Editor uninstall, module removal, and disk usage.**
    `unity uninstall`, `unity editors prune`, and the module removal commands are
    available, but the app's known-limitations list still excludes them. These
    are destructive and need their own confirmation flow.

11. **License activation/return.**
    `unity license activate` / `license return` are not part of the current
    account card. The 1.0.0 `license return` fix for Assigned seats makes the
    command more reliable, but the app still needs a license-management flow.

12. **Insecure credential storage.**
    Do not add `--insecure-storage` to the GUI. The 1.0.0 security change
    deliberately requires the system credential store unless the user opts in
    from a terminal. The app should keep surfacing the CLI's error and let the
    user make that choice outside the GUI.

## Recommended order

1. The fixes in this pass (documentation, changelog classification,
   `self-update` output, stale-session hint).
2. Surface CLI warnings on successful commands.
3. Headless/resume/architecture in the install form (one cohesive change).
4. Close Editor with `--if-saved` behind confirmation.
5. Editor upgrade, then uninstall/module removal.
