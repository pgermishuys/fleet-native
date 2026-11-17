#!/usr/bin/env bash
# The web view walk (component 14, piece A): runs the app on an invented page canvas whose page is served from
# tests/fixtures/web by a local server, with real OS input, and grabs the whole screen after each step (`src/web/demo.rs`).
#
#   bash scripts/web-walk.sh <app binary> [out folder, default shots/out-web]
#
# Linux runs it under Xvfb (needs xvfb-run, xdotool, ffmpeg); Windows uses PowerShell for input and pictures; macOS takes
# pictures with screencapture and clicks with cliclick when it is installed (else the keyboard steps are skipped).
set -uo pipefail
BIN=$1
OUT=${2:-shots/out-web}
PORT=${WEB_WALK_PORT:-8765}
mkdir -p "$OUT"
# Windows runners have `python`; their `python3` may be the Store's stand-in, which only opens the Store.
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) PY=python ;; *) PY=$(command -v python3 || command -v python) ;; esac
"$PY" -m http.server "$PORT" --bind 127.0.0.1 --directory tests/fixtures/web >"$OUT/http.log" 2>&1 &
SERVER=$!
trap 'kill $SERVER 2>/dev/null || true' EXIT
for _ in $(seq 50); do
  curl -fs "http://127.0.0.1:$PORT/pages/walk/index.html" >/dev/null && break
  sleep 0.2
done
export FLEET_NATIVE_WEB_DEMO="http://127.0.0.1:$PORT"
case "$(uname -s)" in
  Linux)
    export WEB_DEMO_SHOT="ffmpeg -y -loglevel error -f x11grab -video_size 1280x800 -i \$DISPLAY -frames:v 1 $OUT/{name}.png"
    export WEB_DEMO_CLICK="bash scripts/web-walk/x-click.sh {x} {y}"
    export WEB_DEMO_TYPE="xdotool type --delay 80 {text}"
    timeout 300 xvfb-run -a -s "-screen 0 1280x800x24" "$BIN"
    ;;
  MINGW* | MSYS* | CYGWIN*)
    WIN_OUT=$(cygpath -w "$OUT")
    export WEB_DEMO_SHOT="powershell -NoProfile -ExecutionPolicy Bypass -File scripts\\web-walk\\shot.ps1 $WIN_OUT\\{name}.png"
    export WEB_DEMO_CLICK="powershell -NoProfile -ExecutionPolicy Bypass -File scripts\\web-walk\\click.ps1 {x} {y}"
    export WEB_DEMO_TYPE="powershell -NoProfile -ExecutionPolicy Bypass -File scripts\\web-walk\\type.ps1 {text}"
    timeout 300 "$BIN"
    ;;
  Darwin)
    export WEB_DEMO_SHOT="screencapture -x $OUT/{name}.png"
    if command -v cliclick >/dev/null; then
      export WEB_DEMO_CLICK="cliclick c:{x},{y}"
      export WEB_DEMO_TYPE="cliclick -w 80 t:{text}"
    fi
    "$BIN" &
    APP=$!
    ( sleep 300 && kill $APP 2>/dev/null ) &
    wait $APP
    ;;
esac
STATUS=$?
echo "the app exited with $STATUS"
ls "$OUT"
exit $STATUS
