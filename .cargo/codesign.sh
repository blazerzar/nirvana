#!/usr/bin/env bash
# Cargo runner on macOS. Signs binaries with an Apple Development identity, so
# the keychain partition list keeps access across rebuilds.
set -euo pipefail

if [[ -n "${CI:-}" ]]; then
    exec "$@"
fi

identity=$(security find-identity -v -p codesigning | awk '/"Apple Development: /{print $2; exit}')

if [[ -n "$identity" ]]; then
    codesign --force --sign "$identity" --identifier nirvana "$1" \
        2> >(grep -vF "replacing existing signature" >&2)
fi

exec "$@"
