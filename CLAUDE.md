# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repository is

orangejuice (`oj`) is a cleanroom reimplementation of the **Jai** programming language (reference beta 0.2.009) in Rust nightly, targeting **Linux x86_64 only**, with a single LLVM 19 backend via `inkwell` and LLVM ORC JIT for compile-time execution.

**Current state: specification-only.** `docs/` holds the three specs; there is no Rust code, no `flake.nix`, and no `Cargo.toml` yet. Milestone M0 (workspace + crate skeleton + nix flake + `oj help`/`oj version`) has not landed. The build/test commands below describe the workflow the spec mandates once the scaffold exists.

## The specs are the source of truth

Three documents, cross-referenced by section (`**L§5.10**`, `**C§6.4**`):

| File | Referenced as | Contents |
|---|---|---|
| `docs/language.md` | **L** | Normative Jai language reference: lexical structure, types/layout, scopes, expressions, procedures, polymorphism, macros, the context, modules, compile-time execution, `#asm`, Preload, grammar. §19 lists known deviations from the reference compiler. |
| `docs/compiler.md` | **C** | How the *reference* compiler behaves: pipeline, CLI + `Default_Metaprogram`, workspaces/messages, `Build_Options`, `Code_Node` kinds and exact struct layouts, the dependency scheduler, linking, diagnostic wording. |
| `docs/spec.md` | — | The orangejuice project spec: goals, non-goals, crate layout, CLI, architecture per crate, testing strategy, milestones M0–M10, definition of done. |

Before implementing anything, read the relevant **L** and **C** sections. `docs/compiler.md` §5.3 in particular is the authoritative list of AST node kinds, flags and numeric enum values — the AST in `oj-syntax` mirrors it so that exporting to metaprogram-visible `Code_*` structs is a projection, not a translation. When behavior or architecture changes, update the corresponding `docs/*.md` in the same commit.

## The vendored Jai distribution

`vendor/jai/` is **git-ignored** and holds the beta 0.2.009 reference distribution: `modules/` (the module tree orangejuice must compile unmodified), `how_to/` (the primary acceptance suite), `examples/`, `bin/jai-linux` (the reference compiler, usable to generate golden outputs). 702 `.jai` files total. `OJ_JAI_DIR` overrides the location; integration tests that need it must skip with a clear message when it is absent.

The vendor tree is both the spec's evidence base and the test corpus — when a question about semantics is not answered by **L**/**C**, read the relevant module under `vendor/jai/modules/` or how_to file rather than guessing.

## Build and test workflow

Rust toolchain, LLVM and every tool come from the nix flake; `cargo`/`rustc` are not on `PATH` outside it.

```
nix develop                 # dev shell: fenix nightly, LLVM 19, clang, pkg-config, LLVM_SYS_191_PREFIX
nix flake check             # crane checks: clippy, fmt, test, doc
```

Inside the shell, the full gate that must pass before every commit:

```
cargo check && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Running one test: `cargo test -p oj-<crate> <test_name>`, or `cargo test -p oj-lexer -- --nocapture` for a single crate. Snapshot tests use `insta` (`cargo insta review` to accept changes).

Nix files are exactly three and no more: `flake.nix`, `nix/shell.nix`, `nix/package.nix`. There is no Nix formatter.

## CLI surface

`oj` is a clap-derive CLI whose `build`/`run` subcommands accept jai-style metaprogram options verbatim, so `jai first.jai - -android` becomes `oj build first.jai - -android`:

```
oj build <file.jai> [jai options]      oj dump tokens <file.jai>
oj run   <file.jai> [jai options] [-- args]   oj dump ast <file.jai> [--tree]
oj version                             oj dump ir  <file.jai> [--proc NAME]
                                       oj dump asm <file.jai> [--llvm]
```

Exit codes: 0 success, 1 compile/link failure, 2 usage error. Env: `OJ_JAI_DIR`, `OJ_LOG` (tracing filter), `OJ_THREADS`.

## Architecture

Cargo workspace; directories are `crates/<name>`, package names are `oj-<name>`, the binary crate is `crates/cli` producing `oj`. The pipeline mirrors the reference compiler's (`docs/spec.md` §6 is the detailed version):

```
source → lexer → syntax (AST) → scope → sema (typecheck + scheduler) → ir → codegen (LLVM) → link
                                          ↕                              ↕
                                   jit + runtime (compile-time execution, data segments)
                                          ↕
                                   meta (Compiler module ABI ↔ metaprograms)
```

Cross-cutting facts that are not obvious from any single crate:

- **The scheduler drives everything.** `oj-sema` runs work items (declarations, bodies, structs, `#run`s, `#if`s, polymorph instantiations) on a thread pool; an item that hits an unresolved dependency suspends and is re-queued when the dependency completes. Quiescence detection produces circular-dependency and batched undeclared-identifier errors. The metaprogram message loop is back-pressure on this scheduler.
- **The metaprogram is the driver, not a plugin.** `Default_Metaprogram.jai` is compiled from the vendor tree and JIT-executed on its own thread; it creates the target workspace and sets `Build_Options`. orangejuice reimplements the *argument parsing* in Rust but still forwards to the real module.
- **`oj-meta` is an ABI, not an API.** `vendor/jai/modules/Compiler/Compiler.jai` is compiled unmodified; the Rust `#[repr(C)]` mirrors must match its layouts byte for byte, asserted by generated size/offset tests.
- **JIT instead of an interpreter** is the one documented semantic deviation (`docs/spec.md` §6.5). The reference stalls a `#run` mid-execution on an unresolved declaration; orangejuice instead computes the `#run`'s transitive dependency closure and schedules it only when complete. `-debugger` is a stack-trace dump, not an interactive debugger.
- **Compile-time and runtime share bytes.** Globals in JIT code live in compiler-owned data segments (`oj-runtime`); the same bytes are written into the executable at the end, writable data restored from a pre-`#run` backup except `#no_reset`, pointers remapped by recorded relocation tables.
- **Diagnostics wording is a tested interface.** Error text matches the reference wherever **C§12** or **L** documents it; every documented message gets a negative test asserting wording and location.

## Conventions

- **Style**: 2-space indentation everywhere (Rust via `rustfmt.toml` `tab_spaces = 2`, plus Nix, Markdown, JSON, TOML). Avoid comments in code — names and tests carry the meaning; doc comments only on public crate APIs where a name cannot.
- **Dependencies**: prefer well-maintained crates over hand-rolled code (`clap`, `inkwell`, `cc`, `pkg-config`, `libloading`, `memmap2`, `rayon`/`crossbeam`, `indexmap`, `smallvec`, `insta`, `object`).
- **Commits**: conventional commits scoped by crate (`feat(lexer): …`, `fix(sema): …`, `docs: …`), one logical change each, gate above passing first.
- **No functionality lands without unit tests** in the owning crate.
