#!/usr/bin/env bash
# Runs every scenario in linux/tests/scenarios against ENGINE (default: the debug build) and the data
# in SHANJIE_DATA (default: build/linux/data). Exits non-zero when any expected commit list differs.
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
engine="${1:-$root/build/linux/target/debug/ibus-engine-shanjie}"
export SHANJIE_DATA="${SHANJIE_DATA:-$root/build/linux/data}"
failed=0
for scenario in "$root"/linux/tests/scenarios/*.json; do
  name="$(basename "$scenario" .json)"
  if timeout 120 "$root/linux/tests/ibus-harness.sh" "$engine" "$scenario" >/dev/null 2>"$root/build/linux/scenario-$name.err"; then
    echo "ok   $name"
  else
    echo "FAIL $name ($(grep -m1 FAIL "$root/build/linux/scenario-$name.err" || echo 'see build/linux/scenario-'"$name"'.err'))"
    failed=1
  fi
done
exit $failed
