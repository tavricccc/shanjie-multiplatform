#!/usr/bin/env bash
# Installs a built Shanjie tree: the release archive, or what linux/scripts/install.sh builds.
#   ./install.sh                      install with sudo, register for this user
#   PREFIX=/usr/local ./install.sh
# System files: $PREFIX/libexec/ibus-engine-shanjie, $PREFIX/bin/shanjie-settings, its desktop entry,
# $PREFIX/share/shanjie/, the IBus component (always /usr/share/ibus/component, where ibus-daemon
# reads components). The user's GNOME input sources gain ('ibus', 'shanjie') at the end; the current
# source is not changed.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-/usr}"
SUDO="${SUDO:-sudo}"
version="$(cat "$here/VERSION")"
[ "$(id -u)" -ne 0 ] || { echo "run as your user; the script asks sudo for the system part" >&2; exit 1; }

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
sed -e "s|@PREFIX@|$PREFIX|g" -e "s|@VERSION@|$version|g" "$here/templates/shanjie.xml.in" > "$stage/shanjie.xml"
sed -e "s|@PREFIX@|$PREFIX|g" "$here/templates/io.github.Nanako0129.Shanjie.Settings.desktop" > "$stage/settings.desktop"

$SUDO bash -euo pipefail -s "$here/files" "$PREFIX" "$stage" <<'SYSTEM'
files="$1"; prefix="$2"; stage="$3"
install -Dm755 "$files/libexec/ibus-engine-shanjie" "$prefix/libexec/ibus-engine-shanjie"
install -Dm755 "$files/bin/shanjie-settings" "$prefix/bin/shanjie-settings"
install -Dm644 "$stage/settings.desktop" "$prefix/share/applications/io.github.Nanako0129.Shanjie.Settings.desktop"
# Replace the data folder whole, so a removed file never lingers from an older version.
rm -rf "$prefix/share/shanjie/data.new"
mkdir -p "$prefix/share/shanjie"
cp -r "$files/share/shanjie/data" "$prefix/share/shanjie/data.new"
rm -rf "$prefix/share/shanjie/data"
mv "$prefix/share/shanjie/data.new" "$prefix/share/shanjie/data"
install -Dm644 "$files/share/shanjie/icons/shanjie.svg" "$prefix/share/shanjie/icons/shanjie.svg"
chmod -R a+rX "$prefix/share/shanjie"
install -Dm644 "$stage/shanjie.xml" /usr/share/ibus/component/shanjie.xml
SYSTEM

# Registration for this user: refresh the IBus registry and offer the engine in GNOME's sources.
ibus write-cache >/dev/null 2>&1 || true
ibus restart >/dev/null 2>&1 || true
if command -v gsettings >/dev/null; then
  current="$(gsettings get org.gnome.desktop.input-sources sources)"
  if [[ "$current" != *"'ibus', 'shanjie'"* ]]; then
    updated="$(python3 -I -c 'import ast,sys; s=ast.literal_eval(sys.argv[1].replace("@a(ss) ","")); s.append(("ibus","shanjie")); print(repr(s))' "$current")"
    gsettings set org.gnome.desktop.input-sources sources "$updated"
  fi
fi
echo "installed Shanjie $version to $PREFIX; choose 善解注音 from the input source menu"
