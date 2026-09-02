#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

if [ -x "$ROOT/bin/moonlight-clock" ]; then
    exec "$ROOT/bin/moonlight-clock" start
fi
if [ -x "$ROOT/moonlight-clock" ]; then
    exec "$ROOT/moonlight-clock" start
fi
if command -v cargo >/dev/null 2>&1 && [ -f "$ROOT/Cargo.toml" ]; then
    exec cargo run --locked --manifest-path "$ROOT/Cargo.toml" -- start
fi
if [ -x "$ROOT/target/release/moonlight-clock" ]; then
    exec "$ROOT/target/release/moonlight-clock" start
fi
echo "Moonlight Clock native binary not found; run ./install.sh first" >&2
exit 1
