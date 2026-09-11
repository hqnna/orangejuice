#!/usr/bin/env bash
#
# Regenerates one module's bindings. Run inside `nix develop`:
#
#     tools/cbind/generate.sh posix
#     tools/cbind/generate.sh socket
#     tools/cbind/generate.sh linux
#     tools/cbind/generate.sh lz4    # needs lz4's headers; see README
#
# The wanted names come from `<module>/wanted.txt`, which is the surface the
# module is meant to export.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
module="${1:?which module}"
work="${TMPDIR:-/tmp}/cbind-$module"
mkdir -p "$work"

case "$module" in
  posix)  target=$root/modules/POSIX/generated.jai  ; lib=libc ;;
  socket) target=$root/modules/Socket/generated.jai ; lib=libc ;;
  linux)  target=$root/modules/Linux/generated.jai  ; lib=libc ;;
  lz4)    target=$root/modules/lz4/generated.jai    ; lib=lz4lib ;;
  *) echo "unknown module: $module" >&2; exit 2 ;;
esac

for source in main structs enums; do
  out=$work/$(echo "$source" | sed 's/^main$/cbind/;s/^structs$/cstructs/;s/^enums$/cenums/')
  rustc -O -o "$out" "$here/src/$source.rs"
done

clang ${CBIND_CFLAGS:-} -Xclang -ast-print -fsyntax-only "$here/$module/headers.c" > "$work/ast.txt" 2>/dev/null
clang ${CBIND_CFLAGS:-} -dM -E "$here/$module/headers.c" 2>/dev/null | sed 's/^#define //' > "$work/macros.txt"

echo "ast $(wc -l < "$work/ast.txt") lines, macros $(wc -l < "$work/macros.txt")"
echo "Now feed the wanted names to $work/{cbind,cstructs,cenums}; see README.md."
