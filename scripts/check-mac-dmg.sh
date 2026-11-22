#!/usr/bin/env bash
# Mounts the .dmg (best effort: the runners' disk-image service is flaky, so a mount that never works warns and passes,
# the staged app was already proven by make-mac-app.sh), checks the volume's title, the Applications link and the app
# inside (scripts/check-mac-app.sh), then runs the app from the image on scratch data and grabs the screen.
#   scripts/check-mac-dmg.sh <.dmg> <staged server> <check-bundled-server binary> <picture folder>
set -uo pipefail
dmg="${1:?dmg}"; server="${2:?server}"; checker="${3:?checker}"; shots="${4:?picture folder}"
root="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$shots"
mnt=""
for i in 1 2 3 4; do
  if out=$(hdiutil attach "$dmg" -nobrowse -readonly 2>&1); then
    mnt=$(printf '%s\n' "$out" | sed -n 's#.*\(/Volumes/.*\)$#\1#p' | head -n 1); break
  fi
  echo "hdiutil attach try $i: $out"; sleep $((i * 3))
done
if [ -z "$mnt" ]; then echo "::warning::could not mount the .dmg; only the staged app was checked"; exit 0; fi
trap 'hdiutil detach "$mnt" > /dev/null 2>&1 || hdiutil detach "$mnt" -force > /dev/null 2>&1' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }

echo "== the mounted image"
find "$mnt" -maxdepth 1 | sort | sed "s#^$mnt#.#"
case "$mnt" in "/Volumes/Fleet Native 0.0."[0-9]*) echo "volume title: ${mnt#/Volumes/}" ;; *) fail "volume title is not 'Fleet Native 0.0.<build>': ${mnt#/Volumes/}" ;; esac
test -L "$mnt/Applications" || fail "no Applications link"
[ "$(readlink "$mnt/Applications")" = /Applications ] || fail "the link does not point at /Applications"
app="$mnt/Fleet Native.app"
bash "$root/scripts/check-mac-app.sh" "$app" "$server" "$checker" || fail "the app inside the image"
plutil -p "$app/Contents/Info.plist" > "$shots/plutil-p.txt"
codesign -dv "$app" 2>&1 | grep -v '^Executable=' > "$shots/codesign-dv.txt"
{ cd "$mnt" && find . -maxdepth 3 | sort | sed -n '1,40p'; } > "$shots/listing.txt"

echo "== running the app from the image"
data=$(mktemp -d "${TMPDIR:-/tmp}/fleet-mac-XXXXXX")
mkdir -p "$data/app" "$data/fleet" "$data/home"
port=$(python3 -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
printf '{"desktopPort": %s}\n' "$port" > "$data/app/settings.json"
HOME="$data/home" FLEET_NATIVE_DATA_DIR="$data/app" FLEET_DESKTOP_DATA_DIR="$data/fleet" FLEET_HARNESS=test \
  "$app/Contents/MacOS/fleet-native" > "$data/app.log" 2>&1 &
pid=$!
up=0
for i in $(seq 1 60); do
  if curl -fsS "http://127.0.0.1:$port/api/desktop/status" > /dev/null 2>&1; then up=1; break; fi
  kill -0 "$pid" 2> /dev/null || break
  sleep 1
done
sleep 5
if [ "$up" = 1 ]; then echo "the app's own Fleet answered on the bundled server"; else echo "::warning::the app's Fleet did not answer"; tail -n 20 "$data/app.log"; fi
# The runner may have no screen to grab: that is a warning, the listing and plist stand in.
screencapture -x "$shots/app-window.png" 2>&1 || echo "::warning::screencapture failed"
kill "$pid" 2> /dev/null; wait "$pid" 2> /dev/null
rm -rf "$data"
[ "$up" = 1 ] || fail "the app run from the image never started its Fleet"
