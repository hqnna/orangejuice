# orangejuice

A cleanroom implementation of Jonathan Blow's language Jai. Built entirely from
the ground up in Rust and made to be compatible with Jai Beta v0.2.009. Aiming
to be a drop-in replacement with quality of life improvements, such as system
LLVM, LLD, and pkg-config support, with compile-time code execution using the
LLVM ORC JIT engine.

It is a standalone distribution, not just a compiler. The `modules` directory is
a cleanroom implementation of Jai's standard library, minus a bunch of modules
that felt unnecessary or could be handled by libraries instead. Based upon but
not copied from the reference implementation of the language. Nothing in this
repository is copied from the reference distribution, and nothing depends on
having one.

```console
$ oj hello.jai                       # compile a program
$ oj hello.jai -- run                # compile and run a program
$ oj a.jai b.jai                     # compile multiple files
$ oj hello.jai - --port 8080         # arguments after `-` reach the program's #runs
$ oj hello.jai -- dump ir proc main  # what the back end made of one procedure
$ oj -- help                         # the compiler's own options
```

The command line is meant to act as a drop-in replacement for `jai`, so all
invocations translate verbatim, everything after the last `--` is orangejuice's
own commands and arguments.

## Platforms

`x86_64-linux`, `aarch64-linux` and `aarch64-darwin`. Each is both a host and a
target: `oj` builds and runs there, and emits and links native code for there.
Releases carry a tarball per platform that brings its own libraries, so it runs
on a machine with nothing installed.

Inline assembly works on both architectures. `#asm` is x86-64 in the reference
language; orangejuice also assembles arm64 blocks, which is an extension of
its own that `docs/language.md` §15.9 specifies. A block written for one
architecture does not mean anything on the other, so a program that wants
both writes `#if CPU == .X64`.

## Building

Everything was built in an isolated sandbox using a Nix flake.

```console
$ nix develop             # dev shell: nightly Rust, LLVM 19, clang
$ nix flake check         # clippy, fmt, test and doc checks
$ nix run . -- --version
```
