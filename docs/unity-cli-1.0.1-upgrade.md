# Unity CLI 1.0.1 upgrade audit

The app was aligned to Unity CLI `1.0.0`. The stable `1.0.1` release is now
installed, and this document records what still works, what needed a fix, and
what 1.0.1 changes for the app's CLI workflows.

## Verification

- `unity --version` reports `1.0.1` (Windows, CLI resolved from the Unity install
  receipt).
- The 1.0.1 notes were read from `unity changelog --target 1.0.1`. Only 1.0.1
  falls between the old 1.0.0 baseline and the installed CLI.
- `cargo test -p unity-core --test live -- --ignored --nocapture` passes against
  `unity 1.0.1`. The changelog check parses 33 entries and surfaces 14 possible
  breaking changes from the real notes: the explicit `completion` break, plus the
  script-facing output changes below.
- `unity commands --format json` exposes a `path` on every node. The new ignored
  live test uses those paths to assert all 29 command paths the adapter builds.

## Compatibility summary

The adapter uses `--format json` for every read and never relies on the piped
`--format tsv` default. The 1.0.1 output changes therefore do not break the app:

- The only `**Breaking:**` entry is `unity completion <shell>` writing a JSON
  envelope instead of the raw script. The app does not call `completion`.
- `--format tsv` gains header rows for many commands, and several commands now
  print structured rows instead of bare values. The app always requests JSON, so
  no parser is affected. The same is true of the new `editors path` ndjson result
  line and the `jobs logs` / `builds log` `line` header.
- `unity upgrade --format ndjson` with `--check`, `--dry-run` or an up-to-date
  CLI now emits one result frame. The app's update check already uses
  `--format json`; the update job still uses human output because 1.0.1 does not
  add an ndjson result frame for the install path itself.
- `unity commands --grep` now uses Rust regular-expression syntax, and numeric
  options take plain whole numbers. The app passes validated whole numbers and
  does not build a search pattern, so neither change reaches it.

The command surface the app uses is unchanged:

`version`, `install-path`, `editors --installed --long`, `editors running`,
`editors info`, `pipeline editors`, `status`, `releases`, `modules`, `projects
--all`, `templates`, `templates info`, `templates create`, `templates pack`,
`projects create`, `projects add` / `remove` / `pin` / `unpin`, `projects
upgrade`, `open`, `changelog`, `auth status`, `auth login`, `auth broker ensure`,
`license status`, `install --dry-run`, `install`, `install-modules`,
`self-update --check`, `self-update`.

## Fixed in this pass

| Issue | Change |
|---|---|
| The changelog heuristic missed 1.0.1's script-facing output changes | `crates/unity-core/src/changelog.rs` now flags piped TSV header rows, bare values that became tables, `--format human` guidance for scripts, strict whole-number options, and the Rust regex search syntax. A 1.0.1 fixture covers positives and additive/fix negatives. |
| Version stamps still said 1.0.0 | `README.md`, `errors.rs`, `models.rs`, `runner.rs` and the CLI-version suggestion in `versions.rs` now target 1.0.x, and the README records the 1.0.1-specific behaviour. |
| No guard against a command rename or removal | `crates/unity-core/tests/live.rs` gains an ignored `app_command_surface_is_still_exposed` test. It reads `unity commands --format json`, collects every node's `path`, and fails when one of the adapter's 29 commands is missing. |

## Workflows 1.0.1 fixed

- **Modules for promoted but unpublished Editors (Windows).** `unity install
  modules` now reads the module list from the release data instead of refusing
  the version as unverifiable. The app's module dialog and install form benefit
  without a code change; a previously failing module list can be retried.
- **Absolute paths when the shell's working directory was deleted.** `unity
  close` and the single-project `unity projects` commands (`info`, `size`,
  `clean`, `require`, `upgrade`) now work by absolute path instead of failing
  with "No such file or directory". The app already passes absolute paths, so
  project upgrades and opens are more robust when it is launched from a removed
  directory.
- **`unity projects unlink cloud` reports the removed link.** A future Cloud
  unlink action can show which project and organization were detached. The app
  does not link or unlink Cloud projects yet.
- **`unity logs --format ndjson` paging fields.** The final result line now
  carries `total`, `nextPage` and `partial` like other listings. A future console
  or CLI-log viewer can use the same paging contract as every other read.
- **`unity commands --format json` `path`.** The live schema test described above
  is built on it. It also makes a future command palette or schema snapshot test
  read one stable key instead of reconstructing paths from parent names.

## Open issues

The 1.0.0 audit's open items still apply; 1.0.1 does not remove any of them.

1. CLI `warnings` and `notifications` are still dropped by the envelope parser.
   The app could show them in the existing notice area, with a length bound.
2. `self-update` still reports only "Job finished successfully". A follow-up read
   of the human output can say "Unity CLI updated to 1.0.1" and mention
   `appliesAfterExit`.
3. Module notices that the CLI writes to stderr are still prefixed with `!` in
   the job card. A notice tone or a known-prefix allow-list would separate them
   from errors.

## Recommended order

1. The changelog-classifier and version-stamp fixes in this pass.
2. Surface CLI warnings on successful commands (still the highest-value small
   integration fix).
3. Use the new `projects unlink cloud` result when the Cloud unlink flow lands.
4. Reuse the ndjson paging contract when the CLI-log or Editor-console viewer
   lands.
