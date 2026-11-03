# shellcheck shell=bash
source "$MISE_PROJECT_ROOT/scripts/lib.sh"

load_secrets linear

linear() {
    local query=$1 variables=${2:-'{}'}
    curl --silent --show-error --fail-with-body --request POST \
        --url https://api.linear.app/graphql \
        --header "Authorization: $API_KEY" \
        --header 'Content-Type: application/json' \
        --data "$(jq --null-input --arg query "$query" --argjson variables "$variables" '{$query, $variables}')" | jq
}
