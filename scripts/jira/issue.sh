#!/usr/bin/env bash
#MISE description="Show a Jira issue summary"
#USAGE arg "<key>" help="Issue key, e.g. ME-498"
#USAGE flag "--dc" help="Use Jira Data Center instead of Jira Cloud"
set -euo pipefail
source "$MISE_PROJECT_ROOT/scripts/jira/lib.sh"

jira GET "issue/${usage_key:?}?fields=summary"
