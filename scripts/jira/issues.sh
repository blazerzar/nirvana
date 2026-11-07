#!/usr/bin/env bash
#MISE description="Show summaries of several Jira issues"
#USAGE arg "<keys>..." help="Issue keys, e.g. ME-498 ME-499"
#USAGE flag "--dc" help="Use Jira Data Center instead of Jira Cloud"
set -euo pipefail
source "$MISE_PROJECT_ROOT/scripts/jira/lib.sh"

keys=${usage_keys:?}

if [[ ${usage_dc:-} == true ]]; then
    path=search
else
    path=search/jql
fi

jira GET "$path" --get \
    --data-urlencode "jql=key in (${keys// /,})" \
    --data-urlencode fields=summary
