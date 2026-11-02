#!/usr/bin/env bash
# Make desktop/build/icon.icns (and icon.png) from the brand mark,
# docs/assets/vulcanbench-logo.png, with the macOS tools sips and iconutil.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
src="$here/../docs/assets/vulcanbench-logo.png"
build="$here/build"
set_="$build/icon.iconset"

[[ -f "$src" ]] || { echo "logo not found at $src" >&2; exit 1; }
command -v sips >/dev/null && command -v iconutil >/dev/null || { echo "needs macOS sips and iconutil" >&2; exit 2; }

rm -rf "$set_"
mkdir -p "$set_"
for size in 16 32 128 256 512; do
  double=$((size * 2))
  sips -z "$size" "$size" "$src" --out "$set_/icon_${size}x${size}.png" >/dev/null
  sips -z "$double" "$double" "$src" --out "$set_/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$set_" -o "$build/icon.icns"
sips -z 512 512 "$src" --out "$build/icon.png" >/dev/null
rm -rf "$set_"
echo "icon written to $build/icon.icns"
