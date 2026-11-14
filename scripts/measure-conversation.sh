#!/usr/bin/env bash
# Measures the long conversation from a release build and writes the result to a file.
#
#   bash scripts/measure-conversation.sh [path/to/fleet-native] [results-file]
#
# Part 1 (any machine): `--bench-conversation` times every frame's commit and layout in a headless
# runtime, with no window and no GPU (src/bench.rs). Run RUNS times (default 3), with peak memory.
# Part 2 (needs a GPU to mean anything; SKIP_GPU=1 leaves it out): Rune's headless renderer, the
# window's GPU path drawn offscreen, driven by scripts/bench/*.json on a fixed 60 fps clock. Rune
# prints the CPU time of each frame including encode and submit, not the wait for the display.
# Set GPU_NOTE to the adapter's name. Windows: scripts/measure-conversation.ps1 does the same.
set -uo pipefail
cd "$(dirname "$0")/.."
bin="${1:-${CARGO_TARGET_DIR:-target}/release/fleet-native}"
[ -f "$bin" ] || bin="$bin.exe"
out="${2:-conversation-results.txt}"
runs="${RUNS:-3}"
[ -f "$bin" ] || { echo "no such program: $bin (cargo build --release first)" >&2; exit 1; }
if /usr/bin/time -v true > /dev/null 2>&1; then timer=(/usr/bin/time -v); else timer=(/usr/bin/time -l); fi
memory='Maximum resident|maximum resident'

{
  echo "# Long conversation, native app"
  echo "date: $(date -u +%Y-%m-%dT%H:%MZ)"
  echo "system: $(uname -sm); cpu: $(grep -m1 'model name' /proc/cpuinfo 2> /dev/null | cut -d: -f2 | xargs || sysctl -n machdep.cpu.brand_string 2> /dev/null)"
  echo "gpu: ${GPU_NOTE:-not stated}"
  echo "build: $bin"
  echo
  echo "## Part 1: commit and layout per frame (no GPU)"
} > "$out"
for run in $(seq "$runs"); do
  { echo; echo "run $run"; "${timer[@]}" "$bin" --bench-conversation < /dev/null 2>&1 | grep -E "^\||$memory"; } >> "$out"
done

if [ -z "${SKIP_GPU:-}" ]; then
  echo -e "\n## Part 2: the GPU path, drawn offscreen" >> "$out"
  for case in "open|dark|open" "stream|long-live|stream" "scroll-end|long-live|scroll-end" "scroll-top|long-conversation|scroll-top"; do
    IFS='|' read -r name scene script <<< "$case"
    for run in $(seq "$runs"); do
      { echo; echo "$name, run $run"; "${timer[@]}" "$bin" --scene "$scene" --script "scripts/bench/$script.json" --trace /dev/null < /dev/null 2>&1 | grep -E "^rune: .*(frames|first frame)|$memory"; } >> "$out"
    done
  done
fi
echo "wrote $out"
