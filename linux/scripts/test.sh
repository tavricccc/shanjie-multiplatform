#!/usr/bin/env bash
# Basic Linux release acceptance against the exact tree that package.sh will archive.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
payload="$(realpath "${1:?Usage: test.sh BUILT_PACKAGE_TREE}")"
report="$root/build/reports/linux-$(date -u +%Y%m%dT%H%M%SZ)-$$"
mkdir -p "$report/profile/config" "$report/profile/cache" "$report/profile/data" "$report/tmp"
export SHANJIE_TEST_PROFILE=1 SHANJIE_TEST_ROOT="$report/profile"
export XDG_CONFIG_HOME="$report/profile/config" XDG_CACHE_HOME="$report/profile/cache" XDG_DATA_HOME="$report/profile/data"
export GSETTINGS_BACKEND=memory TMPDIR="$report/tmp"

cmake -S "$root/linux/tests" -B "$root/build/cmake/linux" "-DSHANJIE_LINUX_PAYLOAD=$payload"
ctest --test-dir "$root/build/cmake/linux" -C Release --show-only=json-v1 > "$report/inventory.json"
ctest --test-dir "$root/build/cmake/linux" -C Release --parallel 1 --stop-on-failure --no-tests=error \
  --output-on-failure --output-log "$report/ctest.log" --output-junit "$report/junit.xml"
python3 "$root/scripts/release_linux.py" attest "$payload" "$root/build/linux/tested.json" --report "$report"
