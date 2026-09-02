#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

cargo fmt --manifest-path "$ROOT/Cargo.toml" -- --check
cargo test --locked --manifest-path "$ROOT/Cargo.toml"
cargo clippy --locked --all-targets --manifest-path "$ROOT/Cargo.toml" -- -D warnings
cargo build --locked --manifest-path "$ROOT/Cargo.toml"
for script in "$ROOT"/*.sh "$ROOT"/scripts/*.sh "$ROOT"/tests/*.sh; do
    sh -n "$script"
done
"$ROOT/tests/test_install.sh"
