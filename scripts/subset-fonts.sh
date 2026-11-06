#!/usr/bin/env bash
# Regenerates the bundled fonts in crates/unity-hub-ui/assets/fonts/.
#
# Downloads Inter, JetBrains Mono, and the Lucide icon font, then subsets them
# with fontTools so the binary only carries Latin text glyphs and the icons the
# UI actually uses. Add a Lucide icon name to ICONS (and a constant in
# crates/unity-hub-ui/src/icons.rs) when the UI needs a new icon.
#
# Requires curl, unzip, tar, and python3 (fontTools is installed into a
# temporary virtualenv).
set -euo pipefail

INTER_VERSION="4.1"
JETBRAINS_MONO_VERSION="2.304"
LUCIDE_VERSION="0.460.0"

ICONS=(
  layout-dashboard box folders newspaper settings refresh-cw check circle-check
  x circle-x triangle-alert circle-alert info terminal download play
  external-link folder-open circle-arrow-up user key-round search git-branch
  cloud star puzzle cpu file-text chevron-down chevron-right loader package
  square-terminal monitor calendar hard-drive shield-alert copy log-in ban
  arrow-right sparkles wrench trash-2 circle-dot plus minus
)

# Basic Latin, Latin-1, Latin Extended-A, general punctuation, arrows, and a
# few symbols the UI prints (→, •, …, ·, −, €, ✓).
TEXT_UNICODES="U+0020-007E,U+00A0-017F,U+2010-2027,U+2030-203A,U+2190-2193,U+2212,U+20AC,U+2713"
# The monospace font also renders CLI logs, which may draw boxes.
MONO_UNICODES="$TEXT_UNICODES,U+2500-257F,U+2580-259F"

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="$root/crates/unity-hub-ui/assets/fonts"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

mkdir -p "$out"
cd "$work"

curl -sSLf -o inter.zip \
  "https://github.com/rsms/inter/releases/download/v$INTER_VERSION/Inter-$INTER_VERSION.zip"
curl -sSLf -o mono.zip \
  "https://github.com/JetBrains/JetBrainsMono/releases/download/v$JETBRAINS_MONO_VERSION/JetBrainsMono-$JETBRAINS_MONO_VERSION.zip"
curl -sSLf -o lucide.tgz \
  "https://registry.npmjs.org/lucide-static/-/lucide-static-$LUCIDE_VERSION.tgz"

unzip -oq inter.zip 'extras/ttf/Inter-Regular.ttf' 'extras/ttf/Inter-Medium.ttf' \
  'extras/ttf/Inter-SemiBold.ttf' LICENSE.txt
unzip -oq mono.zip 'fonts/ttf/JetBrainsMono-Regular.ttf' OFL.txt
tar xzf lucide.tgz package/font/lucide.ttf package/font/info.json package/LICENSE

python3 -m venv venv
venv/bin/pip install --quiet fonttools

subset() {
  venv/bin/pyftsubset "$1" --output-file="$2" --unicodes="$3" \
    --layout-features='kern,liga,calt,tnum,case' --name-IDs='*' --no-hinting --desubroutinize
}

for weight in Regular Medium SemiBold; do
  subset "extras/ttf/Inter-$weight.ttf" "$out/Inter-$weight.ttf" "$TEXT_UNICODES"
done
subset fonts/ttf/JetBrainsMono-Regular.ttf "$out/JetBrainsMono-Regular.ttf" "$MONO_UNICODES"

icon_unicodes="$(venv/bin/python - "${ICONS[@]}" <<'PY'
import json, sys
info = json.load(open("package/font/info.json"))
missing = [name for name in sys.argv[1:] if name not in info]
if missing:
    sys.exit("unknown Lucide icons: " + ", ".join(missing))
print(",".join("U+" + info[name]["encodedCode"].lstrip("\\") for name in sys.argv[1:]))
PY
)"
venv/bin/pyftsubset package/font/lucide.ttf --output-file="$out/lucide.ttf" \
  --unicodes="$icon_unicodes" --no-hinting

cp LICENSE.txt "$out/Inter-LICENSE.txt"
cp OFL.txt "$out/JetBrainsMono-OFL.txt"
cp package/LICENSE "$out/Lucide-LICENSE.txt"

ls -l "$out"
