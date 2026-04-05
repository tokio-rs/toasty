#!/bin/sh
# Prints the toasty minor version (e.g. "0.3") from the workspace Cargo.toml.
# Used by mdbook-cmdrun to inject the version into guide pages.
sed -n 's/^version = "\([0-9]*\.[0-9]*\).*/\1/p' "$(dirname "$0")/../../crates/toasty/Cargo.toml"
