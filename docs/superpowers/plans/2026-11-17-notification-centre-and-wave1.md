# Notification centre and Wave 1 integration — implementation plan

Specs of record: GitHub issues **#18**, **#19** (the epic #14 tickets) and a re-scoped **#29**.
The user's ruling: #29 closes with the notification-centre integration; the Doctor rows stay
with #26–#28.

## Intent

Today the app throws away every CLI advisory. `CliEnvelope` marks `warnings` and
`notifications` as `#[serde(skip)]` (`crates/unity-core/src/envelope.rs:20-23`), every read
drops the envelope after `parse_data`, and the shell renders a single `notice` slot that each
new message overwrites (`crates/unity-hub-controller/src/app.rs`, 35 assignment sites;
`crates/unity-hub-ui/src/shell.rs:31-33`).

The result is a collapsible right-hand rail of notifications with an unread badge, an
auto-expiring toast, and actionable remediations, replacing the single notice banner.

## Global constraints

- **CLI contract (verbatim, from `.agents/skills/unity-cli/SKILL.md:653-689`):**

  ```jsonc
  {
    "code": "CLI_UPDATE_AVAILABLE",
    "message": "A new version of the Unity CLI is available: 1.0.0-beta.8 → 1.0.0-beta.9",
    "data": { "current": "1.0.0-beta.8", "latest": "1.0.0-beta.9" },
    "remediation": { "command": "unity self-update", "requiresUserApproval": true }
  }
  ```

  - `notifications` is absent when empty; `warnings` is always present (may be `[]`).
  - `remediation` may be absent. Absent means informational, nothing to run.
  - `requiresUserApproval: true` means *do not run it*. Treat a missing flag as `true` (safe default).
  - Branch on `code`; `message` is for display; `data` is shaped per code.
- **Run rule (issue #19):** a Run action exists only when `remediation` is present **and**
  `requiresUserApproval == false`, and a second explicit click confirms it. Nothing is ever
  auto-run, and no command is auto-retried. A command that already started work is never retried.
- **No overlay widgets.** `stack!` miscomposites under the tiny-skia fallback
  (`README.md:400-403`). The rail is a third column in the shell row (reflows the page), and the
  toast is an inline banner above the page content inside the scrolling column.
- **No new icon glyphs.** Adding one needs `scripts/subset-fonts.sh` + network + fontTools.
  Use the existing `icons::CIRCLE_DOT` for the notifications affordance.
- **No new crates.** Command splitting for a remediation is a small local parser, not a shell.
- **History is session-only.** No persistence, no OS notifications, no remote feed.
- Quality gate for every task: `cargo fmt --all --check`, `cargo test --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and at the end
  `cargo build -p unity-hub --no-default-features`.
- `unity-core` stays UI-agnostic; no iced types below `unity-hub-ui`.

## Decisions taken (rulings)

1. **Capture mechanism:** a shared advisory sink on `UnityCli` rather than changing the return
   type of ~25 read functions. `UnityCli` is `Clone` over `Arc`s, so worker clones share it.
   Cost if wrong: advisories from reads are attributed to no particular command; dedupe absorbs it.
2. **Warnings may be strings or objects.** Normalise both; a bare string is a message with no code.
3. **Error advisories come from `execute_inner`** so a failed call's `errors[0]` (with
   `remediation` inside `extra`) also lands in the rail with Danger tone. Dedupe prevents a
   refresh loop from stacking them.
4. **Toast replaces, never queues.** The newest notification owns the single toast; its id guards
   the 5 s expiry so a replaced toast cannot clear the newer one.
5. **Rail open marks everything read** (unread count to 0). Collapsed by default.
6. **Failed remediation stays visible** with a Failed badge and Copy; Run is re-offered only by a
   fresh explicit click, never automatically.

## Task 1 — Core: advisories, envelope, sink (unity-core)

Files: `crates/unity-core/src/advisory.rs` (new), `src/lib.rs`, `src/envelope.rs`, `src/runner.rs`.

1. New module `advisory`:
   ```rust
   pub enum AdvisoryKind { Warning, Notification, Error }   // Serialize/Deserialize, camelCase
   pub struct Remediation { pub command: String, pub requires_user_approval: bool }
   pub struct Advisory {
       pub kind: AdvisoryKind,
       pub code: Option<String>,
       pub message: String,
       pub data: Option<serde_json::Value>,
       pub remediation: Option<Remediation>,
   }
   impl Advisory {
       pub fn from_value(kind: AdvisoryKind, value: &serde_json::Value) -> Option<Self>;
       pub fn from_error(error: &UnityError) -> Self;
       pub fn dedupe_key(&self) -> String;
   }
   ```
   - string → message; object → `message` (fallback `text`, then `detail`), `code`, `data`,
     `remediation` object or bare string.
   - `requiresUserApproval` defaults to `true`.
   - `dedupe_key`: `code` plus canonicalised `data` (serde_json renders maps in sorted key order),
     otherwise the message. Blank/whitespace-only entries are dropped.
2. `CliEnvelope`: drop `skip` from `warnings`/`notifications`; keep `#[serde(default)]`.
   Add `impl<T> CliEnvelope<T> { pub fn advisories(&self) -> Vec<Advisory> }` mapping warnings →
   `Warning` and notifications → `Notification`.
3. `UnityCli`: add `advisories: Arc<Mutex<Vec<Advisory>>>`, shared across clones, with
   `pub fn drain_advisories(&self) -> Vec<Advisory>` (poison-safe: `unwrap_or_else(|e| e.into_inner())`).
   In `execute_inner`, after `parse_envelope` succeeds, push `envelope.advisories()` — for successful
   and failed envelopes alike. Errors are **not** turned into advisories here (the controller owns
   their presentation so it keeps `detail`); instead add
   `impl UnityError { pub fn remediation(&self) -> Option<Remediation> }`, which parses
   `extra["remediation"]` (object with `command`/`requiresUserApproval`, or a bare string → command
   with `requires_user_approval = true`).
4. Unit tests (inline `#[cfg(test)] mod tests` in `advisory.rs`, `envelope.rs`, `errors.rs`):
   bare-string warning, notification with remediation and `data`, missing
   `requiresUserApproval` → `true`, blank entries dropped, dedupe key stable across refreshes and
   distinct for different `data`, envelope no longer skips `warnings`/`notifications`, sink drains
   once and accumulates, `UnityError::remediation` reads the flattened `extra` field.

## Task 2 — Stream: advisories from NDJSON frames (controller/jobs.rs)

Files: `crates/unity-hub-controller/src/jobs.rs`.

1. `ParsedLine` gains `advisories: Vec<Advisory>`.
   - Result frames (`type == "result"` or a success/errors/data object): re-parse as
     `CliEnvelope<Value>` and take `advisories()`.
   - `{"type":"warning"|"notice"|"notification"}` frames (mid-job): parse the whole object as an
     advisory (warning → `Warning`, otherwise `Notification`).
   - Progress frames and plain rows contribute none.
2. `JobEvent` gains `advisories: Vec<Advisory>`; the terminal event keeps the result-frame
   advisories, mid-job events carry their own.
3. Tests: mid-job warning frame → one `Warning` advisory on that event; result frame with
   `notifications` → advisories only on the terminal event; a progress-only stream → none;
   repeated identical progress frames never yield advisories.

## Task 3 — Model + controller store, rail state, remediation runs

Files: `crates/unity-hub-model/src/view.rs`, `src/action.rs`,
`crates/unity-hub-controller/src/notifications.rs` (new), `src/lib.rs`, `src/app.rs`.

1. Model (`view.rs`):
   ```rust
   pub enum NotificationActionState { Idle, Confirming, Running, Failed }
   pub struct NotificationVm {
       pub id: u64, pub tone: Tone, pub label: String, pub message: String,
       pub command: Option<String>, pub can_run: bool, pub state: NotificationActionState,
   }
   pub struct ToastVm { pub id: u64, pub tone: Tone, pub message: String }
   pub struct ShellVm { /* existing */ pub notifications: Vec<NotificationVm>,
                        pub unread: u32, pub rail_open: bool, pub toast: Option<ToastVm> }
   ```
   Remove `NoticeVm` and `ShellVm.notice`.
2. Actions (`action.rs`): remove `DismissNotice`; add
   `ToggleNotificationRail`, `DismissNotification(u64)`, `ClearNotifications`,
   `DismissToast(u64)`, `CopyNotificationCommand(u64)`,
   `RunNotificationCommand(u64)`, `ConfirmNotificationCommand(u64)`, `CancelNotificationCommand(u64)`.
3. New `notifications.rs` store: newest-first `VecDeque<Notification>`, `next_id`, `unread`,
   `toast: Option<Toast>`, `seen: HashSet<String>`, and `remediating: Option<u64>`.
   `push_advisory(kind → Tone)` returns whether it was new; duplicates neither toast nor raise
   the badge. `push_event(tone, message)` for job outcomes and app errors. `dismiss`, `clear`,
   `mark_read`, `toast_expired(id)`, `snapshot()`. `Toast` records its id; expiry compares ids.
   Command validation lives here: first token is `unity`/`unity.exe`, no `| & ; < > $ ` " ' \n`
   characters, no empty args; otherwise the notification is copy-only.
4. `app.rs`:
   - `notice` field → `notifications: NotificationStore`; delete `Notice`/`NoticeKind`/`notice_tone`.
   - Every former `self.notice = ...` becomes a store push: `push_error(&error)` when the value is
     a CLI `UnityError` (keeps code, detail and remediation), otherwise `push_event(tone, text)`
     with the same tone. Update all 35 sites, including `broker_env_notice`, `start_job`,
     `poll_jobs`.
   - `update()` drains `self.cli.drain_advisories()` at the end of every message.
   - `poll_jobs` feeds `event.advisories` into the store (job success/failure/cancel still push a
     tone notification with the existing text).
   - `RunNotificationCommand` → state `Confirming`; `ConfirmNotificationCommand` → state `Running`,
     build `cli.spec(&args[1..], "ndjson")`, `start_job("remediation", spec)`; if another job runs,
     revert to `Idle` and push an error notification. `CancelNotificationCommand` → `Idle`.
   - Terminal event for the `remediation` job: success → `Idle` + `refresh_page()`;
     failure → `Failed`.
   - New subscription while `toast.is_some()`: sleep ~5 s then `Message::ToastExpired(id)`.
   - `shell_vm()` maps the store.
5. Tests (controller `app.rs` and `notifications.rs`): dedupe by code+data, unread counting,
   mark-read on rail open, dismiss/clear, toast replacement and expiry, confirmation gate
   (`requiresUserApproval: true` → no Run), command validation rejects shell metacharacters and
   non-unity commands, job result advisories ingested once for a two-progress-frame stream,
   `view_model` maps notifications/toast/rail.
6. **Compile-only UI surgery** (the rail itself is Task 4): delete `notice_view` and the banner in
   `shell.rs:31-33,160-181`, set the new `ShellVm` defaults in `fixtures.rs::shell()`, and change
   the `job_failed` fixture to push a notification instead of `NoticeVm`. This task must leave the
   whole workspace compiling and every existing test green; it renders no rail yet.

## Task 4 — UI: rail, toast, badge, components, fixtures

Files: `crates/unity-hub-ui/src/shell.rs`, `src/components.rs`, `src/fixtures.rs`,
`src/pages/*` (only if they referenced `NoticeVm`), `src/bin/gallery.rs` (only if needed).

1. Shell root becomes
   `row![sidebar, vline, container(column![toast?, scroll(content)]) , rail?]`
   with `SIDEBAR_WIDTH` unchanged and a `RAIL_WIDTH: f32 = 320.0`.
   Delete `notice_view` and the banner; keep `components::notice` for inline callouts in modals.
2. Sidebar footer: a "Notifications" button above Settings, `icons::CIRCLE_DOT`, trailing unread
   badge when `unread > 0`, `.on_press(UiAction::ToggleNotificationRail)`.
3. Rail: header ("Notifications", unread badge, "Clear all" ghost), empty state via
   `components::empty`, then a scrolling column of cards. Card shows tone icon, code/label in mono,
   dismiss X, full message (wraps, never truncated), the command in a `theme::inset` mono block,
   and actions: Copy always when a command exists; Run only when `can_run` and `Idle`;
   "Run this command?" + Confirm/Cancel when `Confirming`; spinner + "Running…" when `Running`;
   Failed badge + Copy (and Run again for a fresh click) when `Failed`.
4. Toast: inline `components::notice(tone, glyph, row![text, X])` above the page content, X →
   `DismissToast(id)`.
5. Fixtures (each must render in the gallery with no CLI): `Notifications · rail unread`,
   `Notifications · rail empty`, `Notifications · toast`, `Notifications · long message`,
   `Notification · action available`, `Notification · confirming`, `Notification · running`,
   `Notification · failed`, `Notifications · job-stream advisory`. Keep `job_failed` working with
   the new shell shape.
6. Tests: `every_fixture_renders` stays green; add assertions that the rail fixtures set
   `unread`, `rail_open`, `toast`, and each action state.

## Task 5 — Wave 1 integration (#29, re-scoped) and verification

Files: `README.md`, `docs/design/contract.md`, `crates/unity-core/tests/live.rs`,
`crates/unity-hub-model/src/action.rs` (dead variants), plus any doc note for the tested CLI version.

1. README: add the Notifications feature bullet; UI notes — rail reflows, toast is inline, no
   overlays; Known limitations — notification history is session-only, no OS notifications.
2. `docs/design/contract.md`: replace "Notice banner: info, success, error" in the Shell inventory
   with the rail, toast, unread badge, empty rail, and the four action states; add a line to the
   Design gallery list.
3. `live.rs` (still `#[ignore = "requires the Unity CLI"]`): add read-only smoke checks for
   `unity doctor`, `unity env`, `unity templates`, `unity status`, and the `projects create`
   command spec (args only, never executed). Record the tested CLI version in the module docs
   (1.0.1 today).
4. Dead-code sweep: every `UiAction` variant must have a handler in `From<UiAction> for Message`
   and a reachable producer; delete any that no longer do (at least `DismissNotice`).
5. Verification: `cargo fmt --all --check`, `cargo test --workspace`,
   `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo build -p unity-hub --no-default-features`, and a gallery fixture-count check.

## Verification per task

Each implementer runs its own focused tests plus `cargo fmt --all --check`; the task reviewer
checks the diff against this plan. The final reviewer runs the whole branch against the three
issues and the global constraints. Only after that does the controller comment on #29 (recording
that Doctor rows are carried by #26–#28) and close #18, #19, #29.
