#!/usr/bin/env bash
#MISE description="Add a worklog to a Jira issue"
#USAGE arg "<key>" help="Issue key, e.g. ME-498"
#USAGE arg "<time_spent>" help="Time spent, e.g. \"1h 30m\""
#USAGE flag "--started <started>" help="Start time, e.g. 2026-05-01T09:00:00.000+0000 (default: now)"
#USAGE flag "--comment <comment>" help="Worklog comment"
#USAGE flag "--dc" help="Use Jira Data Center instead of Jira Cloud"
set -euo pipefail
source "$MISE_PROJECT_ROOT/scripts/jira/lib.sh"

body=$(jq --null-input \
    --arg started "${usage_started:-$(date +%Y-%m-%dT%H:%M:%S.000%z)}" \
    --arg timeSpent "${usage_time_spent:?}" \
    --arg comment "${usage_comment:-}" \
    '{$started, $timeSpent} + if $comment == "" then {} else {$comment} end')

jira POST "issue/${usage_key:?}/worklog" --data "$body"
