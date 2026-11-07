#!/usr/bin/env bash
#MISE description="Show a Linear issue title"
#USAGE arg "<key>" help="Issue identifier, e.g. ENG-123"
set -euo pipefail
source "$MISE_PROJECT_ROOT/scripts/linear/lib.sh"

linear "query(\$id: String!) { issue(id: \$id) { identifier title } }" \
    "$(jq --null-input --arg id "${usage_key:?}" '{$id}')"
