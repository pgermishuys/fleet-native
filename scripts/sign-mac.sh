#!/usr/bin/env bash
# Developer ID signing and notarising for "Fleet Native.app" and its .dmg (component 25, piece E: rows 25.20 to 25.22).
# Runs only when the owner's secrets are set; without them it says so ("not signed: … is not set") and leaves the ad hoc
# signature make-mac-app.sh gave the app, so a build with no secrets stays green.
#
#   bash scripts/sign-mac.sh app <Fleet Native.app>   # sign inside out (hardened runtime, timestamp, entitlements),
#                                                      # then notarise the app and staple the ticket to it
#   bash scripts/sign-mac.sh dmg <file.dmg>           # sign the image, notarise it, staple it, check it with spctl
#
# Signing:    APPLE_CERTIFICATE (base64 .p12), APPLE_CERTIFICATE_PASSWORD, APPLE_SIGNING_IDENTITY
#             ("Developer ID Application: <name> (<team>)").
# Notarising: APPLE_API_KEY (base64 .p8), APPLE_API_KEY_ID, APPLE_API_ISSUER; or APPLE_ID, APPLE_APP_SPECIFIC_PASSWORD,
#             APPLE_TEAM_ID. Notarising needs signing.
# SIGN_DRY_RUN=1 runs every step but prints each signing, keychain and notary command instead of running it (secret values
# masked), so the path can be proven with no certificate.
set -euo pipefail
what="${1:?app or dmg}"; target="${2:?path}"
root="$(cd "$(dirname "$0")/.." && pwd)"
entitlements="$root/packaging/mac/entitlements.plist"
dry="${SIGN_DRY_RUN:-}"

if [ -z "${APPLE_CERTIFICATE:-}" ]; then
  echo "not signed: APPLE_CERTIFICATE is not set (the app keeps its ad hoc signature; Gatekeeper asks before the first run)"
  exit 0
fi
for v in APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY; do
  # Half a set of secrets is a mistake to fix, not a build to ship unsigned.
  [ -n "${!v:-}" ] || { echo "APPLE_CERTIFICATE is set but $v is not set" >&2; exit 1; }
done

# Prints a command with every secret's value replaced, then runs it unless this is a dry run.
run() {
  local line="+" a
  for a in "$@"; do
    for s in APPLE_CERTIFICATE_PASSWORD APPLE_APP_SPECIFIC_PASSWORD KEYCHAIN_PASSWORD APPLE_API_KEY_ID APPLE_API_ISSUER APPLE_ID APPLE_TEAM_ID; do
      [ -n "${!s:-}" ] && a="${a//"${!s}"/***}"
    done
    case "$a" in *" "*) line+=" '$a'" ;; *) line+=" $a" ;; esac
  done
  echo "$line"
  [ -n "$dry" ] || "$@"
}

work="$(mktemp -d)"
keychain="$work/signing.keychain-db"
KEYCHAIN_PASSWORD="$(openssl rand -hex 16 2> /dev/null || echo "$RANDOM$RANDOM$RANDOM")"
cleanup() {
  [ -n "$dry" ] || security delete-keychain "$keychain" > /dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT

# The certificate goes into a keychain of this run's own, never the login keychain, and is gone when the script ends.
if [ -z "$dry" ]; then printf '%s' "$APPLE_CERTIFICATE" | base64 --decode > "$work/cert.p12"; else : > "$work/cert.p12"; fi
run security create-keychain -p "$KEYCHAIN_PASSWORD" "$keychain"
run security set-keychain-settings -lut 21600 "$keychain"
run security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$keychain"
run security import "$work/cert.p12" -k "$keychain" -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
run security set-key-partition-list -S apple-tool:,apple: -s -k "$KEYCHAIN_PASSWORD" "$keychain"
run security list-keychains -d user -s "$keychain" login.keychain-db
sign=(codesign --force --timestamp --options runtime --keychain "$keychain" -s "$APPLE_SIGNING_IDENTITY")

# Notarises one file (a .zip of the app, or the .dmg) and waits for Apple's answer.
notarise() {
  if [ -n "${APPLE_API_KEY:-}" ]; then
    for v in APPLE_API_KEY_ID APPLE_API_ISSUER; do [ -n "${!v:-}" ] || { echo "APPLE_API_KEY is set but $v is not set" >&2; exit 1; }; done
    if [ -z "$dry" ]; then printf '%s' "$APPLE_API_KEY" | base64 --decode > "$work/key.p8"; fi
    run xcrun notarytool submit "$1" --key "$work/key.p8" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER" --wait
  elif [ -n "${APPLE_ID:-}" ]; then
    for v in APPLE_APP_SPECIFIC_PASSWORD APPLE_TEAM_ID; do [ -n "${!v:-}" ] || { echo "APPLE_ID is set but $v is not set" >&2; exit 1; }; done
    run xcrun notarytool submit "$1" --apple-id "$APPLE_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" --team-id "$APPLE_TEAM_ID" --wait
  else
    echo "not notarised: APPLE_API_KEY is not set (nor APPLE_ID); signed, but Gatekeeper still asks before the first run"
    return 1
  fi
}

case "$what" in
  app)
    # Inside out, as make-mac-app.sh signs ad hoc: every Mach-O file of the server, deepest first, then the program, then
    # the bundle. The entitlements go on everything executable; a library ignores them.
    find "$target/Contents/Resources" -type f -print | while IFS= read -r f; do
      if file -b "$f" | grep -q 'Mach-O'; then printf '%s\n' "$f"; fi
    done | awk -F/ '{print NF "\t" $0}' | sort -rn | cut -f2- > "$work/macho"
    echo "signing $(wc -l < "$work/macho" | tr -d ' ') Mach-O files of the server, then the program and the bundle"
    while IFS= read -r f; do run "${sign[@]}" --entitlements "$entitlements" "$f"; done < "$work/macho"
    run "${sign[@]}" --entitlements "$entitlements" "$target/Contents/MacOS/fleet-native"
    run "${sign[@]}" --entitlements "$entitlements" "$target"
    run codesign --verify --deep --strict --verbose=2 "$target"
    # The app is notarised (as a zip, which is what notarytool takes for an app) and its ticket stapled, so it opens
    # without a network check even when copied out of the image.
    run ditto -c -k --keepParent "$target" "$work/app.zip"
    if notarise "$work/app.zip"; then
      run xcrun stapler staple "$target"
      run spctl --assess --type execute --verbose=2 "$target"
    fi
    ;;
  dmg)
    run codesign --force --timestamp --keychain "$keychain" -s "$APPLE_SIGNING_IDENTITY" "$target"
    if notarise "$target"; then
      run xcrun stapler staple "$target"
      run xcrun stapler validate "$target"
      run spctl --assess --type open --context context:primary-signature --verbose=2 "$target"
    fi
    ;;
  *) echo "usage: sign-mac.sh app <Fleet Native.app> | dmg <file.dmg>" >&2; exit 2 ;;
esac
[ -n "$dry" ] && echo "dry run: nothing was signed" || echo "signed: $target"
