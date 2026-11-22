#!/usr/bin/env bash
# Proves a macOS app (rows 25.15, 25.16): Info.plist is valid and says what it must, every Mach-O file inside passes
# `codesign --verify` and so does the bundle, and the server is the pinned one (Fleet's check-bundled-server rule, which
# lets only a re-signed Mach-O file differ). Run on macOS, on the staged app and on the app inside a mounted .dmg.
#   scripts/check-mac-app.sh <Fleet Native.app> [staged server] [check-bundled-server binary]
set -euo pipefail
app="${1:?app}"; server="${2:-}"; checker="${3:-}"
fail() { echo "FAIL: $*" >&2; exit 1; }
plist="$app/Contents/Info.plist"

echo "== Info.plist"
plutil -lint "$plist"
plutil -p "$plist"
want() { [ "$(plutil -extract "$1" raw -o - "$plist")" = "$2" ] || fail "Info.plist $1 is not $2"; }
want CFBundleIdentifier io.tryweave.fleet-native
want CFBundleName "Fleet Native"
want CFBundleExecutable fleet-native
want CFBundlePackageType APPL
want NSHighResolutionCapable true
want LSApplicationCategoryType public.app-category.developer-tools
plutil -extract LSMinimumSystemVersion raw -o - "$plist" | grep -qE '^[0-9]+(\.[0-9]+)*$' || fail "LSMinimumSystemVersion"
plutil -extract CFBundleShortVersionString raw -o - "$plist" | grep -qE '^0\.0\.[0-9]+$' || fail "version is 0.0.<build>"
# The app has a dock icon: not an agent.
if plutil -extract LSUIElement raw -o - "$plist" > /dev/null 2>&1; then fail "LSUIElement is set"; fi
test -f "$app/Contents/Resources/$(plutil -extract CFBundleIconFile raw -o - "$plist").icns" || fail "the icon is missing"

echo "== Mach-O files"
n=0
while IFS= read -r f; do
  if file -b "$f" | grep -q 'Mach-O'; then
    codesign --verify --strict "$f" || fail "codesign --verify: ${f#"$app"/}"
    n=$((n + 1))
  fi
done < <(find "$app" -type f -print)
[ "$n" -ge 1 ] || fail "no Mach-O file found"
echo "$n Mach-O files, each passes codesign --verify --strict"
codesign --verify --deep --strict --verbose=2 "$app"
codesign -dv "$app" 2>&1 | grep -v '^Executable='

if [ -n "$server" ] && [ -n "$checker" ]; then
  echo "== server as pinned"
  [ -d "$app/Contents/Resources/fleet" ] || fail "no Contents/Resources/fleet"
  "$checker" "$server" "$app"
fi
