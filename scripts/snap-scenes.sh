#!/usr/bin/env bash
# Draws every scene to shots/out/<scene>.png with no window (a debug build).
#
#   bash scripts/snap-scenes.sh            # 1280x760 at scale 1
#   SNAP_SCALE=2 bash scripts/snap-scenes.sh
#
# On Linux with no GPU, install mesa-vulkan-drivers; if a picture still fails, point
# VK_ICD_FILENAMES at /usr/share/vulkan/icd.d/lvp_icd*.json.
set -euo pipefail
cd "$(dirname "$0")/.."

size="${SNAP_SIZE:-1280x760}"
scale="${SNAP_SCALE:-1}"
out="shots/out"

cargo build --quiet
bin="${CARGO_TARGET_DIR:-target}/debug/fleet-native"
if [ -f "$bin.exe" ]; then bin="$bin.exe"; fi

mkdir -p "$out"
rm -f "$out"/*.png

# Windows prints CRLF line ends.
scenes="$("$bin" --list-scenes < /dev/null | tr -d '\r')"
if [ -z "$scenes" ]; then
  echo "no scenes listed by $bin" >&2
  exit 1
fi

for scene in $scenes; do
  "$bin" --snap "$out/$scene.png" --scene "$scene" --size "$size" --scale "$scale" --at 0.3 < /dev/null
  if [ ! -s "$out/$scene.png" ]; then
    echo "scene $scene drew nothing" >&2
    exit 1
  fi
  echo "$out/$scene.png"
done
