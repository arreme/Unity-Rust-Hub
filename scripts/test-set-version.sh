#!/usr/bin/env bash
# Test scripts/set-version.sh against a temporary fixture repository.
set -euo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "$(readlink -f "${BASH_SOURCE[0]}")")" && pwd)"
readonly SET_VERSION="$SCRIPT_DIR/set-version.sh"
readonly ROOT="$(mktemp -d)"
readonly FAKE_BIN="$(mktemp -d)"

cleanup() {
  rm -rf "$ROOT" "$FAKE_BIN"
}
trap cleanup EXIT

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

pass() {
  printf 'ok: %s\n' "$*"
}

make_fixture() {
  rm -rf "$ROOT"
  mkdir -p "$ROOT/packaging/arch"

  cat > "$ROOT/Cargo.toml" <<'EOF'
[workspace.package]
version = "0.1.0"
edition = "2021"
EOF

  cat > "$ROOT/Packager.toml" <<'EOF'
name = "unity-hub"
version = "0.1.0"
EOF

  cat > "$ROOT/packaging/arch/PKGBUILD" <<'EOF'
pkgname=unity-rust-hub
pkgver=0.1.0
pkgrel=3
EOF

  cat > "$ROOT/README.md" <<'EOF'
mv UnityRustHub-0.1.0-linux-x86_64.AppImage UnityRustHub.AppImage
EOF

  : > "$ROOT/Cargo.lock"
}

make_fake_cargo() {
  cat > "$FAKE_BIN/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

case "${1:-}" in
  check)
    version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
    cat > Cargo.lock <<LOCK
[[package]]
name = "unity-hub"
version = "$version"
LOCK
    ;;
  test)
    ;;
esac
EOF
  chmod +x "$FAKE_BIN/cargo"
}

make_git_fixture() {
  make_fixture
  git -C "$ROOT" init -q
  git -C "$ROOT" config user.name "Version Test"
  git -C "$ROOT" config user.email "version-test@example.com"
  git -C "$ROOT" config commit.gpgsign false
  git -C "$ROOT" config tag.gpgSign false
  git -C "$ROOT" add .
  git -C "$ROOT" commit -qm "initial"
  git -C "$ROOT" branch -M main
}

make_fake_cargo

# Update-only mode writes every version source.
make_fixture
SET_VERSION_ROOT="$ROOT" "$SET_VERSION" 0.2.0 --no-verify --allow-dirty
grep -qx 'version = "0.2.0"' "$ROOT/Cargo.toml" || fail "Cargo.toml was not updated"
grep -qx 'version = "0.2.0"' "$ROOT/Packager.toml" || fail "Packager.toml was not updated"
grep -qx 'pkgver=0.2.0' "$ROOT/packaging/arch/PKGBUILD" || fail "pkgver was not updated"
grep -qx 'pkgrel=1' "$ROOT/packaging/arch/PKGBUILD" || fail "pkgrel was not reset"
grep -q 'UnityRustHub-0.2.0-linux-x86_64.AppImage' "$ROOT/README.md" || fail "README example was not updated"
pass "update-only mode"

# A leading v is accepted and stripped.
SET_VERSION_ROOT="$ROOT" "$SET_VERSION" v0.2.1 --no-verify --allow-dirty
grep -qx 'version = "0.2.1"' "$ROOT/Cargo.toml" || fail "leading v was not stripped"
pass "leading v"

# Invalid versions are rejected.
if SET_VERSION_ROOT="$ROOT" "$SET_VERSION" not-a-version --no-verify --allow-dirty; then
  fail "invalid version was accepted"
fi
pass "invalid version rejected"

# A stale README example is replaced even when it does not match the manifests.
make_fixture
sed -i 's/0\.1\.0/0.4.0/' "$ROOT/README.md"
SET_VERSION_ROOT="$ROOT" "$SET_VERSION" 0.5.0 --no-verify --allow-dirty
grep -q 'UnityRustHub-0.5.0-linux-x86_64.AppImage' "$ROOT/README.md" ||
  fail "stale README example was not replaced"
pass "stale README example"

# The verification path refreshes and checks Cargo.lock.
make_fixture
SET_VERSION_ROOT="$ROOT" PATH="$FAKE_BIN:$PATH" "$SET_VERSION" 0.2.0
grep -qx 'name = "unity-hub"' "$ROOT/Cargo.lock" || fail "Cargo.lock was not refreshed"
grep -qx 'version = "0.2.0"' "$ROOT/Cargo.lock" || fail "Cargo.lock has the wrong version"
pass "lockfile verification"

# Commit and tag mode works in a clean git repository.
make_git_fixture
SET_VERSION_ROOT="$ROOT" PATH="$FAKE_BIN:$PATH" "$SET_VERSION" 0.2.0 --tag
test "$(git -C "$ROOT" describe --tags --exact-match)" = "v0.2.0" || fail "tag v0.2.0 was not created"
if [[ -n "$(git -C "$ROOT" status --porcelain)" ]]; then
  fail "working tree is dirty after commit and tag"
fi
pass "commit and tag"

# An existing tag is rejected before any files are changed.
if SET_VERSION_ROOT="$ROOT" PATH="$FAKE_BIN:$PATH" "$SET_VERSION" 0.2.0 --tag; then
  fail "existing tag was accepted"
fi
grep -qx 'version = "0.2.0"' "$ROOT/Cargo.toml" || fail "files changed despite existing tag"
pass "existing tag rejected"

# A dirty worktree is rejected before any files are changed.
make_git_fixture
echo "dirty" >> "$ROOT/README.md"
if SET_VERSION_ROOT="$ROOT" PATH="$FAKE_BIN:$PATH" "$SET_VERSION" 0.2.0 --commit; then
  fail "dirty worktree was accepted"
fi
grep -qx 'version = "0.1.0"' "$ROOT/Cargo.toml" || fail "files changed in a dirty worktree"
pass "dirty worktree rejected"

# --allow-dirty cannot be combined with git operations.
if SET_VERSION_ROOT="$ROOT" PATH="$FAKE_BIN:$PATH" "$SET_VERSION" 0.2.0 --commit --allow-dirty; then
  fail "--allow-dirty was accepted with --commit"
fi
pass "--allow-dirty with git rejected"

# --no-verify cannot publish a stale lockfile.
make_git_fixture
if output="$(SET_VERSION_ROOT="$ROOT" "$SET_VERSION" 0.2.0 --release --yes --no-verify 2>&1)"; then
  fail "--release --no-verify was accepted"
fi
[[ "$output" == *"--no-verify cannot be combined"* ]] ||
  fail "unexpected --release --no-verify error: $output"
pass "--release --no-verify rejected"

# --release pushes the branch and tag to origin.
make_git_fixture
REMOTE="$FAKE_BIN/remote.git"
git init --bare -q "$REMOTE"
git -C "$ROOT" remote add origin "$REMOTE"
SET_VERSION_ROOT="$ROOT" PATH="$FAKE_BIN:$PATH" "$SET_VERSION" 0.2.0 --release --yes
test "$(git -C "$REMOTE" tag -l v0.2.0)" = "v0.2.0" || fail "tag was not pushed"
[[ -n "$(git -C "$REMOTE" branch --list main)" ]] || fail "branch was not pushed"
pass "release push"

# A failed commit leaves no staged files behind.
make_git_fixture
git -C "$ROOT" config commit.gpgsign true
git -C "$ROOT" config gpg.format ssh
git -C "$ROOT" config user.signingkey "$ROOT/nonexistent-key"
if SET_VERSION_ROOT="$ROOT" PATH="$FAKE_BIN:$PATH" "$SET_VERSION" 0.2.0 --commit; then
  fail "commit with a broken signing key was accepted"
fi
if [[ -n "$(git -C "$ROOT" diff --cached --name-only)" ]]; then
  fail "failed commit left files staged"
fi
pass "failed commit unstaged"

printf 'set-version tests passed\n'
