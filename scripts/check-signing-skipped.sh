#!/usr/bin/env bash
# The test of piece E's signing steps on a runner with no secrets (rows 25.20 to 25.25): each signing script, given none,
# must say "not signed: <secret> is not set" and succeed; then, given invented values and SIGN_DRY_RUN=1, it must run every
# step but only print the signing calls, with the invented password nowhere in what it printed.
#
#   bash scripts/check-signing-skipped.sh <the debug fleet-native program>
#
# The invented values are not secrets and open nothing: there is no certificate behind them.
set -euo pipefail
exe="${1:?the fleet-native program}"
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
# Fails unless the file holds a line matching the pattern.
has() { grep -qE -- "$2" "$1" || { cat "$1"; fail "$3"; }; }
fake_password='invented-password-0000'
# Whatever the runner's environment has, the first run sees no secret at all.
unset APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY APPLE_API_KEY APPLE_API_KEY_ID APPLE_API_ISSUER \
  APPLE_ID APPLE_APP_SPECIFIC_PASSWORD APPLE_TEAM_ID WINDOWS_CERTIFICATE WINDOWS_CERTIFICATE_PASSWORD GPG_PRIVATE_KEY GPG_PASSPHRASE

case "$(uname -s)" in
  Darwin)
    plutil -lint "$here/../packaging/mac/entitlements.plist"
    # A bundle shaped like the real one: the program, and a Mach-O file where the server's are.
    app="$work/Fleet Native.app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/fleet"
    cp "$exe" "$app/Contents/MacOS/fleet-native"; cp "$exe" "$app/Contents/Resources/fleet/WeaveFleet.Api"
    : > "$work/image.dmg"
    echo "== sign-mac.sh with no secrets"
    bash "$here/sign-mac.sh" app "$app" | tee "$work/out"
    has "$work/out" '^not signed: APPLE_CERTIFICATE is not set' "the app step did not say it skipped"
    bash "$here/sign-mac.sh" dmg "$work/image.dmg" | tee "$work/out"
    has "$work/out" '^not signed: APPLE_CERTIFICATE is not set' "the image step did not say it skipped"
    echo "== sign-mac.sh, dry run with invented values"
    SIGN_DRY_RUN=1 APPLE_CERTIFICATE=aW52ZW50ZWQ= APPLE_CERTIFICATE_PASSWORD="$fake_password" \
      APPLE_SIGNING_IDENTITY='Developer ID Application: Example (EXAMPLE000)' bash "$here/sign-mac.sh" app "$app" | tee "$work/out"
    has "$work/out" "^\+ codesign --force --timestamp --options runtime .* --entitlements .*entitlements\.plist .*/Resources/fleet/WeaveFleet\.Api'?$" "the server was not signed first"
    has "$work/out" "^\+ codesign .*/Contents/MacOS/fleet-native'?$" "the program was not signed"
    has "$work/out" '^not notarised: APPLE_API_KEY is not set' "notarising did not say it skipped"
    has "$work/out" '^dry run: nothing was signed$' "the dry run did not finish"
    # Inside out: the server's file before the program, the program before the bundle.
    order=$(grep -nE "^\+ codesign --force" "$work/out" | sed -E 's/^([0-9]+):.*(WeaveFleet\.Api|MacOS\/fleet-native|Fleet Native\.app)'"'"'?$/\2/')
    [ "$(printf '%s\n' "$order" | tr '\n' ' ')" = "WeaveFleet.Api MacOS/fleet-native Fleet Native.app " ] || fail "not signed inside out: $order"
    ! grep -q "$fake_password" "$work/out" || fail "the password was printed"
    SIGN_DRY_RUN=1 APPLE_CERTIFICATE=aW52ZW50ZWQ= APPLE_CERTIFICATE_PASSWORD="$fake_password" \
      APPLE_SIGNING_IDENTITY='Developer ID Application: Example (EXAMPLE000)' APPLE_API_KEY=aW52ZW50ZWQ= APPLE_API_KEY_ID=EXAMPLEKEY \
      APPLE_API_ISSUER=00000000-0000-0000-0000-000000000000 bash "$here/sign-mac.sh" dmg "$work/image.dmg" | tee "$work/out"
    has "$work/out" '^\+ xcrun notarytool submit .*image\.dmg --key .* --key-id \*\*\* --issuer \*\*\* --wait$' "the image was not notarised"
    has "$work/out" '^\+ xcrun stapler staple ' "the ticket was not stapled"
    ! grep -q "$fake_password" "$work/out" || fail "the password was printed"
    ;;
  Linux)
    : > "$work/fleet-native-linux-x64.AppImage"
    echo "== sign-linux.sh with no secrets"
    bash "$here/sign-linux.sh" "$work/fleet-native-linux-x64.AppImage" | tee "$work/out"
    has "$work/out" '^not signed: GPG_PRIVATE_KEY is not set' "the GPG step did not say it skipped"
    echo "== sign-linux.sh, dry run with invented values"
    SIGN_DRY_RUN=1 GPG_PRIVATE_KEY=invented GPG_PASSPHRASE="$fake_password" bash "$here/sign-linux.sh" "$work/fleet-native-linux-x64.AppImage" | tee "$work/out"
    has "$work/out" '^\+ gpg .*--passphrase \*\*\* --armor --detach-sign --output .*\.AppImage\.asc .*\.AppImage$' "the AppImage was not signed"
    has "$work/out" '^dry run: nothing was signed$' "the dry run did not finish"
    ! grep -q "$fake_password" "$work/out" || fail "the passphrase was printed"
    ;;
  MINGW* | MSYS* | CYGWIN*)
    echo "== sign-windows.ps1 with no secrets"
    pwsh -NoProfile -File "$here/sign-windows.ps1" "$exe" | tee "$work/out"
    has "$work/out" '^not signed: WINDOWS_CERTIFICATE is not set' "the Windows step did not say it skipped"
    echo "== sign-windows.ps1, dry run with invented values"
    SIGN_DRY_RUN=1 WINDOWS_CERTIFICATE=aW52ZW50ZWQ= WINDOWS_CERTIFICATE_PASSWORD="$fake_password" \
      pwsh -NoProfile -File "$here/sign-windows.ps1" "$exe" | tee "$work/out"
    has "$work/out" '^\+ .*signtool\.exe'"'"'? sign /fd SHA256 /f .* /p \*\*\* /tr http://timestamp\.digicert\.com /td SHA256 .*fleet-native\.exe' "the exe was not signed"
    has "$work/out" '^dry run: nothing was signed' "the dry run did not finish"
    ! grep -q "$fake_password" "$work/out" || fail "the password was printed"
    ;;
esac
echo "signing with no secrets: skipped, and the dry run printed every call"
