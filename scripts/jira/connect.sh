#!/usr/bin/env bash
#MISE description="Add a Jira connection to Nirvana"
#USAGE arg "<storage>" help="Where to store the token" {
#USAGE   choices "keyring" "plaintext"
#USAGE }
#USAGE flag "--name <name>" help="Connection name (default: connection kind)"
#USAGE flag "--bin <bin>" default="cargo run --quiet --" help="Nirvana command, e.g. target/release/nirvana"
#USAGE flag "--dc" help="Use Jira Data Center instead of Jira Cloud"
set -euo pipefail
source "$MISE_PROJECT_ROOT/scripts/jira/lib.sh"

if [[ ${usage_dc:-} == true ]]; then
    kind=jira-dc
    credential=$PAT
else
    kind=jira-cloud
    credential=$EMAIL:$API_TOKEN
fi

read -ra bin <<<"${usage_bin:?}"
"${bin[@]}" connection add "${usage_name:-$kind}" \
    --kind "$kind" \
    --host "$HOST" \
    --credential "$credential" \
    --storage "${usage_storage:?}"
