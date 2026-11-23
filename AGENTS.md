# Working in this repository

Unity Rust Hub is a native Rust/iced desktop GUI for the Unity CLI. The CLI is
the source of truth: the app never invents data, never installs or removes
anything without an explicit user click, and never runs a command the user has
not seen.

## Build and verify

Run all four before saying anything is done:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p unity-hub            # the real binary still links
```

The design gallery renders every fixture without a Unity CLI:

```bash
cargo run -p unity-hub-ui --bin unity-hub-design-gallery
```

## Layer rules (enforced by Cargo, do not break them)

```
unity-core            CLI adapter: command specs, envelope parsing, DTOs, errors
        ↑
unity-hub-controller  state, jobs, settings, domain → view-model mapping
        ↓
unity-hub-model       pure contract: view models + UiAction, no dependencies
        ↑
unity-hub-ui          tokens, components, shell, pages, modals, fixtures, gallery
        ↑
unity-hub             iced application and runtime wiring
```

- `unity-hub-ui` may use only `unity-hub-model` and iced. It never builds CLI
  arguments, parses envelopes, or sees `UnityError`.
- `unity-hub-controller` never builds `Element`s.
- `unity-core` has no iced and no UI knowledge.

## Adding a CLI capability

1. DTO in `crates/unity-core/src/models.rs` (`camelCase` serde, defaults for
   every field the CLI may omit).
2. Command builder/reader on `UnityCli` in `crates/unity-core/src/runner.rs`.
   Reads use `self.spec(&parts, "json")` / `run_args`; streaming jobs use
   `"ndjson"`. Validate every user-supplied token in `versions.rs` before it
   reaches the command line.
3. Never interpolate a token into a shell string. `CommandSpec` takes argv
   elements. Show `CommandSpec::display()` for previews.
4. Secrets (VCS tokens) go to the child's stdin only, never argv, logs, or
   settings.

## Adding a screen or state

1. View model in `crates/unity-hub-model/src/view.rs`; actions in
   `action.rs` carry stable identifiers, never row indices.
2. Controller state + `Message` variants + mapping in
   `crates/unity-hub-controller/src/app.rs`. Async results stay private to the
   controller.
3. UI page in `crates/unity-hub-ui/src/pages/`, registered in `pages/mod.rs`,
   `Page`/`PageVm` and the sidebar (`shell.rs`).
4. Fixtures in `crates/unity-hub-ui/src/fixtures.rs` for every new loading,
   empty, error, loaded, and modal state. `fixtures::all()` is the gallery and
   is rendered by a test.
5. Update `docs/design/contract.md` (screen/state inventory) and the README
   feature list.

Component vocabulary lives in `crates/unity-hub-ui/src/components.rs`
(`card`, `list_row`, `badge`, `notice`, `checkbox`, `segmented`, `scroll`,
`empty`, `error_text`, `hint`, `button` + `ButtonKind`). Reuse it instead of
raw iced styling. Colors and spacing come from `tokens.rs` / `theme.rs`, never
from literals.

## Controller conventions

- `Loadable<T>` is the controller's internal read state; `LoadState<T>` is the
  view-model state. Keep the mapping in `app.rs`.
- Reads are serialized through one worker; do not spawn CLI processes in the
  UI layer. Long mutations become streamed jobs via `JobManager` and appear in
  the job card.
- Never offer automatic retry for a job that already started a mutation.
- Errors are `UnityError { code, message, detail, exit_code, retryable, extra }`.
  Branch on `code`, treat `message` as localized text, and never parse stderr
  to make a decision.
- Deduplicate CLI advisories by stable identity; do not stack identical
  banners on every refresh.

## Testing

- Unit tests live next to the code (`#[cfg(test)] mod tests`).
- Command construction tests assert the exact argv (`spec.args`).
- Controller tests build `App::boot()` and assert on `view_model()`.
- Fixtures are covered by `every_fixture_renders`; add fixtures rather than
  hand-testing the gallery.
- No test may call a live `unity` binary. `crates/unity-core/tests/live.rs` is
  ignored by default.

## Documentation

- README: features, CLI alignment notes, safety notes, known limitations.
- `docs/design/contract.md`: view model / action / fixture inventory for the
  designer.
- Both are part of a feature, not follow-up work.

## Commits

Conventional commit subjects, imperative mood, lower case after the prefix
(`feat(editors): add architecture-aware install card`). One ticket per commit
unless the ticket explicitly spans a refactor; the ticket number goes in the
body (`Refs #5`). Do not reformat unrelated files.
