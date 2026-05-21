#!/bin/bash

source .env

curl --request POST \
    --url "${JIRA_BASE_URL}/rest/api/2/issue/ME-498/worklog" \
    --header 'Accept: application/json' \
    --header 'Content-Type: application/json' \
    --header "Authorization: Bearer ${JIRA_DC_PAT}" \
    --data '{
        "started": "2026-05-01T09:00:00.000+0000",
        "timeSpent": "1h 30m",
        "comment": "I did some work here."
    }' | jq
