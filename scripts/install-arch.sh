#!/usr/bin/env bash
# Install or uninstall Unity Rust Hub on Arch-based distributions.
#
# The default install is per-user under ~/.local and needs no sudo.
# Use --system to install under /usr/local with sudo.
set -euo pipefail

readonly APP_NAME="unity-rust-hub"
SCRIPT_DIR="$(cd -- "$(dirname -- "$(readlink -f "${BASH_SOURCE[0]}")")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
readonly REPO_ROOT
readonly ICON_DIR="$REPO_ROOT/Icon/Icon-png-icons"
readonly DESKTOP_FILE="$REPO_ROOT/packaging/unity-rust-hub.desktop"
readonly LICENSE_FILE="$REPO_ROOT/LICENSE"
readonly ICON_SIZES=(16 20 24 32 40 48 64 256)
readonly CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/unity-rust-hub"

MODE=""
SYSTEM=0
NO_BUILD=0
PURGE=0
PREFIX=""
PREFIX_SET=0
BINARY=""

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

info() {
  printf '==> %s\n' "$*"
}

usage() {
  cat <<'EOF'
Usage: install-arch.sh [install|uninstall] [options]

Build and install Unity Rust Hub on an Arch-based distribution.

Commands:
  install             Build (unless --no-build) and install the app. Default.
  uninstall           Remove the files installed by this script.

Options:
  --system            Install under /usr/local and use sudo.
  --prefix PATH       Install under PATH (default: ~/.local, or /usr/local
                      with --system).
  --no-build          Use the existing target/release/unity-rust-hub.
  --binary PATH       Install a specific binary instead of the release build.
  --purge             On uninstall, also remove ~/.config/unity-rust-hub.
  -h, --help          Show this help.

Examples:
  ./scripts/install-arch.sh
  ./scripts/install-arch.sh uninstall
  ./scripts/install-arch.sh --system
  ./scripts/install-arch.sh --system uninstall
  ./scripts/install-arch.sh --prefix /tmp/unity-hub-test --no-build
EOF
}

while (($#)); do
  case "$1" in
    install|uninstall)
      MODE="$1"
      shift
      ;;
    --system)
      SYSTEM=1
      shift
      ;;
    --prefix)
      [[ $# -ge 2 ]] || die "--prefix needs a value"
      PREFIX="${2%/}"
      PREFIX_SET=1
      shift 2
      ;;
    --prefix=*)
      PREFIX="${1#*=}"
      PREFIX="${PREFIX%/}"
      PREFIX_SET=1
      shift
      ;;
    --no-build)
      NO_BUILD=1
      shift
      ;;
    --binary)
      [[ $# -ge 2 ]] || die "--binary needs a value"
      BINARY="$2"
      shift 2
      ;;
    --binary=*)
      BINARY="${1#*=}"
      shift
      ;;
    --purge)
      PURGE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown argument: $1 (see --help)"
      ;;
  esac
done

MODE="${MODE:-install}"

if [[ "$PREFIX_SET" -eq 0 ]]; then
  if [[ "$SYSTEM" -eq 1 ]]; then
    PREFIX="/usr/local"
  else
    PREFIX="$HOME/.local"
  fi
fi

[[ "$PREFIX" == /* ]] || die "--prefix must be an absolute path"

SUDO=""
if [[ "$SYSTEM" -eq 1 ]]; then
  SUDO="sudo"
fi

readonly BIN_DIR="$PREFIX/bin"
readonly SHARE_DIR="$PREFIX/share"
readonly APP_DIR="$SHARE_DIR/applications"
readonly ICON_THEME_DIR="$SHARE_DIR/icons/hicolor"
readonly BIN_DEST="$BIN_DIR/$APP_NAME"
readonly DESKTOP_DEST="$APP_DIR/$APP_NAME.desktop"
readonly LICENSE_DEST="$SHARE_DIR/licenses/$APP_NAME/LICENSE"
readonly SOURCE_BIN="${BINARY:-$REPO_ROOT/target/release/$APP_NAME}"

as_root() {
  if [[ -n "$SUDO" ]]; then
    "$SUDO" "$@"
  else
    "$@"
  fi
}

refresh_caches() {
  if command -v gtk-update-icon-cache >/dev/null 2>&1 && [[ -d "$ICON_THEME_DIR" ]]; then
    as_root gtk-update-icon-cache -q -t -f "$ICON_THEME_DIR" 2>/dev/null || true
  fi
  if command -v update-desktop-database >/dev/null 2>&1 && [[ -d "$APP_DIR" ]]; then
    as_root update-desktop-database -q "$APP_DIR" 2>/dev/null || true
  fi
}

cleanup_empty_caches() {
  if [[ -f "$ICON_THEME_DIR/icon-theme.cache" ]] &&
    [[ -z "$(find "$ICON_THEME_DIR" -type f ! -name 'icon-theme.cache' -print -quit)" ]]; then
    as_root rm -f "$ICON_THEME_DIR/icon-theme.cache"
  fi
  if [[ -f "$APP_DIR/mimeinfo.cache" ]] &&
    [[ -z "$(find "$APP_DIR" -maxdepth 1 -type f -name '*.desktop' -print -quit)" ]]; then
    as_root rm -f "$APP_DIR/mimeinfo.cache"
  fi
}

preflight_privileges() {
  if [[ "$SYSTEM" -eq 0 && "$(id -u)" -eq 0 ]]; then
    die "refusing to install per-user files as root; pass --system or run as a normal user"
  fi

  if [[ "$SYSTEM" -eq 1 ]]; then
    command -v sudo >/dev/null 2>&1 || die "--system requires sudo"
    sudo -v || die "sudo authentication failed"
  fi
}

check_assets() {
  [[ -f "$DESKTOP_FILE" ]] || die "missing desktop entry: $DESKTOP_FILE"
  [[ -f "$LICENSE_FILE" ]] || die "missing license: $LICENSE_FILE"

  local size
  for size in "${ICON_SIZES[@]}"; do
    [[ -f "$ICON_DIR/Icon-${size}x${size}.png" ]] ||
      die "missing icon: $ICON_DIR/Icon-${size}x${size}.png"
  done
}

install_app() {
  preflight_privileges
  check_assets

  if [[ "$NO_BUILD" -eq 0 ]]; then
    info "building $APP_NAME (release)"
    (cd "$REPO_ROOT" && cargo build --release --locked)
  fi

  [[ -f "$SOURCE_BIN" && -x "$SOURCE_BIN" ]] ||
    die "binary not found or not executable: $SOURCE_BIN (build first or pass --binary)"

  info "installing to $PREFIX"
  as_root install -d "$BIN_DIR" "$APP_DIR" "$ICON_THEME_DIR" "$(dirname "$LICENSE_DEST")"
  as_root install -m755 "$SOURCE_BIN" "$BIN_DEST"
  as_root install -m644 "$DESKTOP_FILE" "$DESKTOP_DEST"
  as_root install -m644 "$LICENSE_FILE" "$LICENSE_DEST"

  local size dest
  for size in "${ICON_SIZES[@]}"; do
    dest="$ICON_THEME_DIR/${size}x${size}/apps/$APP_NAME.png"
    as_root install -d "$(dirname "$dest")"
    as_root install -m644 "$ICON_DIR/Icon-${size}x${size}.png" "$dest"
  done

  refresh_caches

  info "installed $APP_NAME"
  if [[ "$PREFIX" == "$HOME/.local" ]]; then
    case ":$PATH:" in
      *":$HOME/.local/bin:"*) ;;
      *)
        printf 'note: %s is not on PATH; add it to your shell profile to run %s from a terminal.\n' \
          "$HOME/.local/bin" "$APP_NAME"
        ;;
    esac
  fi
  printf 'Launch it from your application menu or run: %s\n' "$BIN_DEST"
}

uninstall_app() {
  preflight_privileges

  info "removing $APP_NAME from $PREFIX"
  as_root rm -f "$BIN_DEST" "$DESKTOP_DEST" "$LICENSE_DEST"

  local size
  for size in "${ICON_SIZES[@]}"; do
    as_root rm -f "$ICON_THEME_DIR/${size}x${size}/apps/$APP_NAME.png"
    as_root rmdir --ignore-fail-on-non-empty "$ICON_THEME_DIR/${size}x${size}/apps" 2>/dev/null || true
  done
  as_root rmdir --ignore-fail-on-non-empty "$APP_DIR" 2>/dev/null || true
  as_root rmdir --ignore-fail-on-non-empty "$(dirname "$LICENSE_DEST")" 2>/dev/null || true

  refresh_caches
  cleanup_empty_caches

  if [[ "$PURGE" -eq 1 ]]; then
    rm -rf "$CONFIG_DIR"
    info "removed $CONFIG_DIR"
  fi

  info "uninstalled $APP_NAME"
  printf 'If you installed the pacman package instead, remove it with: sudo pacman -Rns %s\n' "$APP_NAME"
}

case "$MODE" in
  install) install_app ;;
  uninstall) uninstall_app ;;
  *) die "unknown command: $MODE" ;;
esac
