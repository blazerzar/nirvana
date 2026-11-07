# shellcheck shell=bash
load_secrets() {
    local secrets
    secrets=$(infisical export --silent --path="/$1" --format=dotenv-export)
    eval "$secrets"
}
