#!/usr/bin/env bash
#MISE description="Open the GUI in each error state, one after another"
#USAGE arg "[case]" help="Run only this case (default: all)" {
#USAGE   choices "garbage-db" "db-is-dir" "bad-config" "connections-down" "logs-unopenable"
#USAGE }
#USAGE flag "--bin <bin>" default="cargo run --quiet --" help="Nirvana command, e.g. target/release/nirvana"
set -euo pipefail

read -ra bin <<<"${usage_bin:?}"
root=$(mktemp -d)
trap 'chmod -R u+w "$root"; rm -rf "$root"' EXIT

nirvana() {
    local dir=$root/$1
    shift
    NIRVANA_DEV=0 NIRVANA_CONFIG_DIR=$dir NIRVANA_DATA_DIR=$dir NIRVANA_LOG_DIR=$dir/logs "${bin[@]}" "$@"
}

explain() {
    printf '\n\033[1m%s\033[0m\n' "$1"
    shift
    printf '  - %s\n' "$@" "close the window to continue"
}

garbage_db() {
    mkdir -p "$root/garbage-db"
    echo garbage >"$root/garbage-db/nirvana.db"
    explain "garbage-db: the database file is not SQLite" \
        "error toast \"database error: ... file is not a database\" in the main window" \
        "click the logo 5 times: the info window shows the same error instead of the rows"
    nirvana garbage-db
}

db_is_dir() {
    mkdir -p "$root/db-is-dir/nirvana.db"
    explain "db-is-dir: the database path is a directory" \
        "error toast \"database error: ... unable to open database file\" in the main window"
    nirvana db-is-dir
}

bad_config() {
    mkdir -p "$root/bad-config"
    echo 'not toml [' >"$root/bad-config/config.toml"
    explain "bad-config: config.toml is not valid TOML" \
        "\"couldn't load the profile\" error toast with \"config unavailable\" as detail" \
        "no profile chip in the header"
    nirvana bad-config
}

connections_down() {
    nirvana connections-down profile create work --switch
    nirvana connections-down connection add unreachable \
        --kind jira-dc \
        --host https://jira.invalid \
        --credential token \
        --storage plaintext
    explain "connections-down: the only connection points to an unreachable host" \
        "profile chip \"work\" starts with a grey dot while the connection is tested" \
        "the dot turns red once the test fails"
    nirvana connections-down
}

logs_unopenable() {
    mkdir -p "$root/logs-unopenable/logs"
    chmod a-w "$root/logs-unopenable/logs"
    explain "logs-unopenable: the log file cannot be created, so it does not exist" \
        "the terminal prints \"logging disabled\", which is expected" \
        "click the logo 5 times, then click Show logs" \
        "\"couldn't open the log file\" error toast with the missing path as detail"
    nirvana logs-unopenable
}

if [[ -n ${usage_case:-} ]]; then
    cases=("$usage_case")
else
    cases=(garbage-db db-is-dir bad-config connections-down logs-unopenable)
fi
for case in "${cases[@]}"; do
    "${case//-/_}"
done
