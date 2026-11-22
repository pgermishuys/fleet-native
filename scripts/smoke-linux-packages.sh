#!/usr/bin/env bash
# Smoke-tests each Linux *package*, not the build (row 25.41): the .deb unpacked with `dpkg -x` (no root, nothing installed),
# the AppImage unpacked with its own `--appimage-extract` (no FUSE) and started through its AppRun, and the tar when there
# is one. Each runs scripts/smoke-desktop.sh with the server it carries: own Fleet started, a repository listed, the app
# killed, the Fleet gone, the app started again on the same data.
#   scripts/smoke-linux-packages.sh <dir holding fleet-native-linux-x64.{deb,AppImage}[,.tar.gz]> [picture folder]
# With a picture folder, each run grabs the screen once the app has listed the repository (smoke-<kind>.png).
set -euo pipefail
dist="$(cd "${1:?dir}" && pwd)"
shots="${2:-}"; [ -n "$shots" ] && mkdir -p "$shots" && shots="$(cd "$shots" && pwd)"
shot() { [ -n "$shots" ] && echo "$shots/smoke-$1.png"; }
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
deb="$dist/fleet-native-linux-x64.deb"; img="$dist/fleet-native-linux-x64.AppImage"; tar="$dist/fleet-native-linux-x64.tar.gz"
# The app must find the server it carries, never one named by the environment.
unset FLEET_NATIVE_SERVER_BIN FLEET_NATIVE_SERVER_CONTENT_ROOT

echo "== .deb (dpkg -x)"
mkdir "$work/deb"; dpkg -x "$deb" "$work/deb"
SMOKE_SHOT="$(shot deb)" bash "$here/smoke-desktop.sh" "$work/deb/opt/Fleet-Native/fleet-native"

echo "== AppImage (AppRun from --appimage-extract)"
chmod +x "$img"; mkdir "$work/img"
( cd "$work/img" && "$img" --appimage-extract > /dev/null )
# AppRun execs the program, so the pid the smoke test kills is the app's own.
SMOKE_SHOT="$(shot appimage)" APPIMAGE="$img" bash "$here/smoke-desktop.sh" "$work/img/squashfs-root/AppRun"

if [ -f "$tar" ]; then
  echo "== tar"
  mkdir "$work/tar"; tar -xzf "$tar" -C "$work/tar"
  SMOKE_SHOT="$(shot tar)" bash "$here/smoke-desktop.sh" "$work/tar/fleet-native"
fi
echo "Linux packages started, answered and stopped"
