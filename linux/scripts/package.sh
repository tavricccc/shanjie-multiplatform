#!/usr/bin/env bash
# Builds the release archive build/linux/dist/shanjie-linux-<version>-<arch>.tar.gz and its .sha256.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/linux/ibus/Cargo.toml" | head -1)"
name="shanjie-linux-$version-$(uname -m)"
dist="$root/build/linux/dist"
rm -rf "$dist/$name"
mkdir -p "$dist"
"$root/linux/scripts/build.sh" "$dist/$name"
install -m644 "$root/linux/package/README.txt" "$dist/$name/"
tar -C "$dist" -czf "$dist/$name.tar.gz" "$name"
(cd "$dist" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
echo "$dist/$name.tar.gz"
