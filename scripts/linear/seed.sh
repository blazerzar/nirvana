#!/usr/bin/env bash
#MISE description="Create 100 issues in a Linear team"
#USAGE arg "<team>" help="Team key, e.g. ENG"
set -euo pipefail
source "$MISE_PROJECT_ROOT/scripts/linear/lib.sh"

team_id=$(linear "query(\$key: String!) { teams(filter: {key: {eq: \$key}}) { nodes { id } } }" \
    "$(jq --null-input --arg key "${usage_team:?}" '{$key}')" |
    jq -r '.data.teams.nodes[0].id // error("team not found")')

seed_summaries | jq -R -s -r --arg team "$team_id" '
    split("\n") | map(select(length > 0)) | . as $all
    | range(0; length; 50)
    | $all[.:. + 50] | to_entries
    | map("c\(.key): issueCreate(input: {teamId: \($team | tojson), title: \(.value | tojson)}) { issue { identifier } }")
    | "mutation { \(join(" ")) }"' |
    while read -r mutation; do
        linear "$mutation" | jq -r '.data[] | "created \(.issue.identifier)"'
    done
