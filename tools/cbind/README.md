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
| `posix`  | glibc's POSIX set | `modules/POSIX/generated_{x64,arm64}.jai` |
| `socket` | BSD sockets, netdb, netinet | `modules/Socket/generated.jai` |
| `linux`  | epoll, inotify, input, statx, io_uring | `modules/Linux/generated.jai` |
| `lz4`    | lz4, lz4hc, lz4frame | `modules/lz4/generated.jai` |
| `macos`  | the Darwin equivalents, plus kqueue, sysctl and dyld | `modules/POSIX/generated_macos.jai` |

`posix` writes the file for the architecture it runs on, because that binding
is the *architecture's* and not just the system's: 299 of its declarations
differ between x86-64 and arm64. Most are syscall numbers — `SYS_exit` is 60
on one and 93 on the other — but `__nlink_t` is `u64` against `u32`,
`__blksize_t` is `s64` against `s32`, `O_NOFOLLOW` and `O_DIRECT` are
different numbers, and the pthread attribute types are laid out differently.
`socket` and `linux` were generated on both and come out identical, so they
are one file each.

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
1,800 — writing the four wanted lists for it would be more work than the
declarations themselves. That is a judgement about size, not a limitation:
`macos/headers.c` is there and `generate.sh macos` runs on a Mac, so the
moment that surface stops being small it should be generated like everything
else.

What makes the hand-written file safe to keep is that it is *measured*:
`a_darwin_layout_is_what_the_c_compiler_says` in
`crates/driver/tests/distribution.rs` compiles a C program against the system
headers and asserts all 118 sizes, offsets and constant values in `macos.jai`
and the crash handler against it. It found `_SC_GETPW_R_SIZE_MAX` written as
70 where Darwin says 71. **Add a Darwin declaration, add it to that test in
the same commit** — a constant that is wrong by one does not announce itself
the way a struct that is wrong by eight bytes does.

## Running it

Inside `nix develop`, one command per module:

```
tools/cbind/generate.sh posix           # writes generated.jai.new and diffs it
tools/cbind/generate.sh posix --write   # ... and moves it into place
```

lz4's headers are not in the dev shell; point the script at them:

```
CBIND_CFLAGS=-I$(nix build --no-link --print-out-paths 'nixpkgs#lz4.dev')/include \
  tools/cbind/generate.sh lz4
```

Without `--write` nothing is overwritten: the candidate is left beside the
module's file and the diff is printed. **Regenerating a module that has not
changed prints `no change` and writes nothing**, which is the useful property
— a diff means either the headers moved or somebody edited the generated file
by hand.

### What it runs

Four emitters, each reading the names it should emit on stdin and writing Jai
on stdout:

| Program | Emits | Wanted list |
|---|---|---|
| `cconsts` | a C program that prints each macro's value | `wanted-constants.txt` |
| `cstructs` | structs, unions, typedefs and opaque types | `wanted-types.txt` |
| `cenums` | enums, with the reference's naming rules | `wanted-enums.txt` |
| `cbind` | `#foreign` procedure declarations | `wanted-procedures.txt` |

The wanted lists are the module's surface, and they hold the names *C* uses —
the renames below are applied after. Constant values come from compiling and
running the program `cconsts` writes, so nothing is guessed: `O_CREAT` is
`0100` in the header and `64` here because the compiler said so. A macro the
compiler will not take is dropped and the program built again until what is
left compiles; a macro whose value is a *string* is skipped outright, because
casting one to a number yields whatever address the literal landed at — which
is how `P_tmpdir` used to come out as `93824992259364`.

A type the output leans on but does not declare is chased to a fixed point;
anything still undeclared that the headers mention only behind a pointer is
emitted as an opaque struct, which is what a C incomplete type is.

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
- `<module>/flaggroups.txt` lists the families of `#define`s the reference
  groups into `enum_flags`, with the width it gives each and, optionally,
  `strip`. The members' values come from the same compiled program the plain
  constants do, and are then taken out of the constants section. Without
  `strip` the members keep the C spelling and are listed by name, which is
  what the io_uring families are; with it the common prefix comes off, the
  values read as hex in bit order, and the C spellings follow as aliases so
  that both compile — `AI.PASSIVE` and `AI.AI_PASSIVE`.
- `<module>/enums.txt` is the enums the module names or widens differently from
  what the headers say: `__rusage_who` ships as `RUSAGE` because a tag says
  nothing about its members, `DT` is narrowed to `u8` because `dirent.d_type`
  is one byte and widening it would move every field after it, and
  `_SC_definitions` keeps its members' `_SC_` prefix so that `sysconf` reads
  the way C writes it.
- `<module>/overrides.jai` is whole declarations the module ships *instead of*
  what the generator produces. The generated one is dropped and this one takes
  its place. `clockid_t` is there: the headers make it a plain `typedef int`,
  which would leave `clock_gettime(.REALTIME, *now)` with nothing to name.

### Where each module stands

`posix` round-trips: regenerating it prints `no change`, on both
architectures. `socket` and `linux` do not yet. Their `flaggroups.txt` is
applied now and reproduces exactly — `IOSQE_Flags` and the rest come out byte
for byte — but two other things stand in the way, and neither is the
generator's fault:

- **Enums they rename.** `__socket_type` ships as `SOCK`, `io_uring_op` as
  `IORING_OP`, `statx` as `statx_t`. `enums.txt` and `renames.txt` are where
  those belong; nobody has written them down yet.
- **Headers that moved.** The committed files were generated against an older
  glibc, which still had `AI_IDN_ALLOW_UNASSIGNED`, `NI_IDN_*` and a shelf of
  legacy `sockaddr_*`. Regenerating drops them, and dropping a declaration a
  program might name is not a decision the generator should make quietly.

So run those two without `--write`, read the diff, and decide. `lz4` has
lists but has never been run — its headers are not in the dev shell.

## What is not generated

`generated.jai` is exactly what `generate.sh` produces — that is checked by
regenerating and seeing `no change`. Three things make that true without the
generator having to be clever, and all three are data beside the headers:
`renames.txt`, `enums.txt` and `overrides.jai`, described above. If a
regeneration ever produces a diff nobody expected, one of those is where the
answer belongs — not a hand edit to the output, which the next regeneration
would throw away.

## What it deliberately leaves out

Symbols this glibc no longer defines — the `__xstat` family, `_STAT_VER`,
`_sys_siglist` — are not emitted. Declaring them would be inventing an ABI the
machine does not have.
