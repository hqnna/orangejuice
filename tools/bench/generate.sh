#!/usr/bin/env bash
#
# Writes the benchmark programs into a directory. They are generated rather
# than checked in because what they measure is *scale* — a few thousand
# declarations — and a file that large is not worth reading or reviewing.
#
#     tools/bench/generate.sh /tmp/ojbench
#
# `wide`    many small procedures over many small structs: what a program with
#           a lot of surface looks like to the front end.
# `deep`    the same declarations reached through polymorphic containers, which
#           is what makes the checker instantiate.
# `modules` every module the distribution ships, imported at once.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
out="${1:?where to write the programs}"
mkdir -p "$out"

records="${RECORDS:-1200}"

{
  echo '#import "Basic";'
  echo
  for i in $(seq 1 "$records"); do
    cat <<EOF
Rec_$i :: struct { id: int; name: string; weight: float64; tags: [4] int; }
make_$i :: (id: int) -> Rec_$i { r: Rec_$i; r.id = id; r.weight = cast(float64) id; for 0..3  r.tags[it] = id * it; return r; }
score_$i :: (r: Rec_$i) -> float64 { total := r.weight; for r.tags  total += cast(float64) it; return total; }
sum_$i :: (n: int) -> float64 { total := 0.0; for 1..n  total += score_$i(make_$i(it)); return total; }
EOF
  done
  echo 'main :: () {'
  echo '    grand := 0.0;'
  for i in $(seq 1 "$records"); do echo "    grand += sum_$i(3);"; done
  echo '    print("%\n", grand);'
  echo '}'
} > "$out/wide.jai"

{
  echo '#import "Basic";'
  echo '#import "Hash_Table";'
  echo '#import "String";'
  echo
  for i in $(seq 1 $((records / 4))); do
    cat <<EOF
Key_$i :: struct { a: int; b: int; }
operator == :: (x: Key_$i, y: Key_$i) -> bool { return x.a == y.a && x.b == y.b; }
fill_$i :: (n: int) -> int {
    table: Table(string, int);
    init(*table);
    for 1..n  table_add(*table, tprint("k%-%", it, $i), it);
    return table.count;
}
EOF
  done
  echo 'main :: () {'
  echo '    total := 0;'
  for i in $(seq 1 $((records / 4))); do echo "    total += fill_$i(4);"; done
  echo '    print("%\n", total);'
  echo '}'
} > "$out/deep.jai"

{
  for module in $(cd "$root/modules" && find . -maxdepth 1 -name '*.jai' -printf '%f\n' | sed 's/\.jai$//' | sort); do
    case $module in
      Preload | Runtime_Support | Default_Metaprogram | Runtime_Support_Crash_Handler) ;;
      *) echo "#import \"$module\";" ;;
    esac
  done
  for module in $(cd "$root/modules" && find . -maxdepth 2 -name 'module.jai' -printf '%h\n' | sed 's|^\./||' | sort); do
    echo "#import \"$module\";"
  done
  echo 'main :: () { }'
} > "$out/modules.jai"

wc -l "$out"/*.jai
