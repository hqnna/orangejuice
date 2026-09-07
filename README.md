# orangejuice

orangejuice (`oj`) is a cleanroom implementation of the [Jai](https://jai.community/)
programming language (reference beta 0.2.009) written in Rust, targeting Linux
x86_64 with an LLVM 19 backend and LLVM ORC JIT for compile-time execution.

Status: scaffolding. `oj version` and `oj help` work; the compiler itself is
being built milestone by milestone (`docs/spec.md` §9).

## Building

Everything comes from the Nix flake:

```
nix develop                 # dev shell: nightly Rust, LLVM 19, clang
nix flake check             # clippy, fmt, test and doc checks
nix run . -- version        # run oj without installing it
```

Inside the dev shell, `scripts/check.sh` runs the same gate locally.

## Documentation

| File | Contents |
|---|---|
| `docs/language.md` | the Jai language reference orangejuice implements |
| `docs/compiler.md` | the behavior of the reference compiler it must match |
| `docs/spec.md` | goals, architecture, CLI, testing strategy, milestones |

## License

MIT, see `LICENSE`.
