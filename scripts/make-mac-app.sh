#!/usr/bin/env bash
# Builds "Fleet Native.app" and the .dmg that holds it. Run on macOS (uses codesign, hdiutil, plutil, otool).
# Usage: make-mac-app.sh <fleet-native binary> <build number> <out .dmg>
set -euo pipefail
bin="${1:?binary}"; build="${2:?build number}"; out="${3:?output .dmg}"
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
app="$work/stage/Fleet Native.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/fleet-native"
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

# Ad hoc signature ("-"): no developer identity, but Apple silicon refuses to run unsigned code.
codesign --force --deep -s - "$app"
codesign --verify --deep --strict "$app"

ln -s /Applications "$work/stage/Applications"
# Prove the staged folder here, before it is packed: checking it needs no disk-image mount, which the CI runners
# sometimes refuse ("Resource temporarily unavailable").
test -L "$work/stage/Applications"
codesign --verify --deep --strict --verbose=2 "$app"
plutil -p "$app/Contents/Info.plist"
rm -f "$out"
# Transient "Resource busy" errors happen on CI runners: retry, and let the last try show its error.
for i in 1 2 3 4; do
  hdiutil create -volname "Fleet Native" -srcfolder "$work/stage" -ov -format UDZO "$out" && break
  [ "$i" = 4 ] && exit 1
  sleep $((i * 3))
done
rm -rf "$work"
