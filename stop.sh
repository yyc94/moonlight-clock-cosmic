#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

if [ -x "$ROOT/bin/moonlight-clock" ]; then
    exec "$ROOT/bin/moonlight-clock" stop
fi
if [ -x "$ROOT/moonlight-clock" ]; then
    exec "$ROOT/moonlight-clock" stop
fi
if [ -x "$ROOT/target/release/moonlight-clock" ]; then
    exec "$ROOT/target/release/moonlight-clock" stop
fi
echo "Moonlight Clock native binary not found" >&2
exit 1
