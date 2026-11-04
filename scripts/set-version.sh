#!/usr/bin/env bash
# Update every live version reference in the repository, and optionally commit,
# tag, and publish a release.
#
# Safe default: only edit files. Use --commit, --tag, or --release for git work.
set -euo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "$(readlink -f "${BASH_SOURCE[0]}")")" && pwd)"
readonly DEFAULT_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
ROOT="${SET_VERSION_ROOT:-$DEFAULT_ROOT}"

VERSION=""
COMMIT=0
TAG=0
RELEASE=0
YES=0
VERIFY=1
ALLOW_DIRTY=0

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

info() {
  printf '==> %s\n' "$*"
}

usage() {
  cat <<'EOF'
Usage: scripts/set-version.sh <version> [options]

Update the workspace, packager, Arch package, lockfile, and README version
references. Accepts an optional leading "v".

Options:
  --commit        Commit the version changes.
  --tag           Commit and create an annotated v<version> tag.
  --release       Commit, tag, and push the branch and tag. The pushed tag
                  triggers the GitHub release/package workflow.
  --yes, -y       Skip the --release confirmation prompt.
  --no-verify     Skip Cargo.lock refresh and the test suite. Cargo.lock will
                  be stale unless you update it yourself.
  --allow-dirty   Allow update-only mode in a dirty worktree. Cannot be used
                  with --commit, --tag, or --release.
  -h, --help      Show this help.

Examples:
  ./scripts/set-version.sh 0.3.0
  ./scripts/set-version.sh 0.3.0 --commit
  ./scripts/set-version.sh 0.3.0 --tag
  ./scripts/set-version.sh 0.3.0 --release --yes
EOF
}

while (($#)); do
  case "$1" in
    --commit)
      COMMIT=1
      shift
      ;;
    --tag)
      TAG=1
      COMMIT=1
      shift
      ;;
    --release)
      RELEASE=1
      TAG=1
      COMMIT=1
      shift
      ;;
    --yes|-y)
      YES=1
      shift
      ;;
    --no-verify)
      VERIFY=0
      shift
      ;;
    --allow-dirty)
      ALLOW_DIRTY=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    -*)
      die "unknown option: $1 (see --help)"
      ;;
    *)
      [[ -z "$VERSION" ]] || die "version specified more than once"
      VERSION="$1"
      shift
      ;;
  esac
done

[[ -n "$VERSION" ]] || die "missing version (see --help)"
VERSION="${VERSION#v}"

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]; then
  die "invalid version '$VERSION' (expected MAJOR.MINOR.PATCH, optionally with a prerelease suffix)"
fi

if ((ALLOW_DIRTY)) && ((COMMIT || TAG || RELEASE)); then
  die "--allow-dirty cannot be combined with git operations"
fi

[[ -d "$ROOT" ]] || die "repository root not found: $ROOT"
cd "$ROOT"

for file in Cargo.toml Packager.toml packaging/arch/PKGBUILD README.md; do
  [[ -f "$file" ]] || die "missing $file (run from the repository root or set SET_VERSION_ROOT)"
done

current_cargo="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
current_packager="$(sed -n 's/^version = "\(.*\)"/\1/p' Packager.toml | head -n1)"
current_pkgver="$(sed -n 's/^pkgver=//p' packaging/arch/PKGBUILD | head -n1)"

if ((COMMIT || TAG || RELEASE)); then
  git rev-parse --is-inside-work-tree >/dev/null 2>&1 ||
    die "git operations require a git worktree"
  if [[ -n "$(git status --porcelain)" ]]; then
    die "working tree is dirty; commit or stash changes before using --commit, --tag, or --release"
  fi
  if ((TAG)) && git rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null; then
    die "tag v$VERSION already exists"
  fi
fi

info "updating version to $VERSION"

sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Packager.toml
sed -i "s/^pkgver=.*/pkgver=$VERSION/" packaging/arch/PKGBUILD
sed -i "s/^pkgrel=.*/pkgrel=1/" packaging/arch/PKGBUILD

escape_sed() {
  printf '%s' "$1" | sed 's/[.[\*^$]/\\&/g'
}

for old_version in "$current_cargo" "$current_packager" "$current_pkgver"; do
  [[ -n "$old_version" ]] || continue
  old_escaped="$(escape_sed "$old_version")"
  sed -i \
    "s/UnityRustHub-${old_escaped}-linux-x86_64\\.AppImage/UnityRustHub-${VERSION}-linux-x86_64.AppImage/g" \
    README.md
done

grep -qx "version = \"$VERSION\"" Cargo.toml ||
  die "Cargo.toml does not contain version $VERSION"
grep -qx "version = \"$VERSION\"" Packager.toml ||
  die "Packager.toml does not contain version $VERSION"
grep -qx "pkgver=$VERSION" packaging/arch/PKGBUILD ||
  die "packaging/arch/PKGBUILD does not contain pkgver=$VERSION"

if ((VERIFY)); then
  info "refreshing Cargo.lock"
  cargo check --workspace --quiet

  grep -A2 '^name = "unity-hub"$' Cargo.lock |
    grep -q "^version = \"$VERSION\"$" ||
    die "Cargo.lock does not contain unity-hub $VERSION"

  info "running the workspace test suite"
  cargo test --workspace --locked --quiet
else
  info "skipping Cargo.lock refresh and tests (--no-verify)"
fi

if ((COMMIT)); then
  git add Cargo.toml Cargo.lock Packager.toml packaging/arch/PKGBUILD README.md

  if git diff --cached --quiet; then
    info "no file changes to commit"
  else
    git commit -m "chore: release $VERSION"
  fi
fi

if ((TAG)); then
  if git rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null; then
    die "tag v$VERSION already exists"
  fi
  git tag -a "v$VERSION" -m "Unity Rust Hub $VERSION"
  info "created tag v$VERSION"
fi

if ((RELEASE)); then
  if ((YES == 0)); then
    printf 'Push branch and tag v%s to origin? [y/N] ' "$VERSION"
    read -r answer
    case "$answer" in
      y|Y|yes|YES|Yes) ;;
      *) die "aborted" ;;
    esac
  fi

  info "pushing branch and tag v$VERSION"
  git push origin HEAD
  git push origin "v$VERSION"
fi

info "version $VERSION is ready"
if ((!COMMIT)); then
  printf 'Next: review the diff, then rerun with --commit, --tag, or --release.\n'
fi
