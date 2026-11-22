#!/usr/bin/env bash
# Proves the updater's replace step on this system with no window and no GitHub: copies a debug binary to a temp folder with a
# stand-in for Fleet's server beside it (`fleet/`), and runs it with FLEET_NATIVE_TEST_BUILD=1 (a debug build reads it), so it
# believes it is build 1; serves a fake "latest" release from loopback whose build 2 asset holds a slightly different program
# and a newer server, runs `fleet-native --update-now --no-restart` on the copy, and checks the program became the new one (by
# SHA-256), the server folder is the new one (an old file gone, a user's file kept) and nothing was left behind. On Linux it then
# does the same for an AppImage (APPIMAGE names the file, which is replaced as one). Linux and Windows; macOS is proved by a
# test with a real .dmg that holds a server too. FLEET_NATIVE_UPDATES=off is checked last: nothing is replaced.
#
#   cargo build && bash scripts/update-check.sh target/debug/fleet-native
set -euo pipefail
src="${1:?path of a debug build}"
tmp="$(mktemp -d)"
trap 'kill "${server:-0}" 2>/dev/null || true; rm -rf "$tmp" 2>/dev/null || true' EXIT
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) src="${src%.exe}.exe"; key=windows-x64; name=fleet-native-windows-x64.zip; exe=fleet-native.exe ;;
  Linux) key=linux-x64; name=fleet-native-linux-x64.tar.gz; exe=fleet-native ;;
  *) echo "not for this system" >&2; exit 2 ;;
esac
py=python3; command -v python3 >/dev/null || py=python
mkdir -p "$tmp/app/fleet/lib" "$tmp/release"
cp "$src" "$tmp/app/$exe"
echo "server 1" > "$tmp/app/fleet/version"; echo x > "$tmp/app/fleet/lib/only-in-the-old-server"; echo mine > "$tmp/app/mine.txt"
"$tmp/app/$exe" --version
# The "new build": the same program with a few bytes after the end, which neither ELF nor PE minds.
"$py" - "$src" "$tmp/release" "$name" "$exe" "$key" <<'P'
import sys, hashlib, json, tarfile, zipfile, os
src, out, name, exe, key = sys.argv[1:6]
data = open(src, 'rb').read() + b'\0update-check build 2'
open(os.path.join(out, 'new-program'), 'wb').write(data)
open(os.path.join(out, 'new-server'), 'w').write('server 2\n')
def pack(path, members):
    # members: (name in the package, file). Named as CI names them: the exe and fleet/ side by side.
    if path.endswith('.zip'):
        with zipfile.ZipFile(path, 'w', zipfile.ZIP_DEFLATED) as z:
            for n, f in members: z.write(f, n)
    else:
        with tarfile.open(path, 'w:gz') as t:
            for n, f in members: t.add(f, n)
entry = {}
for k, n, members in ((key, name, [(exe, 'new-program'), ('fleet/version', 'new-server')]),
                      ('linux-x64-appimage', 'fleet-native-linux-x64.AppImage', None)):
    path = os.path.join(out, n)
    if members is None:
        open(path, 'wb').write(data)
    else:
        pack(path, [(a, os.path.join(out, b)) for a, b in members])
    blob = open(path, 'rb').read()
    entry[k] = {"name": n, "size": len(blob), "sha256": hashlib.sha256(blob).hexdigest()}
json.dump({"commit": "0" * 40, "built_at": "2026-11-15T14:19:00Z", "build": 2, "assets": entry}, open(os.path.join(out, 'latest.json'), 'w'))
open(os.path.join(out, 'want.sha256'), 'w').write(hashlib.sha256(data).hexdigest())
P
port=$((20000 + RANDOM % 20000))
(cd "$tmp/release" && exec "$py" -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1) &
server=$!
for _ in $(seq 1 50); do curl -fs "http://127.0.0.1:$port/latest.json" >/dev/null && break; sleep 0.2; done
sum_of() { "$py" -c "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "$1"; }
want="$(cat "$tmp/release/want.sha256")"
update() { FLEET_NATIVE_TEST_BUILD=1 FLEET_NATIVE_UPDATE_URL="http://127.0.0.1:$port" "$@" --update-now --no-restart; }

# Off: nothing is replaced.
FLEET_NATIVE_UPDATES=off update "$tmp/app/$exe"
[ "$(sum_of "$tmp/app/$exe")" != "$want" ] || { echo "an off app was updated" >&2; exit 1; }
[ "$(cat "$tmp/app/fleet/version")" = "server 1" ] || { echo "an off app's server was changed" >&2; exit 1; }

# The app folder: the program and the server are replaced together.
update "$tmp/app/$exe"
got="$(sum_of "$tmp/app/$exe")"
if [ "$got" != "$want" ]; then echo "the program was not replaced ($got, wanted $want)" >&2; exit 1; fi
[ "$(cat "$tmp/app/fleet/version")" = "server 2" ] || { echo "the server was not replaced" >&2; exit 1; }
[ ! -e "$tmp/app/fleet/lib/only-in-the-old-server" ] || { echo "the old server's files were mixed in" >&2; exit 1; }
[ "$(cat "$tmp/app/mine.txt")" = "mine" ] || { echo "a user's file was lost" >&2; exit 1; }
# (On Windows `self-replace` leaves its own helper files until this process is gone, so only the others are checked there.)
if ls -A "$tmp/app" | grep -E '^\.fleet-native\.update|\.old$'; then echo "a staged or old copy was left behind" >&2; exit 1; fi
echo "replaced in place with its server: $got"
ls -la "$tmp/app" "$tmp/app/fleet"
# Something is still there to run (on Windows the old exe is deleted by a helper after exit, so give it a moment).
sleep 2
"$tmp/app/$exe" --version

# Linux, AppImage: the file APPIMAGE names is replaced, not the program the image runs (here a copy in another folder).
if [ "$(uname -s)" = Linux ]; then
  mkdir -p "$tmp/img" "$tmp/mounted"
  cp "$src" "$tmp/img/Fleet-Native.AppImage"; cp "$src" "$tmp/mounted/fleet-native"
  APPIMAGE="$tmp/img/Fleet-Native.AppImage" update "$tmp/mounted/fleet-native"
  [ "$(sum_of "$tmp/img/Fleet-Native.AppImage")" = "$want" ] || { echo "the AppImage was not replaced" >&2; exit 1; }
  [ "$(sum_of "$tmp/mounted/fleet-native")" != "$want" ] || { echo "the mounted program was replaced" >&2; exit 1; }
  [ -x "$tmp/img/Fleet-Native.AppImage" ] || { echo "the new AppImage isn't executable" >&2; exit 1; }
  echo "AppImage replaced as one file"
fi
