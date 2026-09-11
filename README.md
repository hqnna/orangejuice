# orangejuice

orangejuice (`oj`) is a cleanroom implementation of the [Jai](https://jai.community/)
programming language, compatible with **beta 0.2.009**. Written in Rust, it
targets Linux x86_64 through LLVM 19, and runs its compile-time code on an LLVM
ORC JIT.

It is a whole distribution, not just a compiler: `modules/` is a clean-room
standard library — Preload, Runtime_Support, Basic, String, Math, Hash_Table,
File, Socket, Thread, POSIX, Compiler and the rest — written from the language
reference rather than derived from anyone else's source. Nothing in this
repository is copied from the reference distribution, and nothing depends on
having one.

```
oj hello.jai                      compile it; the executable lands beside the source
oj hello.jai -- run               compile and run it
oj a.jai b.jai -release           several files, optimized
oj hello.jai - --port 8080        arguments after `-` reach the program's #runs
oj hello.jai -- dump ir proc main what the back end made of one procedure
oj -- help                        the compiler's own options
```

The command line is Jai's, so `jai` invocations translate verbatim; everything
after the last `--` is orangejuice's own (`docs/spec.md` §5).

## Building

Everything comes from the Nix flake:

```
nix develop                 # dev shell: nightly Rust, LLVM 19, clang
nix flake check             # clippy, fmt, test and doc checks
nix run . -- --version
```

Inside the dev shell, `scripts/check.sh` runs the same gate locally:

```
cargo check && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## Examples

`examples/` is the acceptance suite and a tour of the language — 23 programs,
each with the output it must print. `010` through `230` covers values, control
flow, procedures, structs, arrays, pointers, enums, strings, polymorphism,
compile-time execution, macros, the context, modules, type info, operator
overloading, `using`, files, threads, C interop, inline assembly and writing a
metaprogram.

```
oj examples/010_hello.jai -- run
cargo test -p oj-driver --test examples
```

Each golden was compared against the reference compiler, byte for byte, when
the example was written.

## Documentation

| File | Contents |
|---|---|
| `docs/language.md` | the Jai language reference orangejuice implements |
| `docs/compiler.md` | the behavior of the reference compiler it matches |
| `docs/spec.md` | goals, architecture, CLI, testing strategy, milestones |

## License

MIT, see `LICENSE`.
