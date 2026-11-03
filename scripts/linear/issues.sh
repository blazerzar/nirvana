#!/usr/bin/env bash
#MISE description="Show titles of several Linear issues"
#USAGE arg "<keys>..." help="Issue identifiers, e.g. ENG-123 ENG-124"
set -euo pipefail
source "$MISE_PROJECT_ROOT/scripts/linear/lib.sh"

read -ra keys <<<"${usage_keys:?}"

linear "$(jq --null-input --raw-output --args \
    '$ARGS.positional | to_entries
    | map("i\(.key): issue(id: \(.value | tojson)) { identifier title }")
    | "query { \(join(" ")) }"' "${keys[@]}")" | jq '.data |= [.[]?]'
