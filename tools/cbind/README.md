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
| `macos`  | the Darwin equivalents, plus kqueue, sysctl and dyld | `modules/POSIX/generated_macos.jai` |

The generator is not tied to Linux — the emitters read clang's `-ast-print`
and know nothing about glibc — but each *header list* is. `posix/headers.c`
includes `<sys/epoll.h>`, `<sys/inotify.h>` and a dozen more that exist
nowhere else, so `generate.sh posix` has to run on Linux; `macos/headers.c` is
the same list with what Darwin has instead, and has to run on a Mac.
`docs/spec.md` §7.2 is how a Linux host is reached from one.

`posix` is the module with two systems behind it. `POSIX/module.jai` holds
what every Unix agrees on and loads one of two files beside it:

| File | What it is |
|---|---|
| `POSIX/linux.jai` | hand-written: the flag numbers, kernel layouts and glibc object sizes Linux fixes — and it is what `#load`s `generated.jai` |
| `POSIX/macos.jai` | hand-written: the same for Darwin, plus the C library entry points the Linux side takes from the generated file |

`macos.jai` is hand-written rather than generated because the Darwin surface
the standard library actually calls is a few dozen names, where glibc's is
1,800 — running the pipeline for it would be more assembly than declaration.
That is a judgement about size, not a limitation: `generate.sh macos` prepares
the same inputs on a Mac, and the moment the Darwin surface stops being small
it should be generated like everything else.

What makes the hand-written file safe to keep is that it is *measured*:
`a_darwin_layout_is_what_the_c_compiler_says` in
`crates/driver/tests/distribution.rs` compiles a C program against the system
headers and asserts all 118 sizes, offsets and constant values in `macos.jai`
and the crash handler against it. It found `_SC_GETPW_R_SIZE_MAX` written as
70 where Darwin says 71. **Add a Darwin declaration, add it to that test in
the same commit** — a constant that is wrong by one does not announce itself
the way a struct that is wrong by eight bytes does.

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
