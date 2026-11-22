#!/usr/bin/env bash
# Draws every scene to shots/out/<scene>.png with no window (a debug build).
#
#   bash scripts/snap-scenes.sh            # 1280x760 at scale 1
#   SNAP_SCALE=2 bash scripts/snap-scenes.sh
#   SNAP_EVERY=8 bash scripts/snap-scenes.sh   # every 8th scene, a light scene with its dark twin
#   SNAP_SHARDS=4 SNAP_SHARD=2 bash scripts/snap-scenes.sh   # the 2nd of 4 shares (CI draws one share per job)
#
# On Linux with no GPU, install mesa-vulkan-drivers; if a picture still fails, point
# VK_ICD_FILENAMES at /usr/share/vulkan/icd.d/lvp_icd*.json.
set -euo pipefail
cd "$(dirname "$0")/.."

size="${SNAP_SIZE:-1280x760}"
scale="${SNAP_SCALE:-1}"
out="shots/out"

cargo build --quiet
echo "build done after ${SECONDS}s"
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

# SNAP_EVERY=N keeps every Nth scene by its place in the list, so the sample is stable while scenes
# are added at the end. A "-light" scene is kept with the scene it repeats, so a sampled scene is
# drawn in both themes. Used where a start is slow (Windows' software renderer, 3.7 s per scene).
every="${SNAP_EVERY:-1}"
if [ "$every" -gt 1 ]; then
  scenes="$(printf '%s\n' $scenes | awk -v n="$every" '{ b = $0; sub(/-light$/, "", b); if (!(b in id)) id[b] = c++; if (id[b] % n == 0) print }')"
  echo "drawing $(printf '%s\n' $scenes | wc -l | tr -d ' ') scenes (every $every)"
fi

# SNAP_SHARDS=N SNAP_SHARD=K (K from 1) keeps the scenes whose place in the list is K-1 modulo N: the shares are
# disjoint and together they are every scene. CI draws each share in its own job, because one 4-core runner is full
# with 4 pictures at a time.
shards="${SNAP_SHARDS:-1}"
if [ "$shards" -gt 1 ]; then
  shard="${SNAP_SHARD:?SNAP_SHARD (1..SNAP_SHARDS) is needed with SNAP_SHARDS}"
  scenes="$(printf '%s\n' $scenes | awk -v n="$shards" -v k="$shard" '(NR - 1) % n == k - 1')"
  echo "drawing $(printf '%s\n' $scenes | wc -l | tr -d ' ') scenes (share $shard of $shards)"
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
echo "drawn after ${SECONDS}s"
