#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUTPUT=${1:-"$ROOT/dist/docker"}

mkdir -p "$OUTPUT"
docker build --target export --output "type=local,dest=$OUTPUT" "$ROOT"
"$ROOT/scripts/package-release.sh" "$OUTPUT/moonlight-clock"
