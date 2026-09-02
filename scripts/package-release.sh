#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BINARY=${1:-"$ROOT/target/release/moonlight-clock"}
OUTPUT_DIR=${OUTPUT_DIR:-"$ROOT/dist"}

if [ ! -x "$BINARY" ]; then
    echo "Native binary not found or not executable: $BINARY" >&2
    exit 1
fi

VERSION=$("$BINARY" --version | awk 'NR == 1 { print $2 }')
case "$VERSION" in
    ''|*[!0-9A-Za-z.+-]*)
        echo "Cannot determine a valid version from $BINARY" >&2
        exit 1
        ;;
esac

MANIFEST_VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)
if [ "$VERSION" != "$MANIFEST_VERSION" ]; then
    echo "Binary version $VERSION does not match Cargo.toml version $MANIFEST_VERSION" >&2
    exit 1
fi

case "$(uname -m)" in
    x86_64) ARCH=x86_64-linux-gnu ;;
    aarch64|arm64) ARCH=aarch64-linux-gnu ;;
    *)
        echo "Unsupported release architecture: $(uname -m)" >&2
        exit 1
        ;;
esac

NAME="moonlight-clock-cosmic-$VERSION-$ARCH"
STAGING=$(mktemp -d "${TMPDIR:-/tmp}/moonlight-clock-release.XXXXXX")
trap 'rm -rf "$STAGING"' EXIT HUP INT TERM
mkdir -p "$STAGING/$NAME"

cp "$BINARY" "$STAGING/$NAME/moonlight-clock"
chmod 755 "$STAGING/$NAME/moonlight-clock"
for item in install.sh uninstall.sh config.example.toml README.md LICENSE; do
    cp "$ROOT/$item" "$STAGING/$NAME/$item"
done
cp -a "$ROOT/assets" "$STAGING/$NAME/assets"

mkdir -p "$OUTPUT_DIR"
tar -C "$STAGING" -czf "$OUTPUT_DIR/$NAME.tar.gz" "$NAME"
(CDPATH= cd -- "$OUTPUT_DIR" && sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256")
printf '%s\n' "$OUTPUT_DIR/$NAME.tar.gz"
