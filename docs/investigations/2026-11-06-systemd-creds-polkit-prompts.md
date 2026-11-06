# Investigation: repeated `systemd-creds` polkit prompts

**Issue:** App triggers repeated polkit prompts by calling `systemd-creds` on every
login/use.
**Date:** 2026-11-06
**Verdict:** The calls come from the **Unity CLI auth broker**, not from Unity Rust
Hub. The Hub never invokes `systemd-creds`. This repository can stop amplifying the
problem, but the repeated prompts can only be fixed in the Unity CLI.

## 1. What this repository does

A repository-wide search finds no `systemd-creds`, `unity-auth-broker-seal`,
`host+tpm2`, or `LoadCredentialEncrypted` call site in any crate:

```bash
rg -n 'systemd-creds|unity-auth-broker|host\+tpm2|LoadCredentialEncrypted' .
```

The only match is the bundled Unity CLI reference at
`.agents/skills/unity-cli/references/auth-license-cloud.md`, which describes the
CLI's own auth broker.

The Hub only spawns the official `unity` binary:

- `crates/unity-core/src/runner.rs` builds `CommandSpec` argv and runs read commands.
- `crates/unity-hub-controller/src/jobs.rs` runs streamed mutations.
- No crate builds a shell string, and no crate knows about credential sealing.

All Hub reads are serialized through one `cli-reader` worker
(`crates/unity-hub-controller/src/util.rs`), and
`blocking_reads_are_serialized` asserts that no two reads overlap. The controller
has no periodic CLI timer: `App::subscription()` returns `Subscription::none()`
unless a job is running. Reads happen at boot, on page navigation, on an explicit
Refresh/Retry, or after a job finishes.

At boot the Hub runs `unity version` first, then queues the Overview reads:
`self-update --check`, `auth status`, `license status`, `editors running`,
`editors --installed`, and `projects`. The worker runs those one at a time. Only
`auth status`, `license status`, and (when signed in) some project/cloud lookups
can cause the CLI to touch the auth broker; the Hub itself adds no auth work.

**Repo-side defect found while investigating:** page navigation used to restart a
read that had already failed. `load_overview_data(false)`, `load_editors_page`,
`load_projects_page`, and `load_diagnostics_page` only checked `data.is_none()`,
not `error.is_none()`. Navigating back to Overview after a denied authorization
therefore started `auth status`/`license status` again, which could start the
broker and prompt again. This contradicted the README's "a failed command never
triggers an automatic retry" rule. It is fixed in this change: a failed read now
waits for the user's Retry action or an explicit Refresh. The new test is
`navigating_back_does_not_retry_a_failed_read`.

## 2. What the Unity CLI does

The local CLI is `1.0.0-beta.17` (`unity --version`). Its auth broker is a
resident process started on demand and exiting after 120 seconds idle. The CLI's
own log records this lifecycle:

```text
Auth broker listening (activated=False, idleMs=120000)
Auth broker is idle; exiting
```

The binary contains the sealing implementation and its probe names:

```text
unity-seal-probe
seal-tier tier=
systemd-creds
unity-auth-broker-seal
--with-key=host+tpm2
systemd-creds-round-trip-failed
systemd-creds-encrypt-unavailable
tier-not-in-chain
no-tier-available
availableTiers
skippedTiers
```

The journal matches those strings. A burst is a decrypt plus several encrypts,
each in a new transient `systemd-creds@...service` unit:

```text
systemd-creds decrypt --name=unity-auth-broker-seal - -
systemd-creds encrypt --name=unity-auth-broker-seal --with-key=host+tpm2 - -
```

The store itself is **not** sealed by `systemd-creds` on the affected machine.
The header of `~/.local/share/UnityAuthBroker/token-store.seal` names the
`linux-secret-service` tier, and a probe-suppressed `unity doctor` reports:

```json
"authBroker": {
  "seal": {
    "tier": "linux-secret-service",
    "isHardwareBacked": false,
    "availableTier": "linux-tpm2-systemd-creds",
    "skippedTiers": []
  }
}
```

The evidence indicates that `systemd-creds` is being used as a **hardware-tier
probe / migration attempt**, not as the storage that actually holds the token.
The probe tries the system-scope `host+tpm2` key, polkit denies or the user
cancels, the CLI falls back to Secret Service, and the same probe runs again the
next time the broker starts or `unity doctor` probes the credential store.

The doctor card makes the same distinction:

- **Address** `/run/user/1000/unity/auth-broker.sock` is the broker's IPC
  socket, not a storage backend.
- **Credentials** "Your operating system credential store" plus **Store
  protection** "Secret Service keyring" means the keyring path is working.
- **Hardware-sealed** "No" plus **Upgrade available** "TPM2 (sealed to this
  machine)" is the optional upgrade that invokes systemd-creds.
- **Sign-in key** "None stored. A running broker keeps one in memory" and the
  note that the broker "seals a credential only once a command first needs a
  token" describe the lazy seal. It is not a keyring failure.

beta.17 has no supported switch to pin the store to Secret Service or disable
the TPM2 upgrade. `UNITY_NO_AUTH_BROKER=1` bypasses the broker, but doctor still
runs its seal-tier probe (verified with the PATH sandbox). `UNITY_KEYRING_FILE_STORE`
appears to select a file-backed keyring fallback; it is not a seal-tier control
and would weaken storage, so do not use it as a workaround.

## 3. Why every call needs polkit

`systemd-creds` in system scope uses the root-only host key
`/var/lib/systemd/credential.secret` and/or the TPM2 SRK. The privileged part is
exposed through `systemd-creds.socket` and the `systemd-creds@.service` transient
unit. The shipped policy
`/usr/share/polkit-1/actions/io.systemd.credentials.policy` uses:

```xml
<action id="io.systemd.credentials.encrypt">
  <defaults>
    <allow_any>auth_admin_keep</allow_any>
    <allow_inactive>auth_admin_keep</allow_inactive>
    <allow_active>auth_admin_keep</allow_active>
  </defaults>
</action>
<action id="io.systemd.credentials.decrypt">
  <defaults>
    <allow_any>auth_admin_keep</allow_any>
    <allow_inactive>auth_admin_keep</allow_inactive>
    <allow_active>auth_admin_keep</allow_active>
  </defaults>
</action>
```

`auth_admin_keep` means an administrative password is required, kept only for a
brief period (polkit's default is 300 seconds). The "keep" cache is keyed by
action and **subject**; each `systemd-creds` invocation is a new
`unix-process:<pid>`, and the journal shows the PID changing on every call.
polkit may treat processes as equivalent when they share UID, PPID, cgroup, and
controlling terminal, so repeated calls from the same terminal can reuse a
recent authorization for about five minutes. Broker starts from a different
process/cgroup generally will not, and `encrypt` and `decrypt` are separate
actions, so a successful authentication does not reliably cover the next
invocation.

The CLI is using system scope even for a **per-user** broker. That is the core
mistake. On systemd 256 and later, `systemd-creds` supports a user scope:

```text
--user   Select user-scoped credential encryption
--uid=   Select user for scoped credentials (implies --user)
```

`man systemd-creds` (local systemd 262.1) says `--user` was added in version 256
and that it incorporates the selected user's numeric UID, username, and the
system's `machine-id` into the encryption key. `--user` implies `--uid=self`.

`--user` does **not** create a separate user-owned key. The root
`systemd-creds@.service` still uses the system host key and/or TPM2 key; the
UID, username, and machine-id are mixed into the derived key. What changes is
the authorization check: the service skips polkit when the request is
user-scoped, the requested UID equals the caller's UID, and the timestamp is
fresh (within 30 seconds). That own-scope/fresh bypass is why this local test
succeeds with no prompt:

```bash
env HOME=/tmp/... XDG_CONFIG_HOME=/tmp/... \
  systemd-creds --user encrypt --name=unity-auth-broker-seal \
  --with-key=host+tpm2 plain sealed
env HOME=/tmp/... XDG_CONFIG_HOME=/tmp/... \
  systemd-creds --user decrypt --name=unity-auth-broker-seal sealed -
```

Both commands exited 0 with no polkit prompt. The security consequence is
important: the boundary is the **UID**, not the process. Any process running as
the same user can ask the service to decrypt the same user-scoped credential
without polkit. For stronger process isolation, a system service with a
dedicated UID and `LoadCredentialEncrypted=` is better.

On systemd versions older than 256 there is no `--user` and no
`io.systemd.credentials.*` polkit policy; an unprivileged system-scope host-key
operation simply fails because the host secret is root-only. In that case a
per-user CLI should not probe system scope at all, and should use Secret Service
or a system-level broker.

One v262 implementation detail: the unprivileged varlink client does not
forward the requested `withKey` policy; it sends scope and UID, and the service
uses `auto`/`auto-scoped`. The argv still shows `--with-key=host+tpm2`, but the
exact key selection is decided service-side. The polkit decision is about the
system scope, not the specific key.

## 4. Why the calls repeat

The decisive evidence is a PATH sandbox: a fake `systemd-creds` that logs its
argv and exits immediately. Only `unity doctor` invoked it. The fake exits on
the first encrypt, so the real run's decrypt/round-trip and retries account for
the 2-4 calls per burst. These commands invoked it zero times:

- `unity auth status` (with and without `UNITY_NO_AUTH_BROKER=1`)
- `unity license status` (with and without `UNITY_NO_AUTH_BROKER=1`)
- `unity diagnose proxy` (with and without `UNITY_NO_AUTH_BROKER=1`)
- `unity env` (with and without `UNITY_NO_AUTH_BROKER=1`)
- `unity editors running`
- `unity projects`
- `unity self-update --check`

Neither `UNITY_CLI_INTERNAL_NO_CREDENTIAL_PROBE=1` nor
`UNITY_NO_AUTH_BROKER=1` stopped doctor's seal-tier probe; both still produced
the `systemd-creds encrypt --name=unity-auth-broker-seal --with-key=host+tpm2 - -`
call. The internal flag only skips doctor's separate `credential-store` check.

That maps directly to the Hub: the Diagnostics page used to run `unity doctor`
automatically on every page open and Refresh. The journal's 12:35-12:43 bursts
line up with the Diagnostics page's `diagnose-proxy-*.txt` files. The broker
also exits after two idle minutes and can re-probe on a later start, but the
Hub's automatic doctor runs were a direct, repeatable trigger.

The overlapping calls are the dangerous part: cancelled prompts feed
`pam_faillock`, and repeated failures can lock the account out for ten minutes.
The journal at 11:29 local shows two encrypts about half a second apart, and
polkit reports `Another client is already authenticating, please try again
later.`

## 5. Options evaluated

| Option | Assessment |
| --- | --- |
| Decrypt once at startup, keep the secret in memory | Correct pattern, and the broker already tries to be resident. It does not help if every broker restart re-probes the hardware tier, or if the probe runs before the store is loaded. |
| Re-encrypt only when the secret changes | Correct, but the problematic calls here are probes, not store writes. The real store is Secret Service and is only rewritten on a token refresh. |
| Serialize `systemd-creds` calls | Necessary, but not sufficient. The broker has its own stale-socket lock; the probe can still run before/outside it, and multiple broker starts can probe concurrently. |
| Use a user-scoped credential key | **Best upstream fix for the per-user broker.** `systemd-creds --user` (systemd >= 256) makes the request own-scope and skips polkit for fresh requests, while still using the system host/TPM key. Trade-off: any process with the same UID can decrypt the credential; use a system service with a dedicated UID and `LoadCredentialEncrypted=` for stronger isolation. |
| Use Secret Service / KWallet / libsecret | Already the effective fallback (`linux-secret-service`). It is encrypted at rest, but it is not hardware-backed. If the CLI chooses this tier, it should stop probing systemd-creds on every start. |
| Run sealing from a system service with privileges | This is the CLI's other hardened tier: a system-UID broker with `/var/lib/unity-auth-broker`, `/run/unity-auth-broker/socket`, `unity-auth-broker.socket`, and `com.unity3d.authbroker.policy`. The current beta.17 install does not provide those units/policy on this machine, and the CLI exposes only `unity auth broker ensure`, not an install command. Worth pursuing upstream, but not something the Hub can install. |
| Fail gracefully and never retry a denied authorization | Necessary. A denial should be cached for the session (or persisted as "systemd-creds unavailable") and should not cause another prompt until the user explicitly changes the setting. |
| Ship a polkit rule that auto-approves `io.systemd.credentials.encrypt/decrypt` | **Do not do this.** It would turn the root service into a TPM+host-key encryption/decryption oracle for any process in the active session. If a rule is ever unavoidable, scope it to the specific user and action IDs and document the trade-off. |

## 6. What was changed here

`crates/unity-hub-controller/src/app.rs`:

- Added `Loadable::should_start(force)`: a read starts automatically only if it
  has never run; a failed read waits for Retry or an explicit Refresh.
- Applied it to the Overview, Editors, Projects, and Diagnostics page loaders.
- Added `navigating_back_does_not_retry_a_failed_read`.
- Removed `unity doctor` from the automatic Diagnostics load. It now starts
  only through the explicit `LoadDoctor` action (Run doctor / Refresh doctor) or
  the existing Retry action. Added `load_doctor_action_starts_the_opt_in_read`
  and `refreshing_diagnostics_does_not_rerun_doctor`.

`crates/unity-hub-model/src/action.rs` and
`crates/unity-hub-ui/src/pages/diagnostics.rs`:

- Added `UiAction::LoadDoctor`.
- The idle doctor card explains that doctor's credential probe can trigger a
  polkit password prompt and offers **Run doctor**; a loaded card offers
  **Refresh doctor**.
- The page-level Refresh re-runs the automatic sections only. It never re-runs
  doctor.

`docs/design/contract.md`, `README.md`, and the gallery fixtures were updated to
match. The Hub no longer triggers doctor's `systemd-creds` probe automatically,
and a denied auto-read no longer retries on navigation. It cannot affect prompts
caused by the Unity Editor, plugins, terminal commands, or a manual doctor run.

## 7. Recommended upstream fix (Unity CLI)

1. **Use `--user` for the per-user broker.** Change the probe/encrypt/decrypt
   path to `systemd-creds --user` (or `--uid=self`) on systemd >= 256. This does
   not create a separate per-user key; it still uses the system host/TPM key,
   but the root service skips polkit for a fresh own-UID request. Document the
   trade-off: any process with the same UID can decrypt the credential.
2. **Prefer a system service with `LoadCredentialEncrypted=`.** A system-UID
   broker that loads an encrypted credential at service activation gets the
   strongest isolation: the system manager decrypts once, the broker keeps the
   plaintext only in memory, and no per-call polkit prompt is involved. This is
   the systemd-native pattern for a long-running auth broker.
3. **Treat an authorization denial as terminal.** Cache "systemd-creds is not
   authorized" in the broker state (and, if appropriate, on disk) so a denied
   prompt does not repeat on the next broker start. `systemd-creds` itself does
   not retry after a denial; the repeated calls are the CLI's own re-probing.
4. **Serialize probe and broker startup.** The probe should run under the same
   lock as broker startup, so two clients cannot launch `systemd-creds` at once.
5. **Only re-seal when the store changes.** Loading a valid store should not
   rewrite it; a token refresh should re-seal once.
6. **Fall back cleanly.** On systemd < 256, or when the user denies
   authorization, select Secret Service without re-probing system scope.
7. **Report the tier consistently.** `unity doctor` already exposes
   `authBroker.seal.tier`, `availableTier`, `skippedTiers`,
   `systemUnitInstalled`, and `policyFileInstalled`; the fix should make those
   fields reflect the chosen tier instead of a repeatedly attempted one.

## 8. Workarounds (and their trade-offs)

- **Update the Unity CLI** when a release contains the fix. This is the only
  complete fix.
- **Do not add a blanket polkit rule.** If you need silence from every Unity
  process today, an opt-in rule scoped to one user and the two action IDs is:

  ```js
  // /etc/polkit-1/rules.d/49-unity-auth-broker.rules
  polkit.addRule(function(action, subject) {
      if ((action.id == "io.systemd.credentials.encrypt" ||
           action.id == "io.systemd.credentials.decrypt") &&
          subject.local && subject.active &&
          subject.user == "<your-user>") {
          return polkit.Result.YES;
      }
  });
  ```

  Restart polkit after installing it, and remove the file plus restart polkit
  when the CLI fix lands. Trade-off: any process running as that user can ask
  the root service to encrypt/decrypt system credentials; it becomes a
  TPM+host-key oracle. Do not ship this in a package or enable it for all users.
- **`UNITY_NO_AUTH_BROKER=1`** is the CLI's documented escape hatch: it skips the
  resident broker and reads credentials directly from the OS keyring. It does
  **not** stop `unity doctor`'s seal-tier probe (verified with the PATH sandbox),
  and it bypasses the broker's audit trail and peer checks, so treat it as an
  opt-in workaround, not a default.
- **Do not run `unity doctor` repeatedly** while diagnosing this; doctor performs
  a credential probe by default. In the Hub, doctor is now opt-in: the page no
  longer runs it on open or Refresh, and a failed doctor read waits for an
  explicit Retry.
- **If `pam_faillock` locked the account:**
  `faillock --user <user> --reset` (as root), or wait for the ten-minute lockout
  to expire.

## 9. Acceptance criteria mapping

| Criterion | Status |
| --- | --- |
| No polkit prompts at login or during normal use | **Hub fixed for automatic reads.** `unity doctor` is opt-in, and the page Refresh no longer runs it. A manual doctor run, the Unity Editor, plugins, and terminal commands can still prompt; the upstream CLI fix is needed for those. |
| No overlapping `systemd-creds` invocations | **Hub fixed; upstream remains.** The Hub serializes all `unity` processes and no longer starts doctor automatically. The CLI can still race a manual doctor run against another client. |
| Secret handling still encrypted at rest | **Yes.** The effective tier is Secret Service; the recommended `--user` tier is encrypted at rest, TPM-backed where available, and UID-scoped (not process-scoped). |
| Failed/denied authorization does not cause retry storms | **Hub fixed.** Failed reads now wait for Retry/Refresh. The CLI still needs to cache a denial. |
| Root-cause write-up in the PR | This document; see the PR summary below. |

## 10. Short PR summary

> The polkit prompts come from the Unity CLI 1.0.0-beta.17 auth broker, not from
> Unity Rust Hub. The Hub never invokes `systemd-creds`; it only runs `unity` and
> serializes those reads. The CLI's seal-tier probe (run during broker startup
> and by `unity doctor`'s credential probe) tries
> `systemd-creds encrypt/decrypt --with-key=host+tpm2` in **system** scope, which
> requires `auth_admin_keep` authorization from
> `io.systemd.credentials.encrypt/decrypt`, then falls back to Secret Service.
> Because the broker exits after two idle minutes and does not remember a denied
> probe, every later broker start prompts again; concurrent starts also produce
> polkit's "Another client is already authenticating" errors.
>
> This change fixes the Hub-side triggers: failed reads are no longer restarted
> automatically on page navigation, and the Diagnostics page no longer runs
> `unity doctor` automatically. Doctor is the only Hub command that invokes
> `systemd-creds`; it is now opt-in, with a warning that its credential probe can
> trigger a polkit password prompt. The complete fix belongs in the Unity CLI:
> use `systemd-creds --user` for the per-user broker (systemd >= 256; it still
> uses the host/TPM key but avoids polkit for fresh own-UID requests), or better,
> a system service with a dedicated UID and `LoadCredentialEncrypted=`. Also
> cache an authorization denial as terminal, serialize probe/startup, and only
> re-seal when the token changes. No polkit auto-approve rule is shipped.

## 12. Open questions

- **What owns `unity-auth-broker-seal`?** The Unity CLI itself. The string,
  the `systemd-creds` argv, and the `apps/cli/crates/auth-broker/src/linux_tier/polkit.rs`
  path are all embedded in `/home/<user>/.local/bin/unity`
  (`1.0.0-beta.17`), and the broker logs under
  `~/.config/unityhub/logs/` record the matching broker lifecycle. It is not a
  systemd, KDE, or Arch component.
- **Does the secret need to survive reboots?** Yes. The current store is a
  persistent file, `~/.local/share/UnityAuthBroker/token-store.seal`, and the
  broker's `seal-key` is persistent too. A user-scoped `systemd-creds` key is
  also persistent: it is bound to the machine-id, UID, and username, not to the
  current login session. A session-only key (kernel keyring) would lose the
  sign-in on reboot and is not required by the current design.

## 13. References

Local primary sources used for this investigation:

- `/usr/share/polkit-1/actions/io.systemd.credentials.policy`
- `man systemd-creds` (systemd 262.1)
- `/usr/share/doc/systemd/NEWS` (`--user/--uid=` added in 256; Varlink
  `systemd-creds.socket` path)
- `/usr/lib/systemd/system/systemd-creds.socket`
- `/usr/lib/systemd/system/systemd-creds@.service`
- `journalctl -b` for the transient `systemd-creds@...service` units and polkit
  denials
- `~/.config/unityhub/logs/cli-log.json*` for broker startup/idle/exit and peer
  activity
- `~/.config/unityhub/logs/broker-audit.json` for token refresh events
- `~/.local/share/UnityAuthBroker/token-store.seal` header (`UNTYSEAL`, tier
  `linux-secret-service`)
- `unity doctor --format json` with
  `UNITY_CLI_INTERNAL_NO_CREDENTIAL_PROBE=1 UNITY_NO_AUTH_BROKER=1`

Upstream references:

- systemd `systemd-creds(1)`: <https://www.freedesktop.org/software/systemd/man/latest/systemd-creds.html>
- systemd credentials: <https://systemd.io/CREDENTIALS/>
- `systemd.exec(5)`, `LoadCredentialEncrypted=`: <https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html#Credentials>
- systemd v262 `src/creds/creds.c` (polkit checks, own-scope bypass, timestamp
  freshness): <https://github.com/systemd/systemd/blob/v262/src/creds/creds.c>
- systemd v262 `src/shared/creds-util.c` (varlink IPC, key derivation):
  <https://github.com/systemd/systemd/blob/v262/src/shared/creds-util.c>
- systemd v262 `src/creds/io.systemd.credentials.policy`:
  <https://github.com/systemd/systemd/blob/v262/src/creds/io.systemd.credentials.policy>
- systemd v262 `units/systemd-creds.socket` and `units/systemd-creds@.service`:
  <https://github.com/systemd/systemd/blob/v262/units/systemd-creds.socket>
- polkit authorization results and `_keep` caching:
  <https://polkit.pages.freedesktop.org/polkit/polkit.8.html>
- polkit backend cache expiration:
  <https://github.com/polkit-org/polkit/blob/main/src/polkitbackend/polkitbackendinteractiveauthority.c>
- systemd commits adding the polkit actions and `--user`:
  <https://github.com/systemd/systemd/commit/caef0bc3dcd3b7d6883043529baac02cdb6b898c>,
  <https://github.com/systemd/systemd/commit/8464f7cbd652af75566017c62bec5308d1c30775>,
  <https://github.com/systemd/systemd/commit/19f16c9935c49fa93d1e94178238bf4f76c48fa3>,
  <https://github.com/systemd/systemd/commit/c85b68f630852d38d5562ce5e2be36a22e3e7f9b>
