#!/usr/bin/env bash
# Builds the Linux engine, settings app and data into an installable tree:
#   linux/scripts/build.sh DEST   DEST/install.sh then installs it (see linux/package/install.sh)
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
dest="$1"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/linux/ibus/Cargo.toml" | head -1)"

pwsh -NoProfile -File "$root/scripts/prepare-core.ps1"
pwsh -NoProfile -File "$root/linux/scripts/prepare-data.ps1"
for crate in ibus settings; do
  cargo build --release --locked --manifest-path "$root/linux/$crate/Cargo.toml" --target-dir "$root/build/linux/target"
done

mkdir -p "$dest/files/libexec" "$dest/files/bin" "$dest/files/share/shanjie/icons" "$dest/templates"
install -m755 "$root/build/linux/target/release/ibus-engine-shanjie" "$dest/files/libexec/"
install -m755 "$root/build/linux/target/release/shanjie-settings" "$dest/files/bin/"
rm -rf "$dest/files/share/shanjie/data"
cp -r "$root/build/linux/data" "$dest/files/share/shanjie/data"
install -m644 "$root/assets/mode/shanjie.svg" "$dest/files/share/shanjie/icons/"
install -m644 "$root/linux/package/shanjie.xml.in" "$root/linux/package/io.github.Nanako0129.Shanjie.Settings.desktop" "$dest/templates/"
install -m755 "$root/linux/package/install.sh" "$root/linux/scripts/uninstall.sh" "$dest/"
echo "$version" > "$dest/VERSION"
