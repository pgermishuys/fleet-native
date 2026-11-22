#!/usr/bin/env bash
# Smoke test for a built or packaged app (row 24.152; weave-fleet's desktop/scripts/smoke.mjs, ported): start the
# app on scratch data, wait for the Fleet it starts, check the API, list a repository (which runs git from Fleet),
# then kill the app the hard way (SIGKILL) and check its Fleet stops too and gives up the database lock, by starting
# the app again on the same data.
#
#   bash scripts/smoke-desktop.sh <app executable> [app arguments...]
#
# FLEET_NATIVE_SERVER_BIN (and FLEET_NATIVE_SERVER_CONTENT_ROOT) name the server when the app has none bundled beside
# it. Linux needs a display: with none, the app is run under xvfb-run. macOS and Linux only; Windows has its twin,
# scripts/smoke-desktop.ps1. SMOKE_SHOT=<file.png> grabs the screen once the repository is listed (Linux: ffmpeg's
# x11grab; macOS: screencapture); a grab that fails only warns.
#
# The scratch folder holds the app's own data, a fake home and the Fleet's data; nothing outside it is touched, and
# the app is told not to start a second instance of itself (a set FLEET_NATIVE_DATA_DIR does that).
set -uo pipefail
[ "$#" -ge 1 ] || { echo "usage: bash scripts/smoke-desktop.sh <app executable> [app arguments...]"; exit 2; }
APP=$1
shift
if [ "$(uname -s)" = Linux ] && [ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
    command -v xvfb-run > /dev/null || { echo "no display and no xvfb-run"; exit 2; }
    exec xvfb-run -a -s "-screen 0 1400x920x24" bash "$0" "$APP" "$@"
fi

ROOT=$(mktemp -d "${TMPDIR:-/tmp}/fleet-smoke-XXXXXX")
DATA=$ROOT/data
REPOS=$ROOT/repos
mkdir -p "$DATA" "$REPOS/smoke-repo" "$ROOT/home" "$ROOT/app-data" "$ROOT/run"
chmod 700 "$ROOT/run"
(
    cd "$REPOS/smoke-repo" && git init -q && echo smoke > README.md && git add . &&
        git -c user.name=smoke -c user.email=smoke@example.com commit -q -m smoke
) || { echo "git failed"; exit 2; }

# Never Fleet's default 6262, where somebody's own Fleet may live: the app tries the port its settings remember first.
PORT=${SMOKE_PORT:-$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')}
printf '{"desktopPort": %s}\n' "$PORT" > "$ROOT/app-data/settings.json"
INSTANCE=$DATA/fleet.instance.json
PIDS=()
alive() { kill -0 "$1" 2> /dev/null; }
# Reads a string or number field from the instance file without a JSON tool.
field() { sed -n "s/.*\"$1\" *: *\"\{0,1\}\([^\",}]*\)\"\{0,1\}.*/\1/p" "$INSTANCE" 2> /dev/null | head -n 1; }
until_true() { # what, seconds, command...
    local what=$1 secs=$2
    shift 2
    local end=$((SECONDS + secs))
    while [ "$SECONDS" -lt "$end" ]; do
        "$@" && return 0
        sleep 0.5
    done
    echo "Timed out waiting for $what"
    return 1
}
launch() {
    # HOME is the scratch home so the app's own folders, the login shell's rc files and the server's caches are
    # the scratch ones. FLEET_DESKTOP_DATA_DIR is where Fleet keeps its database and instance file.
    HOME=$ROOT/home XDG_DATA_HOME=$ROOT/app-data XDG_RUNTIME_DIR=$ROOT/run FLEET_NATIVE_DATA_DIR=$ROOT/app-data \
        FLEET_DESKTOP_DATA_DIR=$DATA FLEET_HARNESS=test FLEET_WORKSPACE_ROOTS=$REPOS \
        "$APP" "$@" > "$ROOT/app-${#PIDS[@]}.log" 2>&1 &
    LAUNCHED=$!
    PIDS+=("$LAUNCHED")
    disown "$LAUNCHED" # so the shell doesn't print "Killed" when the test kills it
}
fleet_up() { # the previous pid to differ from
    alive "$APP_PID" || { FAILED=1; echo "The app exited before its Fleet started"; exit 1; }
    [ "$(field desktop)" = true ] && [ -n "$(field pid)" ] && [ "$(field pid)" != "$1" ]
}
FAILED=0
cleanup() {
    for p in "${PIDS[@]}"; do alive "$p" && kill -KILL "$p" 2> /dev/null; done
    local leftover
    leftover=$(field pid)
    [ -n "$leftover" ] && alive "$leftover" && kill -KILL "$leftover" 2> /dev/null
    if [ "$FAILED" = 1 ]; then
        for log in $(find "$ROOT" -name 'fleet-server.log' -o -name 'app-*.log' 2> /dev/null); do
            echo; echo "── $log"; tail -c 8000 "$log"
        done
    fi
    rm -rf "$ROOT"
}
trap cleanup EXIT
fail() { FAILED=1; echo "Smoke test failed: $*"; exit 1; }

launch "$@"
APP_PID=$LAUNCHED
until_true "the app's Fleet to write its instance file" 120 fleet_up "" || fail "no Fleet"
URL=$(field url)
FLEET_PID=$(field pid)
echo "Fleet $(field version) is up at $URL (pid $FLEET_PID)"

curl -fsS "$URL/readyz" > /dev/null || fail "/readyz didn't answer"
curl -fsS "$URL/" | grep -q 'id="app"' || fail "Fleet didn't serve its UI"
STATUS=$(curl -fsS "$URL/api/desktop/status") || fail "/api/desktop/status didn't answer"
echo "UI served, desktop status $STATUS"

# Listing repositories runs git for each one. On Windows a git that inherited Fleet's standard input (the app's
# pipe, which Fleet is reading) hung forever, so the list never came (weave-fleet #301).
START=$SECONDS
curl -fsS --max-time 30 "$URL/api/repositories" | grep -q '"smoke-repo"' || fail "/api/repositories didn't list the smoke repository"
echo "Listed the smoke repository in $((SECONDS - START)) s."
if [ -n "${SMOKE_SHOT:-}" ]; then
    sleep 4 # the sessions page asks its Fleet for the list; let it draw
    mkdir -p "$(dirname "$SMOKE_SHOT")"
    if [ "$(uname -s)" = Darwin ]; then
        screencapture -x "$SMOKE_SHOT" || echo "::warning::screencapture failed"
    else
        ffmpeg -loglevel error -y -f x11grab -i "$DISPLAY" -frames:v 1 "$SMOKE_SHOT" || echo "::warning::the screen grab failed"
    fi
    [ -f "$SMOKE_SHOT" ] && echo "Saved the screen to $SMOKE_SHOT."
fi

# The app dying any way at all closes the pipe the Fleet reads, so it stops by itself.
kill -KILL "$APP_PID"
until_true "the app's Fleet to stop after the app was killed" 30 bash -c "! kill -0 $FLEET_PID 2> /dev/null" || fail "the Fleet outlived the app"
echo "Killing the app stopped its Fleet."

# Fleet deletes its instance file as it stops, but a file left behind is stale and the next start replaces it. What
# matters is the lock: a new app has to start its own Fleet on the same data.
launch "$@"
APP_PID=$LAUNCHED
until_true "a new app to start its own Fleet on the same data" 120 fleet_up "$FLEET_PID" || fail "no second Fleet"
URL=$(field url)
SECOND=$(field pid)
curl -fsS "$URL/readyz" > /dev/null || fail "/readyz didn't answer after the restart"
echo "The app started again on the same data (Fleet pid $SECOND), so the lock was released."
kill -KILL "$APP_PID"
until_true "the second app's Fleet to stop after it was killed" 30 bash -c "! kill -0 $SECOND 2> /dev/null" || fail "the second Fleet outlived its app"
echo "Smoke test passed."
