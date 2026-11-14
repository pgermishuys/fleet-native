#!/usr/bin/env bash
# Starts a scratch Fleet to point the app at (docs/FLEET.md, "A scratch Fleet"): a released server
# in a new, empty home, a scripted model instead of a real one, one workspace root and one session.
#
#   FLEET_SERVER=<dir with bin/fleet> WEAVE_FLEET=<weave-fleet checkout> bash scripts/scratch-fleet.sh
#   cargo run -- --server http://127.0.0.1:6411
#
# The scripted model replies from weave-fleet's mobile/e2e/fakellm.py: "run-tests" calls the shell
# tool once, "long-reply" streams a long answer and holds at step 20 until `curl -X POST
# 127.0.0.1:6412/release`. Stop both by the process listening on each port. Needs opencode on PATH.
set -euo pipefail
: "${FLEET_SERVER:?the unpacked fleet-v<version>-<os> folder (its bin/fleet)}"
: "${WEAVE_FLEET:?a weave-fleet checkout, for mobile/e2e/fakellm.py}"
PORT=${PORT:-6411}
LLM=${LLM:-6412}
WORK=${WORK:-${TMPDIR:-/tmp}/fleet-native-scratch}
[ "$PORT" != 6262 ] || { echo "6262 is where a real Fleet lives"; exit 1; }
! (ss -ltn 2> /dev/null | grep -qE ":($PORT|$LLM)\b") || { echo "port $PORT or $LLM is in use"; exit 1; }
command -v opencode > /dev/null || { echo "opencode isn't on PATH"; exit 1; }

H=$WORK/home
rm -rf "$WORK"
mkdir -p "$H/.config/opencode" "$H/.local/share" "$H/.local/state" "$H/.cache" "$H/tmp" "$WORK/proj/demo"
cat > "$H/.config/opencode/opencode.json" << JSON
{"\$schema":"https://opencode.ai/config.json","model":"fakellm/fake-model","provider":{"fakellm":{"npm":"@ai-sdk/openai-compatible","options":{"baseURL":"http://127.0.0.1:$LLM/v1","apiKey":"x"},"models":{"fake-model":{"name":"Fake"}}}}}
JSON
(cd "$WORK/proj/demo" && git init -q && echo "# demo" > README.md && git add . && git -c user.email=e@example.com -c user.name=e commit -qm init)

python3 "$WEAVE_FLEET/mobile/e2e/fakellm.py" "$LLM" "$WORK/fakellm.log" > /dev/null 2>&1 &
# A clean environment: nothing of the caller's home reaches the server. The launcher takes --port
# (the Fleet__Port variable is ignored by it, and without either it would take 6262).
env -i HOME="$H" XDG_CONFIG_HOME="$H/.config" XDG_DATA_HOME="$H/.local/share" XDG_CACHE_HOME="$H/.cache" XDG_STATE_HOME="$H/.local/state" \
  TMPDIR="$H/tmp" PATH="$(dirname "$(command -v opencode)"):/usr/bin:/bin" Fleet__DatabasePath="$H/fleet.db" Fleet__AnalyticsEnabled=false \
  Fleet__Update__CheckOnStartup=false ASPNETCORE_ENVIRONMENT=Production \
  "$FLEET_SERVER/bin/fleet" --port "$PORT" > "$WORK/fleet.log" 2>&1 &
for _ in $(seq 90); do curl -fs "http://127.0.0.1:$PORT/readyz" > /dev/null 2>&1 && break; sleep 1; done
curl -fs "http://127.0.0.1:$PORT/readyz" > /dev/null || { tail -20 "$WORK/fleet.log"; exit 1; }

api() { curl -fsS -H 'content-type: application/json' "$@"; }
api -X POST "http://127.0.0.1:$PORT/api/workspace-roots" -d "{\"path\":\"$WORK/proj\"}" > /dev/null
api -X POST "http://127.0.0.1:$PORT/api/sessions" -d "{\"directory\":\"$WORK/proj/demo\",\"title\":\"Fix pagination\",\"isolationStrategy\":\"existing\",\"harnessType\":\"opencode\",\"initialPrompt\":\"The pagination test fails on the last page. Can you look?\"}" > /dev/null
echo "Fleet ready at http://127.0.0.1:$PORT (scripted model on $LLM, files in $WORK)"
