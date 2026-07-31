#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "usage: $0 <driver-feature> <output.json>" >&2
    exit 2
fi

driver=$1
output=$2
shift 2

case "$driver" in
    sqlite | mysql | postgresql | dynamodb | turso) ;;
    *)
        echo "unsupported driver feature: $driver" >&2
        exit 2
        ;;
esac

# Serialize tests so coverage counters are deterministic.
set -- -- --test-threads=1

cargo llvm-cov \
    --json \
    --output-path "$output" \
    --ignore-filename-regex '(/tests/|/examples/|toasty-driver-integration-suite)' \
    --remap-path-prefix \
    --package tests \
    --no-default-features \
    --features "$driver" \
    --test "$driver" \
    --jobs 1 \
    --quiet \
    "$@"
