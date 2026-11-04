# Shelly-managed AppImage Updates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Linux AppImage carry `X-AppImage-Version` so Shelly can report and install updates from GitHub Releases.

**Architecture:** The release workflow exports the verified version as `VERSION` while `cargo-packager` runs, which makes `appimagetool` embed `X-AppImage-Version` in the AppImage desktop file. A release-time guard extracts the AppImage and fails the build if the field is missing or wrong. The README documents installing a stable-named AppImage through Shelly and configuring `arreme/Unity-Rust-Hub` as its GitHub update source.

**Tech Stack:** GitHub Actions, cargo-packager 0.11.8, appimagetool, Shelly ALPM, Bash.

**Spec:** `docs/superpowers/specs/2026-11-04-shelly-appimage-updates-design.md`

## Global Constraints

- Only `.github/workflows/release.yml`, `README.md`, and the release version fields change. Do not modify Rust source, `Packager.toml` configuration, or packaging desktop entries.
- The release AppImage filename must remain `UnityRustHub-<version>-linux-x86_64.AppImage`.
- Publish exactly one `.AppImage` per release. Do not add a debug, portable, or duplicate stable-name `.AppImage`.
- `VERSION` is the version without the `v` prefix, for example `0.2.1`.
- The installed AppImage filename is `UnityRustHub.AppImage`; it must not be renamed or moved after installation.
- Prereleases are versions containing `-`; Shelly skips them unless `--prerelease` is enabled.
- Do not add an in-app updater, AppImageUpdate/zsync metadata, AUR packaging, or a pacman repository.
- This repository has `commit.gpgsign=true` configured for SSH signing. If the key is locked, unlock it; otherwise use `git -c commit.gpgsign=false commit ...` for plan commits and note that they are unsigned.

## Review Focus

- **Missing metadata:** `VERSION` does not reach `appimagetool`, so Shelly reports `Unknown`. The release guard in Task 1 must fail.
- **Ambiguous assets:** more than one `.AppImage` is published, so Shelly cannot choose one. The release guard in Task 1 must fail unless exactly one exists.
- **Prerelease invisibility:** a prerelease is published but users on the default Shelly channel never see it. Task 2 must document the opt-in.
- **Renamed install:** the AppImage is renamed or moved after installation, so Shelly updates the wrong path. Task 2 must warn against it.
- **Lost asset naming:** the release asset loses its version or architecture token, so Shelly cannot match it. Task 1 must assert the final filename.

---

### Task 1: Embed and verify AppImage version metadata

**Files:**
- Modify: `.github/workflows/release.yml:99-117`

**Interfaces:**
- Consumes: `needs.verify.outputs.version` from the `verify` job, a string such as `0.2.1`.
- Produces: an AppImage whose embedded desktop file contains `X-AppImage-Version=<version>`, plus a workflow step named `Verify AppImage version metadata` that fails on a missing or mismatched field and on a non-single AppImage.

- [ ] **Step 1: Write the failing workflow check**

Run from the repository root:

```bash
grep -A4 'name: Build packages' .github/workflows/release.yml | grep -q 'VERSION:'
```

Expected: exit 1 because the `Build packages` step does not yet set `VERSION`.

- [ ] **Step 2: Export `VERSION` to the packaging step**

Replace the existing `Build packages` step in `.github/workflows/release.yml` with:

```yaml
      - name: Build packages
        env:
          VERSION: ${{ needs.verify.outputs.version }}
        run: cargo packager --config Packager.toml --formats appimage,deb,pacman
```

- [ ] **Step 3: Add the release-time metadata guard**

Insert this step immediately after `Build packages` and before
`Rename packages and add raw binary`:

```yaml
      - name: Verify AppImage version metadata
        shell: bash
        run: |
          set -euo pipefail
          VERSION="${{ needs.verify.outputs.version }}"

          mapfile -t appimages < <(find dist -maxdepth 1 -type f -name '*.AppImage' -print)
          if [[ "${#appimages[@]}" -ne 1 ]]; then
            printf 'expected exactly one AppImage in dist, found %d\n' "${#appimages[@]}" >&2
            printf '%s\n' "${appimages[@]}" >&2
            exit 1
          fi

          appimage="${appimages[0]}"
          extract_dir="$(mktemp -d)"
          trap 'rm -rf "$extract_dir"' EXIT

          (
            cd "$extract_dir"
            "$GITHUB_WORKSPACE/$appimage" --appimage-extract >/dev/null
          )

          desktop="$(find "$extract_dir/squashfs-root" -maxdepth 1 -name '*.desktop' -print -quit)"
          if [[ -z "$desktop" ]]; then
            echo "no .desktop file found in AppImage" >&2
            exit 1
          fi

          grep -qx "X-AppImage-Version=${VERSION}" "$desktop"
```

- [ ] **Step 4: Assert the final asset name**

In the `Rename packages and add raw binary` step, add this line immediately
after the AppImage rename loop and before the Debian rename loop:

```bash
          test -f "dist/UnityRustHub-${VERSION}-linux-x86_64.AppImage"
```

The resulting AppImage section should read:

```yaml
          shopt -s nullglob
          for f in dist/*.AppImage; do
            mv "$f" "dist/UnityRustHub-${VERSION}-linux-x86_64.AppImage"
          done
          test -f "dist/UnityRustHub-${VERSION}-linux-x86_64.AppImage"
          for f in dist/*.deb; do
            mv "$f" "dist/UnityRustHub-${VERSION}-linux-amd64.deb"
          done
```

- [ ] **Step 5: Run the workflow check again**

```bash
grep -A4 'name: Build packages' .github/workflows/release.yml | grep -q 'VERSION:'
```

Expected: PASS.

- [ ] **Step 6: Prove the metadata is actually embedded**

Build a local AppImage with the same environment variable and run the same
extraction check:

```bash
REPO_ROOT="$(pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Packager.toml | head -n1)"

VERSION="$VERSION" cargo packager --config Packager.toml --formats appimage

APPIMAGE="$REPO_ROOT/dist/unity-rust-hub_${VERSION}_x86_64.AppImage"
test -f "$APPIMAGE"

rm -rf /tmp/unity-appimage-check
mkdir -p /tmp/unity-appimage-check
(cd /tmp/unity-appimage-check && "$APPIMAGE" --appimage-extract >/dev/null)

grep -qx "X-AppImage-Version=${VERSION}" \
  /tmp/unity-appimage-check/squashfs-root/unity-rust-hub.desktop
```

Expected: PASS. The embedded desktop file contains
`X-AppImage-Version=0.2.0` until Task 3 bumps the version.

- [ ] **Step 7: Commit**

```bash
git add .github/workflows/release.yml
git -c commit.gpgsign=false commit -m "ci: embed and verify AppImage version metadata"
```

---

### Task 2: Document Shelly installation and updates

**Files:**
- Modify: `README.md:14-25`
- Modify: `README.md:183-199`

**Interfaces:**
- Consumes: the stable installed filename `UnityRustHub.AppImage` and the GitHub repository `arreme/Unity-Rust-Hub`.
- Produces: a README section that tells users how to install through Shelly, configure the update source, check for updates, opt into prereleases, and avoid renaming the installed file.

- [ ] **Step 1: Write the failing documentation checks**

Run from the repository root:

```bash
grep -q '### Updating with Shelly' README.md
grep -q 'shelly sync appimage UnityRustHub arreme/Unity-Rust-Hub GitHub' README.md
grep -q -- '--prerelease' README.md
grep -q 'Do not rename or move the file after installing it' README.md
```

Expected: each command exits 1 because the section and warnings do not exist yet.

- [ ] **Step 2: Add the Shelly section after the Download bullets**

Insert this section after the sentence ending with
"or point Settings at an existing `unity` binary." and before `## Features`:

````markdown
### Updating with Shelly (Arch-based systems)

The AppImage embeds its version so
[Shelly](https://github.com/Seafoam-Labs/Shelly-ALPM) can track updates.
Install it through Shelly instead of copying it into place:

```bash
mv UnityRustHub-0.2.1-linux-x86_64.AppImage UnityRustHub.AppImage
shelly install appimage ./UnityRustHub.AppImage
shelly sync appimage UnityRustHub arreme/Unity-Rust-Hub GitHub
```

Then check for and apply updates with:

```bash
shelly list-updates appimage
shelly upgrade appimage
```

Shelly only tracks AppImages it installed; a manually copied file has no update
entry until you import and configure it.

In the Shelly GUI, open **App Images → Unity Rust Hub → Update source**,
choose **GitHub**, and enter `arreme/Unity-Rust-Hub`. Prereleases are skipped
by default; use `--prerelease` on `shelly sync appimage` or the GUI toggle to
track them.

Use the stable `UnityRustHub.AppImage` filename. Do not rename or move the file
after installing it, or Shelly will lose the update target.
````

- [ ] **Step 3: Make the local packaging command produce metadata**

Replace the Linux packaging command in `### Packaging a release locally` with:

```bash
# Linux: AppImage, Debian package, pacman package.
# VERSION makes appimagetool embed X-AppImage-Version for Shelly.
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Packager.toml | head -n1)" \
  cargo packager --config Packager.toml --formats appimage,deb,pacman
```

Keep the Windows and macOS commands unchanged.

- [ ] **Step 4: Run the documentation checks again**

```bash
grep -q '### Updating with Shelly' README.md
grep -q 'shelly sync appimage UnityRustHub arreme/Unity-Rust-Hub GitHub' README.md
grep -q -- '--prerelease' README.md
grep -q 'Do not rename or move the file after installing it' README.md
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add README.md
git -c commit.gpgsign=false commit -m "docs: add Shelly AppImage update instructions"
```

---

### Task 3: Bump the release version to 0.2.1

**Files:**
- Modify: `Cargo.toml:13`
- Modify: `Packager.toml:18`
- Modify: `Cargo.lock` for the `unity-hub` workspace package version

**Interfaces:**
- Consumes: the current version `0.2.0`.
- Produces: workspace, lockfile, and packager versions all equal to `0.2.1`, ready for tag `v0.2.1`.

- [ ] **Step 1: Write the failing version check**

```bash
CARGO_VERSION="$(cargo metadata --no-deps --format-version 1 \
  | jq -r '.packages[] | select(.name == "unity-hub") | .version')"
PKG_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Packager.toml | head -n1)"

test "$CARGO_VERSION" = "0.2.1"
test "$PKG_VERSION" = "0.2.1"
```

Expected: both tests fail because both versions are still `0.2.0`.

- [ ] **Step 2: Update both version fields**

Change `[workspace.package] version` in `Cargo.toml` from `0.2.0` to `0.2.1`:

```toml
[workspace.package]
version = "0.2.1"
edition = "2021"
license = "MIT"
```

Change `version` in `Packager.toml` from `0.2.0` to `0.2.1`:

```toml
version = "0.2.1"
```

- [ ] **Step 3: Refresh the lockfile**

```bash
cargo check --workspace
```

Expected: the command succeeds and `Cargo.lock` records version `0.2.1` for
`unity-hub`.

- [ ] **Step 4: Run the version check again**

```bash
CARGO_VERSION="$(cargo metadata --no-deps --format-version 1 \
  | jq -r '.packages[] | select(.name == "unity-hub") | .version')"
PKG_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Packager.toml | head -n1)"

test "$CARGO_VERSION" = "0.2.1"
test "$PKG_VERSION" = "0.2.1"
grep -A2 '^name = "unity-hub"$' Cargo.lock | grep -q '^version = "0.2.1"$'
```

Expected: PASS.

- [ ] **Step 5: Run the full verification suite**

```bash
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Expected: all commands succeed.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock Packager.toml
git -c commit.gpgsign=false commit -m "chore: release 0.2.1"
```

---

### Task 4: Release and verify 0.2.1

**Files:**
- No source changes.

**Interfaces:**
- Consumes: commits from Tasks 1-3 and tag `v0.2.1`.
- Produces: a GitHub release with exactly one Linux AppImage carrying `X-AppImage-Version=0.2.1`, plus a Shelly installation that reports version `0.2.1`.

**Important:** Pushing the tag publishes a release. Ask the user for explicit
go-ahead before pushing.

- [ ] **Step 1: Confirm the branch is clean and verified**

```bash
git status --short
cargo test --workspace --locked
```

Expected: no unexpected modified files and all tests pass.

- [ ] **Step 2: Push the implementation commits**

Push `main` using the repository's normal review/merge flow. Do not tag until
the changes are on the commit that should be released.

- [ ] **Step 3: Create and push the release tag**

```bash
git tag -a v0.2.1 -m "Unity Rust Hub 0.2.1"
git push origin v0.2.1
```

Expected: the release workflow starts.

- [ ] **Step 4: Watch the release workflow**

```bash
gh run watch
```

Or open the **Actions** tab and wait for the `Release` workflow to finish.

Expected: the `Verify AppImage version metadata` step passes, and the release
publishes successfully.

- [ ] **Step 5: Verify the release assets**

Open the `v0.2.1` release page and confirm:

- exactly one asset ends in `.AppImage`;
- its name is `UnityRustHub-0.2.1-linux-x86_64.AppImage`;
- a `SHA256SUMS` asset exists.

Download the AppImage and extract it locally:

```bash
cd ~/Downloads
APPIMAGE="$PWD/UnityRustHub-0.2.1-linux-x86_64.AppImage"

rm -rf /tmp/unity-release-check
mkdir -p /tmp/unity-release-check
(cd /tmp/unity-release-check && "$APPIMAGE" --appimage-extract >/dev/null)

grep -qx 'X-AppImage-Version=0.2.1' \
  /tmp/unity-release-check/squashfs-root/unity-rust-hub.desktop
```

Expected: PASS.

- [ ] **Step 6: Install through Shelly and verify the reported version**

```bash
cd ~/Downloads
mv UnityRustHub-0.2.1-linux-x86_64.AppImage UnityRustHub.AppImage

shelly install appimage ./UnityRustHub.AppImage
shelly sync appimage UnityRustHub arreme/Unity-Rust-Hub GitHub
shelly list appimage
shelly list-updates appimage
```

Expected:

- `shelly list appimage` shows Unity Rust Hub with version `0.2.1`;
- `shelly list-updates appimage` reports no update.

The following release verifies update detection and in-place replacement.
