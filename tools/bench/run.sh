#!/usr/bin/env bash
#
# Times a build of each generated benchmark, best of N, and prints the stage
# breakdown for the largest. Run inside `nix develop`:
#
#     tools/bench/generate.sh /tmp/ojbench
#     tools/bench/run.sh /tmp/ojbench
#
# `oj` is taken from `target/release` unless `OJ` names another one, so two
# builds can be compared by pointing it at each in turn.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
dir="${1:?where the programs were written}"
oj="${OJ:-$root/target/release/oj}"
rounds="${ROUNDS:-4}"

best() {
  local lowest=9999999
  for _ in $(seq 1 "$rounds"); do
    local start end
    start=$(date +%s%3N)
    "$oj" "$@" >/dev/null 2>&1 || true
    end=$(date +%s%3N)
    local took=$((end - start))
    [ "$took" -lt "$lowest" ] && lowest=$took
  done
  echo "$lowest"
}

printf '%-12s %10s %10s %10s\n' program lines metaprogram direct
for program in wide deep modules; do
  file="$dir/$program.jai"
  [ -f "$file" ] || continue
  printf '%-12s %10s %8sms %8sms\n' \
    "$program" "$(wc -l < "$file")" \
    "$(best "$file")" "$(best "$file" -- no_metaprogram)"
done

echo
echo "stages, largest program, no metaprogram:"
OJ_TIMING=1 "$oj" "$dir/wide.jai" -- no_metaprogram 2>&1 >/dev/null | grep -v build_module
