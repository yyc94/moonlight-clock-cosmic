#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
DATA_HOME=${XDG_DATA_HOME:-"$HOME/.local/share"}
CONFIG_HOME=${XDG_CONFIG_HOME:-"$HOME/.config"}
PREFIX=${MOONLIGHT_INSTALL_DIR:-"$DATA_HOME/moonlight-clock"}
BIN_DIR=${MOONLIGHT_BIN_DIR:-"$HOME/.local/bin"}
AUTOSTART_DIR="$CONFIG_HOME/autostart"
DESKTOP_FILE="$AUTOSTART_DIR/io.github.moonlight-clock.desktop"

case "$PREFIX" in
    ""|/|"$HOME"|"$HOME/.local"|"$DATA_HOME")
        echo "Refusing unsafe installation path: $PREFIX" >&2
        exit 1
        ;;
esac

if [ -n "${MOONLIGHT_BINARY:-}" ]; then
    SOURCE_BINARY=$MOONLIGHT_BINARY
elif [ -x "$ROOT/moonlight-clock" ]; then
    SOURCE_BINARY=$ROOT/moonlight-clock
elif command -v cargo >/dev/null 2>&1 && [ -f "$ROOT/Cargo.toml" ]; then
    cargo build --locked --release --manifest-path "$ROOT/Cargo.toml"
    SOURCE_BINARY=$ROOT/target/release/moonlight-clock
elif [ -x "$ROOT/target/release/moonlight-clock" ]; then
    SOURCE_BINARY=$ROOT/target/release/moonlight-clock
else
    echo "No native Moonlight Clock binary found; install from a release archive or install Rust 1.93+" >&2
    exit 1
fi

if [ ! -x "$SOURCE_BINARY" ]; then
    echo "Moonlight Clock binary is not executable: $SOURCE_BINARY" >&2
    exit 1
fi

if [ -x "$PREFIX/bin/moonlight-clock" ]; then
    "$PREFIX/bin/moonlight-clock" stop >/dev/null 2>&1 || true
fi

mkdir -p "$PREFIX/bin" "$BIN_DIR" "$AUTOSTART_DIR"
rm -f "$PREFIX/start.sh" "$PREFIX/stop.sh"
TEMP_BINARY="$PREFIX/bin/.moonlight-clock.$$"
trap 'rm -f "$TEMP_BINARY"' EXIT HUP INT TERM
cp "$SOURCE_BINARY" "$TEMP_BINARY"
chmod 755 "$TEMP_BINARY"
mv -f "$TEMP_BINARY" "$PREFIX/bin/moonlight-clock"
trap - EXIT HUP INT TERM

for item in config.example.toml README.md LICENSE uninstall.sh; do
    if [ "$ROOT/$item" != "$PREFIX/$item" ]; then
        cp -a "$ROOT/$item" "$PREFIX/$item"
    fi
done
if [ "$ROOT/assets" != "$PREFIX/assets" ]; then
    mkdir -p "$PREFIX/assets"
    cp -a "$ROOT/assets/." "$PREFIX/assets/"
fi

ln -sfn "$PREFIX/bin/moonlight-clock" "$BIN_DIR/moonlight-clock"
ln -sfn "$PREFIX/bin/moonlight-clock" "$BIN_DIR/moonlight-clockctl"

"$PREFIX/bin/moonlight-clock" init-config >/dev/null

{
    printf '%s\n' '[Desktop Entry]'
    printf '%s\n' 'Type=Application'
    printf '%s\n' 'Name=Moonlight Clock'
    printf '%s\n' 'Comment=Persona 3 inspired desktop widget for COSMIC'
    printf 'Exec="%s/bin/moonlight-clock" run\n' "$PREFIX"
    printf 'TryExec=%s/bin/moonlight-clock\n' "$PREFIX"
    printf '%s\n' 'Terminal=false'
    printf '%s\n' 'OnlyShowIn=COSMIC;'
    printf '%s\n' 'X-GNOME-Autostart-enabled=true'
} >"$DESKTOP_FILE"
chmod 644 "$DESKTOP_FILE"

echo "Installed Moonlight Clock in $PREFIX"
echo "COSMIC autostart is enabled for the next login."
