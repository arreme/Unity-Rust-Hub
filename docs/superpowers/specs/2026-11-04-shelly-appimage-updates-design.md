# Shelly-managed AppImage updates

**Date:** 2026-11-04
**Status:** Design approved in chat; written spec pending review
**Scope:** Linux AppImage distribution through Shelly

## Summary

Make the release AppImage carry an embedded application version so Shelly can
track it. Users install the AppImage once through Shelly and configure the GitHub
repository `arreme/Unity-Rust-Hub` as its update source. Shelly then reports and
installs future releases without requiring an in-app updater, a background
service, or manual asset selection.

The application source does not change. The work is limited to the release
workflow, a release-time verification step, and user documentation.

## Context

Shelly reads an installed AppImage's version from `X-AppImage-Version=` in the
AppImage's embedded `.desktop` file. The current `v0.2.0` AppImage does not
contain that field, so Shelly reports the version as `Unknown`.

Shelly's GitHub update source:

- queries the repository's GitHub releases API;
- skips prereleases by default;
- selects exactly one architecture-compatible `.AppImage` asset;
- downloads the full AppImage and replaces the installed file in place.

No `.zsync` file or AppImageUpdate metadata is required for the Shelly path.

## Goals

- Shelly displays the real installed Unity Rust Hub version.
- Shelly detects a newer non-prerelease GitHub release without manual asset
  selection.
- Shelly updates the existing `UnityRustHub.AppImage` file in place.
- A broken or missing version field fails the release workflow before publishing.
- The README explains the one-time Shelly setup and the ongoing update commands.

## Non-goals

- An in-app update checker or update banner.
- An in-app self-updater.
- AUR packaging or a self-hosted pacman repository.
- AppImageUpdate/zsync support.
- Code signing or GPG verification of release assets.
- Changes to the Rust crates, the `Packager.toml` configuration (other than the
  release version bump), or the desktop entry used by the pacman and Debian
  packages.

## User flow

1. Download the latest
   `UnityRustHub-<version>-linux-x86_64.AppImage` from GitHub Releases.
2. Rename it to `UnityRustHub.AppImage` before installing.
3. Install it through Shelly:
   `shelly install appimage ./UnityRustHub.AppImage`.
4. Configure the update source once:
   `shelly sync appimage UnityRustHub arreme/Unity-Rust-Hub GitHub`.
5. Shelly reads the installed version from `X-AppImage-Version`.
6. On a later release, Shelly reports an update. `shelly upgrade appimage`, the
   GUI's "Update All", or the AppImage's update action replaces the file at the
   same path.

The GUI equivalent is: App Images → Unity Rust Hub → update source → GitHub →
`arreme/Unity-Rust-Hub`.

## Architecture

### Version metadata

`appimagetool` reads the `VERSION` environment variable and writes its value as
`X-AppImage-Version` into the AppImage's embedded desktop file. The release
workflow already verifies the release tag against the workspace version, so the
same version is exported to the packaging step.

Example release step:

```yaml
- name: Build packages
  env:
    VERSION: ${{ needs.verify.outputs.version }}
  run: cargo packager --config Packager.toml --formats appimage,deb,pacman
```

The existing asset rename remains unchanged:

```text
UnityRustHub-<version>-linux-x86_64.AppImage
```

### Release-time guard

Immediately after packaging and before renaming, a workflow step:

1. asserts that exactly one `.AppImage` exists in `dist/`;
2. extracts it with `--appimage-extract`;
3. fails unless the embedded desktop file contains exactly
   `X-AppImage-Version=<version>`.

This guard prevents a future change to `appimagetool`, `cargo-packager`, or the
workflow environment from silently publishing an AppImage that Shelly cannot
version-track.

### Documentation

Add an "Updating with Shelly" subsection to `README.md` with:

- the stable-filename rename;
- the install command;
- the one-time update-source command and GUI equivalent;
- `shelly list-updates appimage` and `shelly upgrade appimage`;
- a note that prereleases require an explicit opt-in;
- a note not to rename the AppImage after installing it.

## Components and file changes

| File | Change |
|---|---|
| `.github/workflows/release.yml` | Export `VERSION` to the Linux packaging step; add the AppImage metadata guard. |
| `README.md` | Document Shelly installation, update-source setup, and update commands. |

No application source, manifest, or packaging configuration changes are needed.

## Edge cases and failure behavior

- **Current `v0.2.0` asset:** lacks metadata. Prefer a `v0.2.1` release to test
  the new pipeline. Re-releasing `v0.2.0` requires replacing the AppImage and
  `SHA256SUMS` assets under the existing tag.
- **Prereleases:** versions containing `-` are published as GitHub prereleases
  and are skipped by Shelly unless `--prerelease` is enabled.
- **Ambiguous assets:** Shelly requires exactly one compatible `.AppImage`.
  Do not publish a debug or portable `.AppImage` alongside the main asset, and
  do not add a duplicate stable-name copy. Raw binaries, `.deb`, `.pkg.tar.zst`,
  and `.flatpak` assets are ignored.
- **Future architectures:** an `aarch64` asset must contain `aarch64` or `arm64`
  in its filename so Shelly's architecture filter can select it.
- **Stable install name:** `UnityRustHub.AppImage` keeps the update target stable.
  A versioned installed filename can still match releases through version-token
  normalization, but it leaves the installed file versioned and is less clear.
- **Installed outside Shelly:** only AppImages installed through
  `shelly install appimage` (or the GUI) are tracked. A manually copied AppImage
  has no update entry until it is imported and configured.
- **Renaming after install:** Shelly stores the install path. Renaming or moving
  the file after installation breaks update replacement.
- **Missing metadata regression:** the release guard fails the build instead of
  publishing an untrackable AppImage.
- **Trust boundary:** Shelly downloads over GitHub HTTPS and does not consume
  the release's `SHA256SUMS` for AppImage updates. This is acceptable for this
  scope but is not equivalent to signed packages.

## Verification

### Local metadata check

```bash
VERSION="$(cargo metadata --no-deps --format-version 1 \
  | jq -r '.packages[] | select(.name == "unity-hub") | .version')"

REPO_ROOT="$(pwd)"
VERSION="$VERSION" cargo packager --config Packager.toml --formats appimage

rm -rf /tmp/unity-appimage-check
mkdir -p /tmp/unity-appimage-check
(cd /tmp/unity-appimage-check && \
  "$REPO_ROOT/dist/unity-rust-hub_${VERSION}_x86_64.AppImage" --appimage-extract)

grep -qx "X-AppImage-Version=${VERSION}" \
  /tmp/unity-appimage-check/squashfs-root/unity-rust-hub.desktop
```

### CI

The release workflow must fail if the AppImage is missing, extraction fails, or
the embedded version field is absent or different from the release version.

### End-to-end smoke test

After the next release, on CachyOS:

```bash
mv UnityRustHub-0.2.1-linux-x86_64.AppImage UnityRustHub.AppImage
shelly install appimage ./UnityRustHub.AppImage
shelly sync appimage UnityRustHub arreme/Unity-Rust-Hub GitHub
shelly list appimage          # should show 0.2.1
shelly list-updates appimage  # should show no update
```

The following real release verifies update detection and in-place replacement.

## Rollout

1. Merge the workflow and README changes.
2. Bump the workspace version, `Cargo.lock`, and `Packager.toml` to `0.2.1`.
3. Tag `v0.2.1`, push the tag, and let the release workflow build and verify.
4. Confirm the release contains exactly one Linux AppImage and a matching
   `SHA256SUMS`.
5. Install it through Shelly, configure the update source, and verify the
   reported version.
6. Optionally add a release note: "AppImage now carries version metadata for
   Shelly update tracking."

Re-releasing `v0.2.0` is possible by deleting the current AppImage and
`SHA256SUMS` assets and rerunning the release workflow for that tag. A `0.2.1`
release is preferred because it tests the full pipeline without mutating an
already-published release.

## Success criteria

- Shelly shows the installed version instead of `Unknown`.
- Shelly detects a newer non-prerelease release without manual asset selection.
- The update replaces the same `UnityRustHub.AppImage` path.
- The release workflow rejects an AppImage without the expected version field.
- The application's runtime behavior, settings, and packaging configuration are
  unchanged.
