#!/bin/sh
set -eu

DATA_HOME=${XDG_DATA_HOME:-"$HOME/.local/share"}
CONFIG_HOME=${XDG_CONFIG_HOME:-"$HOME/.config"}
PREFIX=${MOONLIGHT_INSTALL_DIR:-"$DATA_HOME/moonlight-clock"}
BIN_DIR=${MOONLIGHT_BIN_DIR:-"$HOME/.local/bin"}

case "$PREFIX" in
    ""|/|"$HOME"|"$HOME/.local"|"$DATA_HOME")
        echo "Refusing unsafe installation path: $PREFIX" >&2
        exit 1
        ;;
esac

if [ -x "$PREFIX/bin/moonlight-clock" ]; then
    "$PREFIX/bin/moonlight-clock" stop || true
elif [ -x "$PREFIX/stop.sh" ]; then
    "$PREFIX/stop.sh" || true
fi
for command in moonlight-clock moonlight-clockctl; do
    link="$BIN_DIR/$command"
    if [ -L "$link" ] && [ "$(readlink "$link")" = "$PREFIX/bin/moonlight-clock" ]; then
        rm -f "$link"
    fi
done
rm -f "$CONFIG_HOME/autostart/io.github.moonlight-clock.desktop"
rm -rf "$PREFIX"

echo "Uninstalled Moonlight Clock"
echo "User configuration remains in ${XDG_CONFIG_HOME:-$HOME/.config}/moonlight-clock"
