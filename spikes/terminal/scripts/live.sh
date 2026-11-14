#!/usr/bin/env bash
# Opens the spike's real window on a virtual display (Xvfb), types into its shell with xdotool,
# resizes it, runs vim and a 50 MB flood, and grabs the screen at each step with ffmpeg.
#
#   bash scripts/live.sh <spike binary> <scratch dir> <out dir>
#
# The shell runs with an empty HOME inside <scratch dir> (its `lantern` project, made by
# streams/record.sh), so nothing from the real machine shows. Needs xvfb-run, xdotool, ffmpeg.
set -euo pipefail
bin="$(realpath "$1")"; scratch="$(realpath "$2")"; out="$(mkdir -p "$3" && realpath "$3")"
export SPIKE_BIN="$bin" SPIKE_OUT="$out"
cd "$scratch/lantern"

export SCRATCH="$scratch" XDOTOOL="$(command -v xdotool)"
xvfb-run -a -s "-screen 0 1000x640x24" bash --noprofile --norc -c '
  shot() { ffmpeg -y -loglevel error -f x11grab -video_size 1000x640 -i "$DISPLAY" -frames:v 1 "$SPIKE_OUT/$1.png"; }
  type() { "$XDOTOOL" type --delay 30 "$1"; "$XDOTOOL" key Return; }
  # The app and its shell get a clean environment with the scratch HOME.
  env -i DISPLAY="$DISPLAY" XAUTHORITY="${XAUTHORITY:-}" PATH=/usr/bin:/bin HOME="$SCRATCH" SHELL=/bin/bash LANG=C.UTF-8 \
    TMPDIR="${TMPDIR:-/tmp}" VK_ICD_FILENAMES="${VK_ICD_FILENAMES:-}" LD_LIBRARY_PATH="${LD_LIBRARY_PATH:-}" \
    RUNE_COMMIT_BUDGET_MS="${RUNE_COMMIT_BUDGET_MS:-0}" "$SPIKE_BIN" > "$SPIKE_OUT/app.log" 2>&1 &
  app=$!
  sleep 8
  win=$("$XDOTOOL" search --name "Terminal spike" | head -1)
  "$XDOTOOL" windowmove "$win" 0 0 windowsize "$win" 980 600 windowfocus "$win"
  sleep 2
  type "export PS1=\"lantern\\$ \"; clear"
  type "ls --color=auto -F"
  type "git log --oneline --graph --all --color=always | head -5"
  type "printf \"%s\\n\" \"wide 日本語 emoji 🦀 marks é\"; echo \$COLUMNS x \$LINES"
  sleep 2; shot 1-typed
  "$XDOTOOL" windowsize "$win" 720 420; sleep 2
  type "clear; echo \$(tput cols) x \$(tput lines) after resize"
  sleep 2; shot 2-resized
  "$XDOTOOL" windowsize "$win" 980 600; sleep 2
  type "vim -u NONE -N -c \"syntax on\" -c \"set number\" src/lamp/wick.rs"
  sleep 3; shot 3-vim
  "$XDOTOOL" type ":q"; "$XDOTOOL" key Return; sleep 1
  type "clear; time (yes lantern wick oil glow | head -c 50000000)"
  for i in 1 2 3 4 5 6; do sleep 1; shot "4-flood-$i"; done
  sleep 30; shot 5-after-flood
  # Input still gets through a flood: Ctrl+C stops it.
  type "yes lantern wick oil glow | head -c 2000000000"
  sleep 3; "$XDOTOOL" key ctrl+c; sleep 1; shot 6-interrupted
  kill $app
'
grep -c "commit" "$out/app.log" || true
ls -la "$out"
