#!/usr/bin/env bash
# Proves the updater's replace step on this system with no window and no GitHub: copies a debug binary to a temp folder and
# runs it with FLEET_NATIVE_TEST_BUILD=1 (a debug build reads it), so it believes it is build 1; serves a fake "latest" release from loopback whose
# build 2 asset holds a slightly different program, runs `fleet-native --update-now --no-restart` on the copy, and
# checks the copy became the new program (by SHA-256). Linux and Windows; macOS is proved by a test with a real .dmg.
#
#   cargo build && bash scripts/update-check.sh target/debug/fleet-native
set -euo pipefail
src="${1:?path of a debug build}"
tmp="$(mktemp -d)"
trap 'kill "${server:-0}" 2>/dev/null || true; rm -rf "$tmp"' EXIT
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) src="${src%.exe}.exe"; key=windows-x64; name=fleet-native-windows-x64.zip; exe=fleet-native.exe ;;
  Linux) key=linux-x64; name=fleet-native-linux-x64.tar.gz; exe=fleet-native ;;
  *) echo "not for this system" >&2; exit 2 ;;
esac
py=python3; command -v python3 >/dev/null || py=python
mkdir -p "$tmp/app" "$tmp/release"
cp "$src" "$tmp/app/$exe"
"$tmp/app/$exe" --version
# The "new build": the same program with a few bytes after the end, which neither ELF nor PE minds.
"$py" - "$src" "$tmp/release" "$name" "$exe" "$key" <<'P'
import sys, hashlib, json, tarfile, zipfile, os
src, out, name, exe, key = sys.argv[1:6]
data = open(src, 'rb').read() + b'\0update-check build 2'
open(os.path.join(out, 'new-program'), 'wb').write(data)
path = os.path.join(out, name)
if name.endswith('.zip'):
    with zipfile.ZipFile(path, 'w', zipfile.ZIP_DEFLATED) as z:
        z.write(os.path.join(out, 'new-program'), exe)
else:
    with tarfile.open(path, 'w:gz') as t:
        t.add(os.path.join(out, 'new-program'), exe)
blob = open(path, 'rb').read()
json.dump({"commit": "0" * 40, "built_at": "2026-11-15T14:19:00Z", "build": 2,
           "assets": {key: {"name": name, "size": len(blob), "sha256": hashlib.sha256(blob).hexdigest()}}},
          open(os.path.join(out, 'latest.json'), 'w'))
open(os.path.join(out, 'want.sha256'), 'w').write(hashlib.sha256(data).hexdigest())
P
port=$((20000 + RANDOM % 20000))
(cd "$tmp/release" && exec "$py" -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1) &
server=$!
for _ in $(seq 1 50); do curl -fs "http://127.0.0.1:$port/latest.json" >/dev/null && break; sleep 0.2; done
FLEET_NATIVE_TEST_BUILD=1 FLEET_NATIVE_UPDATE_URL="http://127.0.0.1:$port" "$tmp/app/$exe" --update-now --no-restart
got="$("$py" -c "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "$tmp/app/$exe")"
want="$(cat "$tmp/release/want.sha256")"
if [ "$got" != "$want" ]; then echo "the program was not replaced ($got, wanted $want)" >&2; exit 1; fi
echo "replaced in place: $got"
ls -la "$tmp/app"
# Something is still there to run (on Windows the old exe is deleted by a helper after exit, so give it a moment).
sleep 2
"$tmp/app/$exe" --version
