#!/usr/bin/env bash
# A real X click at screen pixel ($1, $2) for the web view walk on Linux (scripts/web-walk.sh). With no window manager
# nothing gives the clicked window the keyboard, so this does what one would: focus the top-level window under the pointer.
set -eu
xdotool mousemove "$1" "$2"
sleep 0.15
xdotool click 1
eval "$(xdotool getmouselocation --shell)"
xdotool windowfocus "$WINDOW" 2>/dev/null || true
