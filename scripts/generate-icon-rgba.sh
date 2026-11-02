#!/usr/bin/env bash
# Regenerates the raw RGBA window icon used by `Icon::from_rgba`.
# Keeping the runtime on raw pixels lets us drop iced's `image` feature and its
# full set of image codecs from the release binary.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
input="$root/crates/unity-hub/assets/icon.png"
output="$root/crates/unity-hub/assets/icon-128.rgba"

if command -v magick >/dev/null 2>&1; then
  magick "$input" -resize 128x128! -depth 8 "rgba:$output"
elif command -v convert >/dev/null 2>&1; then
  convert "$input" -resize 128x128! -depth 8 "rgba:$output"
else
  echo "ImageMagick (magick/convert) is required to regenerate $output" >&2
  exit 1
fi

echo "Wrote $output"
