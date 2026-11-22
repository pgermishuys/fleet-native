#!/usr/bin/env bash
# Proves the Linux packages (rows 25.5, 25.10, 25.11, 25.25) without installing anything: the desktop file validates, the
# .deb says what it depends on and holds the app, the server, the link, the entry and the icons, the AppImage lists the
# same, and both carry the server exactly as pinned (check-bundled-server). Unpacks into a scratch folder, removed after.
#   scripts/check-linux-packages.sh <dir holding the .deb and .AppImage> <staged server> <check-bundled-server binary>
set -euo pipefail
dist="${1:?dir}"; server="${2:?staged server}"; checker="${3:?check-bundled-server}"
root="$(cd "$(dirname "$0")/.." && pwd)"
deb="$dist/fleet-native-linux-x64.deb"; img="$dist/fleet-native-linux-x64.AppImage"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
has() { grep -qE -- "$2" "$1" || fail "$3 (no match for $2 in $1)"; }

echo "== desktop file"
# desktop-file-validate fails on any error; a hint (like a deprecated key) is only printed.
VALIDATE="${DESKTOP_FILE_VALIDATE:-desktop-file-validate}"
"$VALIDATE" "$root/packaging/fleet-native.desktop"
has "$root/packaging/fleet-native.desktop" '^StartupWMClass=fleet-native$' "StartupWMClass is the window's app id"
has "$root/packaging/fleet-native.desktop" '^Categories=Development;$' "Categories"
has "$root/packaging/fleet-native.desktop" '^Comment=.+' "Comment"
echo ok

echo "== .deb"
dpkg-deb --info "$deb" | tee "$work/info"
has "$work/info" '^ Package: fleet-native$' "package name"
has "$work/info" '^ Version: 0\.0\.[0-9]+$' "version is 0.0.<build>"
has "$work/info" '^ Architecture: amd64$' "architecture"
has "$work/info" '^ Depends: .*libxkbcommon-x11-0.*libvulkan1.*libwebkit2gtk-4\.1-0' "Depends lists the libraries the app opens"
dpkg-deb --contents "$deb" > "$work/deb.list"
has "$work/deb.list" ' \./opt/Fleet-Native/fleet-native$' "the app in /opt/Fleet-Native"
has "$work/deb.list" ' \./opt/Fleet-Native/fleet/WeaveFleet\.Api$' "the server beside it"
has "$work/deb.list" ' \./opt/Fleet-Native/fleet/mods-host/host\.js$' "the server's mods host"
has "$work/deb.list" ' \./usr/bin/fleet-native -> /opt/Fleet-Native/fleet-native$' "the /usr/bin link"
has "$work/deb.list" ' \./usr/share/applications/fleet-native\.desktop$' "the desktop entry"
for s in 16 24 32 48 64 128 256 512; do
  has "$work/deb.list" " \./usr/share/icons/hicolor/${s}x${s}/apps/fleet-native\.png$" "hicolor ${s}x${s}"
done
# Nothing outside /opt/Fleet-Native, /usr/bin, /usr/share (so a removal leaves nothing behind).
if grep -vE ' \./(opt/Fleet-Native|usr/(bin|share))(/|$)| \./(opt|usr)/$| \./$' "$work/deb.list" | grep -q .; then fail "files outside /opt/Fleet-Native, /usr/bin and /usr/share"; fi
mkdir "$work/deb"; dpkg -x "$deb" "$work/deb"
"$VALIDATE" "$work/deb/usr/share/applications/fleet-native.desktop"
"$checker" "$server" "$work/deb"

echo "== AppImage"
chmod +x "$img"
mkdir "$work/img"
# --appimage-extract needs no FUSE; it is the runtime's own listing of what the image holds.
( cd "$work/img" && "$img" --appimage-extract > "$work/img.list" )
( cd "$work/img/squashfs-root" && find . -maxdepth 4 | sort | sed -n '1,60p' )
for f in AppRun fleet-native.desktop fleet-native.png .DirIcon usr/bin/fleet-native usr/bin/fleet/WeaveFleet.Api usr/bin/fleet/mods-host/host.js usr/share/icons/hicolor/256x256/apps/fleet-native.png; do
  [ -e "$work/img/squashfs-root/$f" ] || fail "the AppImage lacks $f"
done
[ -x "$work/img/squashfs-root/AppRun" ] || fail "AppRun is not executable"
"$VALIDATE" "$work/img/squashfs-root/fleet-native.desktop"
# appimagetool adds its own X-AppImage keys to the entry at the image root, so only the keys that matter are compared.
has "$work/img/squashfs-root/fleet-native.desktop" '^StartupWMClass=fleet-native$' "the AppImage entry keeps StartupWMClass"
"$checker" "$server" "$work/img/squashfs-root"

echo "== hashes"
for f in "$deb" "$img"; do
  [ -f "$f.sha256" ] || fail "no $f.sha256"
  ( cd "$dist" && sha256sum -c "$(basename "$f").sha256" )
done
echo "Linux packages ok"
