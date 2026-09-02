#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
TEST_HOME=$(mktemp -d "${TMPDIR:-/tmp}/moonlight-clock-install.XXXXXX")
export HOME="$TEST_HOME"
export XDG_DATA_HOME="$TEST_HOME/data"
export XDG_CONFIG_HOME="$TEST_HOME/config"
export XDG_STATE_HOME="$TEST_HOME/state"
export XDG_RUNTIME_DIR="$TEST_HOME/runtime"
PREFIX="$XDG_DATA_HOME/moonlight-clock"
export MOONLIGHT_BINARY=${MOONLIGHT_BINARY:-"$ROOT/target/debug/moonlight-clock"}

cleanup() {
    if [ -x "$PREFIX/bin/moonlight-clock" ]; then
        "$PREFIX/bin/moonlight-clock" stop >/dev/null 2>&1 || true
    fi
    rm -rf "$TEST_HOME"
}
trap cleanup EXIT HUP INT TERM

mkdir -p "$PREFIX"
: >"$PREFIX/start.sh"
: >"$PREFIX/stop.sh"
"$ROOT/install.sh" >/dev/null
test -L "$HOME/.local/bin/moonlight-clock"
test -L "$HOME/.local/bin/moonlight-clockctl"
test -x "$PREFIX/bin/moonlight-clock"
test ! -e "$PREFIX/start.sh"
test ! -e "$PREFIX/stop.sh"
AUTOSTART_FILE="$XDG_CONFIG_HOME/autostart/io.github.moonlight-clock.desktop"
test -f "$AUTOSTART_FILE"
grep -Fqx 'Type=Application' "$AUTOSTART_FILE"
grep -Fqx "Exec=\"$PREFIX/bin/moonlight-clock\" run" "$AUTOSTART_FILE"
grep -Fqx "TryExec=$PREFIX/bin/moonlight-clock" "$AUTOSTART_FILE"
grep -Fqx 'OnlyShowIn=COSMIC;' "$AUTOSTART_FILE"
grep -Fqx 'X-GNOME-Autostart-enabled=true' "$AUTOSTART_FILE"
test "$(grep -c '^Exec=' "$AUTOSTART_FILE")" = 1

test -f "$XDG_CONFIG_HOME/moonlight-clock/config.toml"
test "$(stat -c '%a' "$XDG_CONFIG_HOME/moonlight-clock/config.toml")" = 600
grep -q '^# Select the Wayland output that owns the widget' "$XDG_CONFIG_HOME/moonlight-clock/config.toml"
grep -q '^# Set the refresh interval in minutes' "$XDG_CONFIG_HOME/moonlight-clock/config.toml"
"$PREFIX/bin/moonlight-clock" --version | grep -q '^moonlight-clock '
"$ROOT/install.sh" >/dev/null
test -f "$PREFIX/assets/preview.png"
test ! -e "$PREFIX/assets/assets"
test "$(grep -c '^Exec=' "$AUTOSTART_FILE")" = 1
"$ROOT/uninstall.sh" >/dev/null

test ! -e "$PREFIX"
test ! -e "$HOME/.local/bin/moonlight-clock"
test ! -L "$HOME/.local/bin/moonlight-clock"
test ! -e "$HOME/.local/bin/moonlight-clockctl"
test ! -L "$HOME/.local/bin/moonlight-clockctl"
test ! -e "$XDG_CONFIG_HOME/autostart/io.github.moonlight-clock.desktop"
