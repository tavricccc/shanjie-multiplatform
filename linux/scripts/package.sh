#!/usr/bin/env bash
# Builds the release archive build/linux/dist/shanjie-linux-<version>-<arch>.tar.gz and its .sha256.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/linux/ibus/Cargo.toml" | head -1)"
name="shanjie-linux-$version-$(uname -m)"
dist="$root/build/linux/dist"
if [[ "${1:-}" = --tested ]]; then
  payload="$(realpath "${2:?Usage: package.sh --tested PAYLOAD RECEIPT}")"
  receipt="$(realpath "${3:?Usage: package.sh --tested PAYLOAD RECEIPT}")"
else
  payload="$dist/$name"
  rm -rf "$payload"
  mkdir -p "$dist"
  "$root/linux/scripts/build.sh" "$payload"
  install -m644 "$root/linux/package/README.txt" "$payload/"
  bash "$root/linux/scripts/test.sh" "$payload"
  receipt="$root/build/linux/tested.json"
fi
python3 "$root/scripts/release_linux.py" package "$payload" "$receipt" --output "$dist"
