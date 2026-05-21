#!/bin/bash

source .env

echo $JIRA_BASE_URL
echo $JIRA_USER_EMAIL
echo $JIRA_CLOUD_API

curl --request GET \
  --url "${JIRA_BASE_URL}/rest/api/2/issue/ME-498?fields=summary" \
  --header "Authorization: Bearer ${JIRA_CLOUD_API}" \
  --header "Accept: application/json" | jq
