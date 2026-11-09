#!/usr/bin/env bash
#MISE description="Add a Linear connection to Nirvana"
#USAGE arg "<storage>" help="Where to store the token" {
#USAGE   choices "keyring" "plaintext"
#USAGE }
#USAGE flag "--name <name>" default="linear" help="Connection name"
#USAGE flag "--bin <bin>" default="cargo run --quiet --" help="Nirvana command, e.g. target/release/nirvana"
set -euo pipefail
source "$MISE_PROJECT_ROOT/scripts/linear/lib.sh"

workspace=$(linear "query { organization { urlKey } }" | jq -r '.data.organization.urlKey')

read -ra bin <<<"${usage_bin:?}"
"${bin[@]}" connection add "${usage_name:?}" \
    --kind linear \
    --host "linear.app/$workspace" \
    --credential "$API_KEY" \
    --storage "${usage_storage:?}"
