#!/usr/bin/env bash
# Runs ibus_client.py against a private ibus-daemon on a private session bus, so a test never
# touches the desktop's IBus, its address file, its settings or its engine choice.
#   linux/tests/ibus-harness.sh ENGINE_BINARY SCRIPT.json
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
engine="$(realpath "$1")"; script="$(realpath "$2")"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/shanjie-ibus.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/config" "$tmp/cache" "$tmp/data"
exec dbus-run-session -- env -u DISPLAY WAYLAND_DISPLAY="shanjie-test-$$" GSETTINGS_BACKEND=memory \
  XDG_CONFIG_HOME="$tmp/config" XDG_CACHE_HOME="$tmp/cache" XDG_DATA_HOME="$tmp/data" \
  IBUS_ADDRESS="unix:path=$tmp/ibus.sock" SHANJIE_TEST_HOME="$tmp" SHANJIE_DAEMON_LOG="${SHANJIE_DAEMON_LOG:-}" \
  python3 -I "$here/ibus_client.py" "$engine" "$script"
