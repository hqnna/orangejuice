# cbind — the C binding generator

`modules/POSIX/generated.jai` and its three siblings are not typed out by hand.
They are produced from the system's own C headers, which is how the reference
distribution produces its bindings too: a constant's value is whatever
`<fcntl.h>` says on the machine, a struct's layout is whatever the compiler
lays out, and a procedure's parameter names are the ones glibc gave them. What
ships is therefore a projection of the C ABI rather than a transcription of
anyone's source.

| Module | Headers | Generated into |
|---|---|---|
| `posix`  | glibc's POSIX set | `modules/POSIX/generated.jai` |
| `socket` | BSD sockets, netdb, netinet | `modules/Socket/generated.jai` |
| `linux`  | epoll, inotify, input, statx, io_uring | `modules/Linux/generated.jai` |
| `lz4`    | lz4, lz4hc, lz4frame | `modules/lz4/generated.jai` |

**Every one of these is a Linux binding, and the generator has to be run on
Linux.** `posix/headers.c` includes `<sys/epoll.h>`, `<sys/inotify.h>` and a
dozen more that exist nowhere else, so the preparation step fails outright on
a Mac rather than producing something wrong. `docs/spec.md` §7.2 is how a
Linux host is reached from one.

That matters most for `posix`, which is the one module with a second system
behind it. `POSIX/module.jai` holds what every Unix agrees on and loads one of
two files beside it:

| File | What it is |
|---|---|
| `POSIX/linux.jai` | hand-written: the flag numbers, kernel layouts and glibc object sizes Linux fixes — and it is what `#load`s `generated.jai` |
| `POSIX/macos.jai` | hand-written: the same for Darwin, plus the C library entry points the Linux side takes from the generated file |

There is no generated Darwin binding. What the standard library calls on a Mac
is small enough to declare, and `a_darwin_layout_is_what_the_c_compiler_says`
in `crates/driver/tests/distribution.rs` measures every layout in `macos.jai`
against the system's own headers, which is the check a generated file would
otherwise be standing in for.

## Running it

Inside `nix develop`:

```
tools/cbind/generate.sh posix
```

lz4's headers are not in the dev shell; point the script at them:

```
CBIND_CFLAGS=-I$(nix build --no-link --print-out-paths 'nixpkgs#lz4.dev')/include \
  tools/cbind/generate.sh lz4
```

The three emitters each read the wanted names on stdin and write Jai on
stdout:

| Program | Emits |
|---|---|
| `cbind` | `#foreign` procedure declarations |
| `cstructs` | structs, unions, typedefs and opaque types |
| `cenums` | enums, with the reference's naming rules |

Constant *values* come from compiling a generated C program that prints each
one, so nothing is guessed. A type the output leans on but does not declare is
chased to a fixed point; anything still undeclared that the headers mention
only behind a pointer is emitted as an opaque struct, which is what a C
incomplete type is.

## The rules it follows

- Types map the way the reference maps them: `char *` is `*u8`, `int` is `s32`,
  a variadic `...` is `..Any`.
- An anonymous C enum is named after the common prefix of its members, and that
  prefix comes off each member — `DT_DIR` becomes `DT.DIR`. A *tagged* enum
  keeps the names C gave its members, since the tag says nothing about them:
  `__rusage_who` becomes `RUSAGE.RUSAGE_SELF`.
- A C struct whose name a function also uses cannot keep it, because Jai has
  one namespace for both. `<module>/renames.txt` says which name wins, matching
  the reference case by case — it keeps the `sigaction` and `stat` *functions*
  and renames their structs, and keeps the `flock` *struct* and drops its
  function.
- `<module>/tail.jai` holds what no header can answer: C's macros (`S_IS*`,
  `W*`, `FD_*`, `CMSG_*`), the globals libc defines, the io_uring syscall
  wrappers, and the conveniences the reference ships beside its bindings.
- `<module>/flaggroups.txt` lists the families of `#define`s the reference groups
  into `enum_flags`, with the width it gives each.

## What it deliberately leaves out

Symbols this glibc no longer defines — the `__xstat` family, `_STAT_VER`,
`_sys_siglist` — are not emitted. Declaring them would be inventing an ABI the
machine does not have.
