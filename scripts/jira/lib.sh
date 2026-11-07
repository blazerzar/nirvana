# shellcheck shell=bash
source "$MISE_PROJECT_ROOT/scripts/lib.sh"

if [[ ${usage_dc:-} == true ]]; then
    load_secrets jira-dc
    auth=(--header "Authorization: Bearer $PAT")
else
    load_secrets jira-cloud
    auth=(--user "$EMAIL:$API_TOKEN")
fi

jira() {
    local method=$1 path=$2
    shift 2
    curl --silent --show-error --fail-with-body --request "$method" \
        --url "https://$HOST/rest/api/2/$path" \
        "${auth[@]}" \
        --header 'Accept: application/json' \
        --header 'Content-Type: application/json' \
        "$@" | jq
}
