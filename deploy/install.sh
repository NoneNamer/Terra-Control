#!/bin/sh
# Installs or updates the terrarium controller on the Raspberry Pi.
#
# Copy these items into one folder on the Pi (e.g. ~/terrarium) and run
#     sudo sh ~/terrarium/deploy/install.sh
#   terra-control   (from dist/ after `docker build --output dist .`)
#   config.toml
#   static/
#   deploy/         (this script and terrarium.service)
#   readme.md       (optional)
#
# An existing /opt/terrarium/config.toml and the database are never overwritten.

set -eu

SRC=$(cd "$(dirname "$0")/.." && pwd)
DEST=/opt/terrarium
SERVICE=terrarium.service

if [ "$(id -u)" -ne 0 ]; then
    echo "Please run as root: sudo sh $0" >&2
    exit 1
fi

for item in terra-control config.toml static deploy/$SERVICE; do
    if [ ! -e "$SRC/$item" ]; then
        echo "Missing $SRC/$item" >&2
        exit 1
    fi
done

case "$(uname -m)" in
    aarch64) ;;
    *) echo "Warning: this machine is $(uname -m), the binary is built for aarch64 (64-bit Pi OS)" >&2 ;;
esac

# Stop a running instance first: it switches all outputs off and stores the UV lamp state.
# (The binary can't be replaced while it is running.)
if systemctl is-active --quiet "$SERVICE"; then
    echo "Stopping $SERVICE ..."
    systemctl stop "$SERVICE"
fi

mkdir -p "$DEST"
install -m 755 "$SRC/terra-control" "$DEST/terra-control"
rm -rf "$DEST/static"
cp -r "$SRC/static" "$DEST/static"
if [ -f "$SRC/readme.md" ]; then
    install -m 644 "$SRC/readme.md" "$DEST/readme.md"
fi

if [ -f "$DEST/config.toml" ]; then
    if ! cmp -s "$SRC/config.toml" "$DEST/config.toml"; then
        install -m 644 "$SRC/config.toml" "$DEST/config.toml.new"
        echo "Kept the existing $DEST/config.toml."
        echo "  The shipped template differs and was saved as config.toml.new."
        echo "  If the program refuses to start, compare: diff $DEST/config.toml $DEST/config.toml.new"
    fi
else
    install -m 644 "$SRC/config.toml" "$DEST/config.toml"
    echo "Installed $DEST/config.toml - check the sensor IDs, relays and [hardware] simulate."
fi

if grep -q '^simulate *= *true' "$DEST/config.toml"; then
    echo "Note: $DEST/config.toml has simulate = true - the real hardware is NOT used."
fi
if grep -q '^setup_mode *= *true' "$DEST/config.toml"; then
    echo "Note: setup_mode = true - the schedule is paused, lamps are only switched manually."
fi

install -m 644 "$SRC/deploy/$SERVICE" "/etc/systemd/system/$SERVICE"
systemctl daemon-reload
systemctl enable "$SERVICE"
systemctl restart "$SERVICE"

sleep 3
if systemctl is-active --quiet "$SERVICE"; then
    echo "$SERVICE is running. Logs: journalctl -u $SERVICE -f"
else
    echo "$SERVICE did not start. Last log lines:" >&2
    journalctl -u "$SERVICE" -n 30 --no-pager >&2 || true
    exit 1
fi
