#!/usr/bin/env bash
#
# Regenerates one module's bindings, end to end. Run inside `nix develop`:
#
#     tools/cbind/generate.sh posix
#     tools/cbind/generate.sh socket
#     tools/cbind/generate.sh linux
#     tools/cbind/generate.sh macos    # on a Mac; the other three want Linux
#     tools/cbind/generate.sh lz4      # needs lz4's headers; see README
#
# What comes out is written beside the module's file as `<name>.new`, and the
# diff against what is committed is printed. It is *not* moved into place:
# these bindings carry hand curation the generator cannot know about — see
# `README.md` — so the last step is a person reading the diff.
#
# `--write` moves it into place anyway, for when the diff has been read.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)

write=no
args=()
for arg in "$@"; do
  case "$arg" in
    --write) write=yes ;;
    *) args+=("$arg") ;;
  esac
done
module="${args[0]:?which module}"
work="${TMPDIR:-/tmp}/cbind-$module"
mkdir -p "$work"

# The glibc binding is the architecture's as well as the system's, so `posix`
# writes the file for the machine it is generated on — see `POSIX/linux.jai`.
case "$(uname -m)" in
  aarch64|arm64) arch=arm64 ;;
  *)             arch=x64 ;;
esac

case "$module" in
  posix)  target=$root/modules/POSIX/generated_$arch.jai ; lib=libc ;;
  macos)  target=$root/modules/POSIX/generated_macos.jai ; lib=libc ;;
  socket) target=$root/modules/Socket/generated.jai      ; lib=libc ;;
  linux)  target=$root/modules/Linux/generated.jai       ; lib=libc ;;
  lz4)    target=$root/modules/lz4/generated.jai         ; lib=lz4lib ;;
  *) echo "unknown module: $module" >&2; exit 2 ;;
esac
: "$lib"

for source in main structs enums consts; do
  out=$work/$(echo "$source" | sed 's/^main$/cbind/;s/^structs$/cstructs/;s/^enums$/cenums/;s/^consts$/cconsts/')
  rustc -O -o "$out" "$here/src/$source.rs"
done

clang ${CBIND_CFLAGS:-} -Xclang -ast-print -fsyntax-only "$here/$module/headers.c" > "$work/ast.txt" 2>/dev/null
clang ${CBIND_CFLAGS:-} -dM -E "$here/$module/headers.c" 2>/dev/null | sed 's/^#define //' > "$work/macros.txt"
echo "ast $(wc -l < "$work/ast.txt") lines, macros $(wc -l < "$work/macros.txt")"

wanted() { cat "$here/$module/wanted-$1.txt"; }

# The constants, by asking the compiler. A macro that does not compile — one
# that names a type, or an identifier the headers only declare behind a guard
# — is dropped and the program built again, until what is left compiles.
: > "$work/rejected.txt"
while : ; do
  wanted constants | "$work/cconsts" "$work/macros.txt" "$here/$module/headers.c" "$work/rejected.txt" \
    > "$work/constants.c" 2>"$work/constants.log"
  if clang ${CBIND_CFLAGS:-} -w -o "$work/constants" "$work/constants.c" 2>"$work/constants.err"; then
    break
  fi
  # Each `printf` is on a line of its own, so a diagnostic's line number names
  # the macro that caused it.
  before=$(wc -l < "$work/rejected.txt")
  sed -n 's/^[^:]*constants\.c:\([0-9]*\):.*$/\1/p' "$work/constants.err" | sort -un | while read -r line; do
    sed -n "${line}s|.*/\* \([A-Za-z_][A-Za-z_0-9]*\) \*/.*|\1|p" "$work/constants.c"
  done >> "$work/rejected.txt"
  sort -u -o "$work/rejected.txt" "$work/rejected.txt"
  if [ "$(wc -l < "$work/rejected.txt")" = "$before" ]; then
    echo "the constants program does not compile and no macro can be blamed:" >&2
    head -20 "$work/constants.err" >&2
    exit 1
  fi
done
"$work/constants" > "$work/constants.jai"
rejected=$(wc -l < "$work/rejected.txt")
[ "$rejected" = "0" ] || echo "dropped $rejected macro(s) the compiler would not take"

wanted types      | "$work/cstructs" "$work/ast.txt" > "$work/types.jai"  2>/dev/null
wanted enums      | "$work/cenums"   "$work/ast.txt" > "$work/enums.jai"  2>/dev/null
wanted procedures | "$work/cbind"    "$work/ast.txt" > "$work/procs.jai"  2>/dev/null

# A C struct and a C function may share a name; Jai has one namespace for both,
# so `renames.txt` says which keeps it. The emitters do not apply that — they
# each see one kind of declaration — so it is applied here, to every section.
renames=$here/$module/renames.txt
if [ -f "$renames" ]; then
  script=$work/renames.sed
  : > "$script"
  while read -r from to; do
    [ -z "${from:-}" ] && continue
    case "$from" in \#*) continue ;; esac
    printf 's/\\b%s\\b/%s/g\n' "$from" "$to" >> "$script"
  done < "$renames"
  # The *procedure* keeps its C name where the struct was the one renamed, so
  # the substitution is undone on the declaration itself.
  for file in types procs; do
    sed -i -f "$script" "$work/$file.jai"
  done
  while read -r from to; do
    [ -z "${from:-}" ] && continue
    case "$from" in \#*) continue ;; esac
    sed -i "s/^$to :: (/$from :: (/" "$work/procs.jai"
  done < "$renames"
fi

# The enums the module names or widens differently from what the headers say.
overrides=$here/$module/enums.txt
if [ -f "$overrides" ]; then
  # Puts an anonymous enum's common prefix back on each member, and on every
  # member it names in an initialiser — `_SC_IOV_MAX :: _SC_UIO_MAXIOV`.
  cat > "$work/keep-prefix.awk" <<'AWK'
$0 ~ "^" tag " :: enum" { inside = 1; print; next }
inside && /^}/          { inside = 0; print; next }
inside && /^    [A-Za-z_0-9]/ {
  line = $0
  sub(/^    /, "", line)
  if (match(line, / :: /)) {
    name = substr(line, 1, RSTART - 1)
    rest = substr(line, RSTART + 4)
    out = ""
    while (match(rest, /[A-Za-z_][A-Za-z_0-9]*/)) {
      out = out substr(rest, 1, RSTART - 1) tag "_" substr(rest, RSTART, RLENGTH)
      rest = substr(rest, RSTART + RLENGTH)
    }
    print "    " tag "_" name " :: " out rest
  } else {
    print "    " tag "_" line
  }
  next
}
{ print }
AWK
  while read -r from to base flags; do
    [ -z "${from:-}" ] && continue
    case "$from" in \#*) continue ;; esac
    prefix=""
    case " ${flags:-} " in *" using "*) prefix="using " ;; esac
    # An anonymous enum has its members' common prefix taken off, which is
    # what turns `DT_DIR` into `DT.DIR`. Where the module wants the C spelling
    # back — `sysconf` reads `_SC_NPROCESSORS_ONLN` — it is put back on.
    case " ${flags:-} " in
      *" keep-prefix "*)
        awk -v tag="$from" -f "$work/keep-prefix.awk" "$work/enums.jai" > "$work/enums.tmp"
        mv "$work/enums.tmp" "$work/enums.jai"
        ;;
    esac
    sed -i "s/^$from :: enum [A-Za-z0-9]* {/$prefix$to :: enum $base {/" "$work/enums.jai"
    # The tag the headers gave it still resolves, as an alias — unless the
    # enum is `using`, which already puts its members in scope under the tag.
    keeps_tag=yes
    case " ${flags:-} " in *" using "*) keeps_tag=no ;; esac
    if [ "$from" != "$to" ] && [ "$keeps_tag" = yes ]; then
      printf '\n%s :: %s;\n' "$from" "$to" >> "$work/enums.jai"
    fi
  done < "$overrides"
fi

# Declarations the module ships in place of what the headers say. The
# generated one is dropped, whichever section it landed in, and the
# replacement is emitted in its place at the end.
overridden=$here/$module/overrides.jai
if [ -f "$overridden" ]; then
  grep -oE '^(using )?[A-Za-z_][A-Za-z_0-9]* ::' "$overridden" \
    | sed 's/^using //; s/ ::$//' | sort -u > "$work/overridden.txt"
  cat > "$work/drop.awk" <<'AWK'
BEGIN { while ((getline name < list) > 0) drop[name] = 1 }
skipping {
  depth += gsub(/{/, "{") - gsub(/}/, "}")
  if (depth <= 0) skipping = 0
  next
}
{
  line = $0
  sub(/^using /, "", line)
  if (match(line, /^[A-Za-z_][A-Za-z_0-9]* ::/)) {
    name = substr(line, 1, RLENGTH - 3)
    if (name in drop) {
      depth = gsub(/{/, "{") - gsub(/}/, "}")
      if (depth > 0) skipping = 1
      next
    }
  }
  print
}
AWK
  for file in constants types enums procs; do
    awk -v list="$work/overridden.txt" -f "$work/drop.awk" "$work/$file.jai" > "$work/$file.tmp"
    mv "$work/$file.tmp" "$work/$file.jai"
  done
  echo "replaced $(wc -l < "$work/overridden.txt") declaration(s) from overrides.jai"
fi

section() { printf '\n// %s %s ---\n\n' "$(printf -- '-%.0s' $(seq 1 $((71 - ${#1}))))" "$1"; }

{
  cat "$here/$module/header.jai"
  section "constants";  cat "$work/constants.jai"
  section "the types";  cat "$work/types.jai"
  section "enums";      cat "$work/enums.jai"
  section "procedures"; cat "$work/procs.jai"
  if [ -f "$overridden" ]; then
    section "the overrides"
    # The file's own header comment goes; everything from the first real
    # declaration is kept as written, blank lines inside a body included.
    awk 'started { print; next } /^[^\/[:space:]]/ { started = 1; print }' "$overridden"
  fi
  cat "$here/$module/tail.jai"
} > "$target.new"

echo
if diff -q "$target" "$target.new" >/dev/null 2>&1; then
  echo "no change: $(basename "$target") is what the headers say"
  rm -f "$target.new"
  exit 0
fi
diff --unified=0 "$target" "$target.new" | head -60 || true
echo
echo "diffstat: $(diff "$target" "$target.new" | grep -c '^<') removed, $(diff "$target" "$target.new" | grep -c '^>') added"
if [ "$write" = yes ]; then
  mv "$target.new" "$target"
  echo "written: $target"
else
  echo "candidate: $target.new  (read the diff, then re-run with --write)"
fi
