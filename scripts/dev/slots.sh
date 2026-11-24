#!/usr/bin/env bash
#MISE description="Add about 8 hours of slots on each of the last few workdays, today included"
#USAGE flag "--days <days>" default="5" help="Number of days, today and the workdays before it"
#USAGE flag "--jira <jira>" default="jira-cloud" help="Jira connection name"
#USAGE flag "--linear <linear>" default="linear" help="Linear connection name"
#USAGE flag "--bin <bin>" default="cargo run --quiet --" help="Nirvana command, e.g. target/release/nirvana"
set -euo pipefail

read -ra bin <<<"${usage_bin:?}"
jira=${usage_jira:?}
linear=${usage_linear:?}
tickets=(
    "$jira DES-1" "$jira DES-2" "$jira DES-50" "$jira DES-100"
    "$jira PROD-1" "$jira PROD-2" "$jira PROD-50" "$jira PROD-100"
    "$jira NIR-1" "$jira NIR-30" "$jira NIR-67"
    "$linear NIR-3" "$linear NIR-50" "$linear NIR-102"
    "$linear BLE-1" "$linear BLE-5" "$linear BLE-30" "$linear BLE-104"
)
day_start=$((8 * 60))
day_length=$((8 * 60))

days_ago() {
    date -d "$1 days ago" "+$2" 2>/dev/null || date -v-"$1"d "+$2"
}

add_day() {
    local day=$1 limit=$2 minute duration connection key
    minute=$((day_start + RANDOM % 31))
    while ((minute < limit)); do
        duration=$((20 + RANDOM % 21 * 5))
        ((duration = limit - minute < duration ? limit - minute : duration))
        read -r connection key <<<"${tickets[RANDOM % ${#tickets[@]}]}"
        "${bin[@]}" add "$key" \
            --connection "$connection" \
            --start "$(printf '%s %02d:%02d' "$day" $((minute / 60)) $((minute % 60)))" \
            --duration "${duration}m"
        ((minute += duration + RANDOM % 4 * 5))
    done
}

now=$((10#$(date +%H) * 60 + 10#$(date +%M)))
end=$((day_start + day_length))
add_day "$(days_ago 0 %F)" $((now < end ? now : end))

days=1 ago=0
while ((days < ${usage_days:?})); do
    ((ago += 1))
    if (($(days_ago "$ago" %u) <= 5)); then
        add_day "$(days_ago "$ago" %F)" "$end"
        ((days += 1))
    fi
done
