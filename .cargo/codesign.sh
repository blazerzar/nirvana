#!/usr/bin/env bash
# Cargo runner on macOS. Signs binaries with a stable self-signed identity,
# so the keychain keeps "Always Allow" across rebuilds.
set -euo pipefail

identity="Nirvana Dev"
identifier="nirvana"

create_identity() {
    local dir
    dir=$(mktemp -d)
    trap 'rm -rf "$dir"' RETURN
    cat >"$dir/openssl.cnf" <<EOF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $identity
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
EOF
    /usr/bin/openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
        -config "$dir/openssl.cnf" -keyout "$dir/key.pem" -out "$dir/cert.pem" 2>/dev/null
    /usr/bin/openssl pkcs12 -export -inkey "$dir/key.pem" -in "$dir/cert.pem" \
        -passout pass:nirvana -out "$dir/identity.p12"
    security import "$dir/identity.p12" -P nirvana -T /usr/bin/codesign >/dev/null
}

if ! security find-identity -p codesigning | grep -qF "\"$identity\""; then
    create_identity
fi

if ! codesign -dv "$1" 2>&1 | grep -qxF "Authority=$identity"; then
    codesign --force --sign "$identity" --identifier "$identifier" "$1" \
        2> >(grep -vF "replacing existing signature" >&2)
fi

exec "$@"
