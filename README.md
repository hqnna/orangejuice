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

## Building

Everything was built in an isolated sandbox using a Nix flake.

```console
$ nix develop             # dev shell: nightly Rust, LLVM 19, clang
$ nix flake check         # clippy, fmt, test and doc checks
$ nix run . -- --version
```
