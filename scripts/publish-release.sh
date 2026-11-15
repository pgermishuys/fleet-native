#!/usr/bin/env bash
# Updates the rolling `latest` pre-release in place with the builds in $1. CI runs this (the `publish` job).
# Needs: GH_TOKEN, BUILD (a number that only goes up), COMMIT, and gh, jq in the PATH.
set -euo pipefail
dist="${1:?folder with the three assets}"
repo="${GITHUB_REPOSITORY:-pgermishuys/fleet-native}"
tag=latest
assets=(fleet-native-windows-x64.zip fleet-native-macos-arm64.dmg fleet-native-linux-x64.tar.gz)
for a in "${assets[@]}"; do [ -f "$dist/$a" ] || { echo "missing $a" >&2; exit 1; }; done

# A slower, older run must not replace a newer release.
prev=""
if gh release download "$tag" -R "$repo" -p latest.json -O "$dist/previous.json" 2>/dev/null; then
  have=$(jq -r '.build // 0' "$dist/previous.json")
  prev=$(jq -r '.commit // ""' "$dist/previous.json")
  rm "$dist/previous.json"
  if [ "$have" -ge "$BUILD" ]; then
    echo "release already has build $have, not replacing it with $BUILD"
    exit 0
  fi
fi

jq -n --arg commit "$COMMIT" --arg time "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --argjson build "$BUILD" '{commit:$commit, built_at:$time, build:$build, assets:{}}' > "$dist/latest.json"
for a in "${assets[@]}"; do
  size=$(wc -c < "$dist/$a" | tr -d ' ')
  sum=$(sha256sum "$dist/$a" | cut -d' ' -f1)
  jq --arg k "${a#fleet-native-}" --arg n "$a" --argjson s "$size" --arg h "$sum" \
    '.assets[$k | sub("\\..*$";"")] = {name:$n, size:$s, sha256:$h}' "$dist/latest.json" > "$dist/l.tmp"
  mv "$dist/l.tmp" "$dist/latest.json"
done

# Merged pull requests since the previous release: squash-merge titles end in (#n).
if [ -n "$prev" ] && git cat-file -e "$prev^{commit}" 2>/dev/null; then range="$prev..$COMMIT"; else range="-20 $COMMIT"; fi
prs=$(git log $range --format='%s' | grep -E '\(#[0-9]+\)$' | sed 's/^/- /' || true)
cat > "$dist/notes.md" <<N
**An unsigned development build** of fleet-native, made from \`${COMMIT:0:7}\` (build $BUILD). It is not signed or notarised, so
Windows and macOS warn before it runs (see the README's "Try it"). It is an experiment and will change daily.

Merged since the previous build:

${prs:-- (nothing new with a pull request number)}
N

# Update in place, so the download links never 404: the release is not deleted, assets are replaced one by one
# (--clobber), and latest.json goes last because it says what the assets are. CI allows one publish at a time.
# The `latest` tag is never created or moved from here: GitHub refuses a workflow's GITHUB_TOKEN a ref change that
# brings in edits to .github/workflows (HTTP 403, "Resource not accessible by integration"). Downloads and
# latest.json (commit and build) don't depend on where the tag points.
step() { echo "publish: $*"; }
run() { step "$*"; "$@" || { echo "::error::publish step failed: $*" >&2; exit 1; }; }

if gh release view "$tag" -R "$repo" >/dev/null 2>&1; then
  run gh release upload "$tag" -R "$repo" --clobber "${assets[@]/#/$dist/}" "$dist/latest.json"
  run gh release edit "$tag" -R "$repo" --title "Latest build ($BUILD)" --notes-file "$dist/notes.md" --prerelease --latest=false
  # An asset from an older layout (the macOS .tar.gz) that is no longer built.
  for old in $(gh release view "$tag" -R "$repo" --json assets -q '.assets[].name'); do
    case " ${assets[*]} latest.json " in *" $old "*) ;; *) run gh release delete-asset "$tag" -R "$repo" "$old" --yes ;; esac
  done
else
  # Creating the release creates its tag, which the workflow token may be refused. Don't try to work around it.
  echo "::error::the release '$tag' does not exist. Create it once as the repository owner (a workflow token may not create the tag):" >&2
  echo "  gh release create $tag -R $repo --prerelease --latest=false --title 'Latest build' --notes 'placeholder'" >&2
  echo "then re-run this job." >&2
  exit 1
fi
