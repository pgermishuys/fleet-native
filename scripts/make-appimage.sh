#!/usr/bin/env bash
# Builds the Linux AppImage: one file that runs with no install, carries Fleet's server (fleet/ beside the exe) and the
# icon and desktop entry. The libraries the app opens (libxkbcommon-x11, the Vulkan loader, WebKitGTK) are the system's.
#   scripts/make-appimage.sh <fleet-native binary> <build number> <server folder> <out .AppImage>
# Also writes <out>.sha256. appimagetool and the runtime come from packaging/appimage-tools (SHA-256 checked) and are kept
# in $APPIMAGE_TOOLS_DIR (default: ~/.cache/fleet-native-appimage-tools, an empty folder of its own); set APPIMAGETOOL and
# APPIMAGE_RUNTIME to use ones you already have. FUSE is not needed to build: the tool is run with --appimage-extract-and-run.
set -euo pipefail
bin="${1:?binary}"; build="${2:?build number}"; server="${3:?server folder}"; out="${4:?output .AppImage}"
root="$(cd "$(dirname "$0")/.." && pwd)"
tools="${APPIMAGE_TOOLS_DIR:-$HOME/.cache/fleet-native-appimage-tools}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

get_tool() { # <name> -> path of the checked file
  local name="$1" want url
  read -r _ want url < <(awk -v n="$name" '$1 == n' "$root/packaging/appimage-tools")
  mkdir -p "$tools"
  local file="$tools/$name-${want:0:12}"
  if [ ! -f "$file" ] || [ "$(sha256sum "$file" | cut -d' ' -f1)" != "$want" ]; then
    curl -fsSL --retry 5 --retry-all-errors -o "$file.part" "$url"
    mv "$file.part" "$file"
  fi
  local got; got="$(sha256sum "$file" | cut -d' ' -f1)"
  if [ "$got" != "$want" ]; then rm -f "$file"; echo "SHA-256 of $name is $got, the pin says $want: not using it" >&2; exit 1; fi
  chmod +x "$file"
  echo "$file"
}
appimagetool="${APPIMAGETOOL:-$(get_tool appimagetool)}"
runtime="${APPIMAGE_RUNTIME:-$(get_tool runtime)}"

dir="$work/Fleet-Native.AppDir"
mkdir -p "$dir/usr/bin" "$dir/usr/share/applications"
cp "$bin" "$dir/usr/bin/fleet-native"
cp -R "$server" "$dir/usr/bin/fleet"
cp "$root/packaging/fleet-native.desktop" "$dir/fleet-native.desktop"
cp "$root/packaging/fleet-native.desktop" "$dir/usr/share/applications/fleet-native.desktop"
cp "$root/packaging/icons/256/fleet-native.png" "$dir/fleet-native.png"
cp "$root/packaging/icons/256/fleet-native.png" "$dir/.DirIcon"
for d in "$root"/packaging/icons/*/; do
  s="$(basename "$d")"; mkdir -p "$dir/usr/share/icons/hicolor/${s}x${s}/apps"
  cp "$d/fleet-native.png" "$dir/usr/share/icons/hicolor/${s}x${s}/apps/fleet-native.png"
done
# The runtime sets APPIMAGE (the updater needs it) and mounts the image; the program finds fleet/ beside itself.
cat > "$dir/AppRun" <<'RUN'
#!/bin/sh
here="$(dirname "$(readlink -f "$0")")"
exec "$here/usr/bin/fleet-native" "$@"
RUN
chmod +x "$dir/AppRun"

rm -f "$out"
ARCH=x86_64 VERSION="0.0.$build" "$appimagetool" --appimage-extract-and-run --no-appstream --runtime-file "$runtime" "$dir" "$out"
chmod +x "$out"
( cd "$(dirname "$out")" && sha256sum "$(basename "$out")" > "$(basename "$out").sha256" )
ls -la "$out" "$out.sha256"
