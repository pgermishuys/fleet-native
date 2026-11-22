#!/usr/bin/env bash
# Builds "Fleet Native.app" and the .dmg that holds it. Run on macOS (uses codesign, hdiutil, plutil, otool).
# Usage: make-mac-app.sh <fleet-native binary> <build number> <out .dmg> [server folder]
# With a server folder (scripts/fetch-server.sh) it goes in Contents/Resources/fleet, where the app looks for it. With
# CHECK_SERVER_CMD set (the check-bundled-server example) that command proves the signed app still carries it as built.
# The disk image's title is "Fleet Native 0.0.<build>".
set -euo pipefail
bin="${1:?binary}"; build="${2:?build number}"; out="${3:?output .dmg}"; server="${4:-}"
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
app="$work/stage/Fleet Native.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/fleet-native"
if [ -n "$server" ]; then cp -R "$server" "$app/Contents/Resources/fleet"; fi
cp "$root/assets/icon/fleet-native.icns" "$app/Contents/Resources/fleet-native.icns"

# The oldest macOS the binary says it runs on.
minos=$(otool -l "$bin" | awk '/LC_BUILD_VERSION/{f=1} f&&/minos/{print $2; exit}')
minos="${minos:-11.0}"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Fleet Native</string>
  <key>CFBundleDisplayName</key><string>Fleet Native</string>
  <key>CFBundleIdentifier</key><string>io.tryweave.fleet-native</string>
  <key>CFBundleExecutable</key><string>fleet-native</string>
  <key>CFBundleIconFile</key><string>fleet-native</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.0.${build}</string>
  <key>CFBundleVersion</key><string>${build}</string>
  <key>LSMinimumSystemVersion</key><string>${minos}</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
</dict>
</plist>
PLIST
plutil -lint "$app/Contents/Info.plist"

# Fleet's own title is "Fleet <version>"; this one is "Fleet Native 0.0.<build>", the name the app goes by.
volname="Fleet Native 0.0.${build}"

# Ad hoc signature ("-"): no developer identity, but Apple silicon refuses to run unsigned code. Signed inside out: every
# Mach-O file of the server first (deepest first), then the program, then the bundle, which seals the rest as resources.
# No --deep: it signs in an order of its own and hides which file failed. (Piece E adds the identity, hardened runtime and
# a timestamp; they would change only the flags here.)
macho_files() { # every Mach-O file under $1, deepest first
  find "$1" -type f -print | while IFS= read -r f; do
    if file -b "$f" | grep -q 'Mach-O'; then printf '%s\n' "$f"; fi
  done | awk -F/ '{print NF "\t" $0}' | sort -rn | cut -f2-
}
macho_files "$app/Contents/Resources" | while IFS= read -r f; do codesign --force -s - "$f"; done
codesign --force -s - "$app/Contents/MacOS/fleet-native"
codesign --force -s - "$app"

# The plist is right, every Mach-O passes codesign --verify, and the server is the pinned one (scripts/check-mac-app.sh).
bash "$root/scripts/check-mac-app.sh" "$app" "$server" "${CHECK_SERVER_CMD:-}"

ln -s /Applications "$work/stage/Applications"
# Prove the staged folder here, before it is packed: checking it needs no disk-image mount, which the CI runners
# sometimes refuse ("Resource temporarily unavailable").
test -L "$work/stage/Applications"
rm -f "$out"
# Transient "Resource busy" errors happen on CI runners: retry, and let the last try show its error.
for i in 1 2 3 4; do
  hdiutil create -volname "$volname" -srcfolder "$work/stage" -ov -format UDZO "$out" && break
  [ "$i" = 4 ] && exit 1
  sleep $((i * 3))
done
rm -rf "$work"
