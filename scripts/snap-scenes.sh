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

# One process per scene (Rune's headless run is one picture per start), several at once. Software
# renderers start slowly, so this is mostly waiting; SNAP_JOBS=1 draws them one by one.
jobs="${SNAP_JOBS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 2)}"

snap_one() {
  scene="$1"
  "$bin" --snap "$out/$scene.png" --scene "$scene" --size "$size" --scale "$scale" --at 0.3 < /dev/null
  if [ ! -s "$out/$scene.png" ]; then
    echo "scene $scene drew nothing" >&2
    return 1
  fi
  echo "$out/$scene.png"
}
export -f snap_one
export bin out size scale

# xargs exits non-zero if any scene failed.
printf '%s\n' $scenes | xargs -P "$jobs" -I{} bash -c 'snap_one {}'
