# cbind — the C binding generator

`modules/POSIX/generated.jai` is not typed out by hand. It is produced from the
system's own C headers, which is how the reference distribution produces its
bindings too: a constant's value is whatever `<fcntl.h>` says on the machine,
a struct's layout is whatever the compiler lays out, and a procedure's
parameter names are the ones glibc gave them. What ships is therefore a
projection of the C ABI rather than a transcription of anyone's source.

## Running it

Inside `nix develop`:

```
clang -Xclang -ast-print -fsyntax-only tools/cbind/headers.c > ast.txt
clang -dM -E tools/cbind/headers.c | sed 's/^#define //' > macros.txt
```

Then the three emitters, each of which reads the wanted names on stdin and
writes Jai on stdout:

| Program | Emits |
|---|---|
| `cbind` | `#foreign` procedure declarations |
| `cstructs` | structs, unions, typedefs and opaque types |
| `cenums` | enums, with the reference's naming rules |

Constant *values* come from compiling a generated C program that prints each
one, so nothing is copied and nothing is guessed.

## The rules it follows

- Types map the way the reference maps them: `char *` is `*u8`, `int` is `s32`,
  a variadic `...` is `..Any`.
- An anonymous C enum is named after the common prefix of its members, and that
  prefix comes off each member — `DT_DIR` becomes `DT.DIR`. A *tagged* enum
  keeps the names C gave its members, since the tag says nothing about them.
- A C struct whose name a function also uses cannot keep it, because Jai has
  one namespace for both. `renames.txt` says which name wins.
- `tail.jai` holds the handful of declarations no generator can read out of a
  header: the `S_IS*` and `W*` macros, the globals libc defines, and two C
  shapes the parser does not cover.

## What it deliberately leaves out

Symbols this glibc no longer defines — the `__xstat` family, `_STAT_VER`,
`_sys_siglist` — are not emitted. Declaring them would be inventing an ABI the
machine does not have.
