#!/usr/bin/env bash
#MISE description="Set up a dev DB with a profile and Jira Cloud and Linear connections"
#USAGE arg "<storage>" help="Where to store the tokens" {
#USAGE   choices "keyring" "plaintext"
#USAGE }
#USAGE flag "--profile <profile>" default="dev" help="Profile name"
#USAGE flag "--bin <bin>" default="cargo run --quiet --" help="Nirvana command, e.g. target/release/nirvana"
set -euo pipefail

storage=${usage_storage:?}

read -ra bin <<<"${usage_bin:?}"
"${bin[@]}" profile create "${usage_profile:?}" --switch

mise run jira:connect "$storage" --bin "$usage_bin"
mise run linear:connect "$storage" --bin "$usage_bin"
