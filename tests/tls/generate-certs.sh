#!/bin/bash
set -euo pipefail

# Generates test PKI for TLS integration tests.
# Output goes to ./certs/ relative to this script.
# Certs are valid for 100 years so they never expire in CI.

DIR="$(cd "$(dirname "$0")/certs" && pwd)"

# CA
openssl req -new -x509 -nodes \
    -days 36500 \
    -keyout "$DIR/ca.key" \
    -out "$DIR/ca.crt" \
    -subj "/CN=Toasty Test CA"

# Server cert (SAN=localhost,127.0.0.1)
openssl req -new -nodes \
    -keyout "$DIR/server.key" \
    -out "$DIR/server.csr" \
    -subj "/CN=localhost"

openssl x509 -req \
    -in "$DIR/server.csr" \
    -CA "$DIR/ca.crt" \
    -CAkey "$DIR/ca.key" \
    -CAcreateserial \
    -days 36500 \
    -out "$DIR/server.crt" \
    -extfile <(printf "subjectAltName=DNS:localhost,IP:127.0.0.1")

# Client cert
openssl req -new -nodes \
    -keyout "$DIR/client.key" \
    -out "$DIR/client.csr" \
    -subj "/CN=toasty"

openssl x509 -req \
    -in "$DIR/client.csr" \
    -CA "$DIR/ca.crt" \
    -CAkey "$DIR/ca.key" \
    -CAcreateserial \
    -days 36500 \
    -out "$DIR/client.crt"

# Cleanup CSRs and serial files
rm -f "$DIR"/*.csr "$DIR"/*.srl

echo "Generated test certificates in $DIR"
