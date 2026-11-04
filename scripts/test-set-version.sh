#!/usr/bin/env bash
# Test scripts/set-version.sh against a temporary fixture repository.
set -euo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "$(readlink -f "${BASH_SOURCE[0]}")")" && pwd)"
readonly SET_VERSION="$SCRIPT_DIR/set-version.sh"
readonly ROOT="$(mktemp -d)"

cleanup() {
  rm -rf "$ROOT"
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

# Commit and tag mode works in a clean git repository.
make_fixture
git -C "$ROOT" init -q
git -C "$ROOT" config user.name "Version Test"
git -C "$ROOT" config user.email "version-test@example.com"
git -C "$ROOT" config commit.gpgsign false
git -C "$ROOT" config tag.gpgSign false
git -C "$ROOT" add .
git -C "$ROOT" commit -qm "initial"

SET_VERSION_ROOT="$ROOT" "$SET_VERSION" 0.2.0 --tag --no-verify
test "$(git -C "$ROOT" describe --tags --exact-match)" = "v0.2.0" || fail "tag v0.2.0 was not created"
if [[ -n "$(git -C "$ROOT" status --porcelain)" ]]; then
  fail "working tree is dirty after commit and tag"
fi
pass "commit and tag"

# An existing tag is rejected before any files are changed.
if SET_VERSION_ROOT="$ROOT" "$SET_VERSION" 0.2.0 --tag --no-verify; then
  fail "existing tag was accepted"
fi
grep -qx 'version = "0.2.0"' "$ROOT/Cargo.toml" || fail "files changed despite existing tag"
pass "existing tag rejected"

printf 'set-version tests passed\n'
