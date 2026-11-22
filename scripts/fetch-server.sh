#!/usr/bin/env bash
# Stage Fleet's server for a package: download the pinned CLI archive (packaging/server-version), check its SHA-256,
# and put the archive's `app/` folder (the AOT publish output, less its debug symbols) in <out>, which a package
# carries as `fleet/` beside the exe. Nothing is used before its hash is checked.
#
#   scripts/fetch-server.sh <rid> <out> [download-dir]      rid: linux-x64 | osx-arm64 | win-x64 | win-arm64
#
# <out> must not exist or be empty. The archive is kept in [download-dir] (default: a temp folder, removed after) so CI
# can cache it by its pinned hash. Runs under Git-bash on Windows (it unpacks .zip with PowerShell there).
set -euo pipefail

rid="${1:?usage: fetch-server.sh <rid> <out> [download-dir]}"
out="${2:?usage: fetch-server.sh <rid> <out> [download-dir]}"
here="$(cd "$(dirname "$0")" && pwd)"
pin="${FLEET_SERVER_PIN:-$here/../packaging/server-version}"
base="${FLEET_SERVER_BASE_URL:-https://github.com/pgermishuys/weave-fleet/releases/download}"

tag="$(sed -n 's/^tag=//p' "$pin")"
[ -n "$tag" ] || { echo "no tag= line in $pin" >&2; exit 1; }
case "$rid" in win-*) ext=zip ;; *) ext=tar.gz ;; esac
name="fleet-$tag-$rid.$ext"
want="$(awk -v n="$name" '$2 == n { print $1 }' "$pin")"
[ -n "$want" ] || { echo "$pin pins no archive named $name" >&2; exit 1; }

if [ -e "$out" ] && [ -n "$(ls -A "$out")" ]; then echo "$out is not empty" >&2; exit 1; fi

if [ -n "${3:-}" ]; then dl="$3"; mkdir -p "$dl"; else dl="$(mktemp -d)"; trap 'rm -rf "$dl"' EXIT; fi
archive="$dl/$name"

hash_of() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# A cached archive is only used when it still matches the pin; a wrong one is fetched again, never trusted.
if [ -f "$archive" ] && [ "$(hash_of "$archive")" != "$want" ]; then rm -f "$archive"; fi
if [ ! -f "$archive" ]; then
  echo "downloading $name"
  curl -fsSL --retry 5 --retry-all-errors -o "$archive.part" "$base/$tag/$name"
  mv "$archive.part" "$archive"
fi
got="$(hash_of "$archive")"
if [ "$got" != "$want" ]; then
  rm -f "$archive"
  echo "SHA-256 of $name is $got, the pin says $want: not using it" >&2
  exit 1
fi
echo "$name: sha256 $got (as pinned)"

# Unpack into a folder of our own, then move `app/` into place.
work="$(mktemp -d)"
trap 'rm -rf "$work"; [ -z "${3:-}" ] && rm -rf "$dl"' EXIT
if [ "$ext" = zip ]; then
  powershell -NoProfile -Command "Expand-Archive -LiteralPath '$(cygpath -w "$archive" 2>/dev/null || echo "$archive")' -DestinationPath '$(cygpath -w "$work" 2>/dev/null || echo "$work")'"
else
  tar -xzf "$archive" -C "$work"
fi
app="$(find "$work" -mindepth 2 -maxdepth 2 -type d -name app | head -n 1)"
[ -n "$app" ] || { echo "$name has no app/ folder" >&2; exit 1; }

# Debug symbols are not shipped (Fleet's electron-builder leaves them out too).
find "$app" \( -name '*.pdb' -o -name '*.dbg' \) -type f -delete
mkdir -p "$(dirname "$out")"
rm -rf "$out"
mv "$app" "$out"
if [ ! -e "$out/WeaveFleet.Api" ] && [ ! -e "$out/WeaveFleet.Api.exe" ]; then echo "no WeaveFleet.Api in $out" >&2; exit 1; fi
if [ ! -e "$out/mods-host/host.js" ]; then echo "no mods-host/host.js in $out" >&2; exit 1; fi
echo "server staged in $out ($(du -sh "$out" | cut -f1))"
