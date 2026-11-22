#!/usr/bin/env bash
# Builds the Linux .deb: the app and Fleet's server in /opt/Fleet-Native, a /usr/bin link, the desktop entry and the
# icons (hicolor sizes). `dpkg -r fleet-native` removes all of it; nothing is written outside what it lists.
#   scripts/make-deb.sh <fleet-native binary> <build number> <server folder> <out .deb>
# Also writes <out>.sha256.
set -euo pipefail
bin="${1:?binary}"; build="${2:?build number}"; server="${3:?server folder}"; out="${4:?output .deb}"
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
pkg="$work/pkg"
opt="$pkg/opt/Fleet-Native"
mkdir -p "$opt" "$pkg/usr/bin" "$pkg/usr/share/applications" "$pkg/DEBIAN"
install -m 755 "$bin" "$opt/fleet-native"
cp -R "$server" "$opt/fleet"
ln -s /opt/Fleet-Native/fleet-native "$pkg/usr/bin/fleet-native"
install -m 644 "$root/packaging/fleet-native.desktop" "$pkg/usr/share/applications/fleet-native.desktop"
for d in "$root"/packaging/icons/*/; do
  s="$(basename "$d")"; mkdir -p "$pkg/usr/share/icons/hicolor/${s}x${s}/apps"
  install -m 644 "$d/fleet-native.png" "$pkg/usr/share/icons/hicolor/${s}x${s}/apps/fleet-native.png"
done
# Directories 755, so a umask of 077 on the builder doesn't make an app only root can run.
find "$pkg" -type d -exec chmod 755 {} +

size="$(du -sk --exclude=DEBIAN "$pkg" | cut -f1)"
# Depends: the libraries the program links (libxkbcommon, Wayland, WebKitGTK: it will not start without them) and opens at run time
# (libxkbcommon-x11 for X11 keys, the Vulkan loader to draw; any Vulkan driver provides the loader's ICD).
cat > "$pkg/DEBIAN/control" <<CONTROL
Package: fleet-native
Version: 0.0.${build}
Section: devel
Priority: optional
Architecture: amd64
Installed-Size: ${size}
Depends: libc6 (>= 2.31), libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-cursor0, libwayland-egl1, libvulkan1, libwebkit2gtk-4.1-0
Maintainer: Weave Fleet <noreply@users.noreply.github.com>
Homepage: https://github.com/pgermishuys/fleet-native
Description: Run and watch many AI coding agents at once (native experiment)
 Weave Fleet's desktop app, drawn natively on the GPU, with Fleet's server
 inside. Installs to /opt/Fleet-Native and does not replace Fleet's own app.
CONTROL

rm -f "$out"
dpkg-deb --root-owner-group -Zxz -z3 --build "$pkg" "$out"
( cd "$(dirname "$out")" && sha256sum "$(basename "$out")" > "$(basename "$out").sha256" )
dpkg-deb --info "$out"
ls -la "$out" "$out.sha256"
