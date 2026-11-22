#!/usr/bin/env bash
# The optional detached GPG signature of the Linux packages (component 25, piece E: row 25.25): `<file>.asc` beside each,
# made with the owner's key when it is set. Without it, it says so ("not signed: GPG_PRIVATE_KEY is not set") and the
# .sha256 files are what a download is checked by.
#
#   bash scripts/sign-linux.sh <file> [<file> ...]
#
# Secrets: GPG_PRIVATE_KEY (the armoured private key) and GPG_PASSPHRASE (may be empty for a key with none).
# SIGN_DRY_RUN=1 prints each gpg command instead of running it (the passphrase masked).
set -euo pipefail
[ "$#" -ge 1 ] || { echo "usage: sign-linux.sh <file> [<file> ...]" >&2; exit 2; }
if [ -z "${GPG_PRIVATE_KEY:-}" ]; then
  echo "not signed: GPG_PRIVATE_KEY is not set (each file has its .sha256)"
  exit 0
fi
dry="${SIGN_DRY_RUN:-}"
run() {
  local line="+" a
  for a in "$@"; do
    [ -n "${GPG_PASSPHRASE:-}" ] && a="${a//"$GPG_PASSPHRASE"/***}"
    line+=" $a"
  done
  echo "$line"
  [ -n "$dry" ] || "$@"
}
# A keyring of this run's own, gone when the script ends.
export GNUPGHOME; GNUPGHOME="$(mktemp -d)"; trap 'rm -rf "$GNUPGHOME"' EXIT
chmod 700 "$GNUPGHOME"
if [ -z "$dry" ]; then printf '%s\n' "$GPG_PRIVATE_KEY" | gpg --batch --import; else echo "+ gpg --batch --import <GPG_PRIVATE_KEY>"; fi
for f in "$@"; do
  run gpg --batch --yes --pinentry-mode loopback --passphrase "${GPG_PASSPHRASE:-}" --armor --detach-sign --output "$f.asc" "$f"
  run gpg --batch --verify "$f.asc" "$f"
done
[ -n "$dry" ] && echo "dry run: nothing was signed" || echo "signed: $# files"
