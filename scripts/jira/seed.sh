#!/usr/bin/env bash
#MISE description="Create 100 issues in a Jira project"
#USAGE arg "<project>" help="Project key, e.g. ME"
#USAGE flag "--dc" help="Use Jira Data Center instead of Jira Cloud"
set -euo pipefail
source "$MISE_PROJECT_ROOT/scripts/jira/lib.sh"

project=${usage_project:?}

issue_type=$(jira GET "project/$project" | jq -r '
    [.issueTypes[] | select(.subtask == false and (.hierarchyLevel // 0) == 0)]
    | (map(select(.name == "Task")) + .)[0].id')

seed_summaries | jq -R -s -c --arg project "$project" --arg type "$issue_type" '
    split("\n") | map(select(length > 0)) | . as $all
    | range(0; length; 50)
    | {issueUpdates: $all[.:. + 50] | map({
        fields: {project: {key: $project}, issuetype: {id: $type}, summary: .}
    })}' |
    while read -r body; do
        jira POST issue/bulk --data "$body" |
            jq -r '(.issues[] | "created \(.key)"), (.errors[] | "failed: \(.elementErrors)")'
    done
