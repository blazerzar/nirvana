# shellcheck shell=bash
load_secrets() {
    local secrets
    secrets=$(infisical export --silent --path="/$1" --format=dotenv-export)
    eval "$secrets"
}

seed_summaries() {
    local actions=("Implement" "Write tests for" "Document" "Refactor" "Fix bugs in" "Add logging to" "Review" "Benchmark" "Improve error messages in" "Clean up")
    local subjects=("timer start command" "timer stop command" "connection list" "worklog publishing" "ticket search" "config loading" "credential storage" "SQLite migrations" "log rotation" "edit command")
    local action subject
    for action in "${actions[@]}"; do
        for subject in "${subjects[@]}"; do
            echo "$action $subject"
        done
    done
}
