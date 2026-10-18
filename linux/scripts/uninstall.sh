#!/usr/bin/env bash
# Removes what install.sh installed. The user's folder (~/.local/share/shanjie: settings, personal
# dictionary, learning) is kept unless --delete-user-data is given, and then it is moved aside to
# ~/.local/share/shanjie.deleted_files/<time> rather than erased.
set -euo pipefail
PREFIX="${PREFIX:-/usr}"
SUDO="${SUDO:-sudo}"
[ "$(id -u)" -ne 0 ] || { echo "run as your user; the script asks sudo for the system part" >&2; exit 1; }
if command -v gsettings >/dev/null; then
  current="$(gsettings get org.gnome.desktop.input-sources sources)"
  updated="$(python3 -I -c 'import ast,sys; s=ast.literal_eval(sys.argv[1].replace("@a(ss) ","")); s=[x for x in s if x!=("ibus","shanjie")]; print(repr(s) if s else "@a(ss) []")' "$current")"
  gsettings set org.gnome.desktop.input-sources sources "$updated"
fi
$SUDO rm -f "$PREFIX/libexec/ibus-engine-shanjie" "$PREFIX/bin/shanjie-settings" "$PREFIX/share/applications/io.github.Nanako0129.Shanjie.Settings.desktop" /usr/share/ibus/component/shanjie.xml
$SUDO rm -rf "$PREFIX/share/shanjie"
ibus write-cache >/dev/null 2>&1 || true
ibus restart >/dev/null 2>&1 || true
if [ "${1:-}" = "--delete-user-data" ]; then
  data="${XDG_DATA_HOME:-$HOME/.local/share}"
  if [ -d "$data/shanjie" ]; then
    mkdir -p "$data/shanjie.deleted_files"
    mv "$data/shanjie" "$data/shanjie.deleted_files/$(date +%Y%m%d-%H%M%S)"
  fi
fi
echo "removed Shanjie from $PREFIX"
