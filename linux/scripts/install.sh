#!/usr/bin/env bash
# Builds and installs the Linux engine and settings app from this checkout.
#   linux/scripts/install.sh            build as you, install with sudo, register for this user
#   PREFIX=/usr/local linux/scripts/install.sh
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[ "$(id -u)" -ne 0 ] || { echo "run as your user; the script asks sudo for the system part" >&2; exit 1; }
tree="$(mktemp -d)"
trap 'rm -rf "$tree"' EXIT
"$here/build.sh" "$tree"
"$tree/install.sh"
