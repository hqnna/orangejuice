# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repository is

orangejuice (`oj`) is a cleanroom reimplementation of the **Jai** programming language (reference beta 0.2.009) in Rust nightly, targeting **Linux x86_64 only**, with a single LLVM 19 backend via `inkwell` and LLVM ORC JIT for compile-time execution.

**Current state: M4 (types and layout, constants, Match, casts, overloads, structs/enums/variants, Preload types) has landed.** The nix flake, the cargo workspace and the sixteen crates of `docs/spec.md` §4 exist. `oj version`, `oj help`, `oj dump tokens`, `oj dump ast`, `oj dump scopes` and `oj dump types` work; every other subcommand exits 1 with a "not implemented yet" message naming the milestone that will implement it. Filled in so far: `crates/cli` (the clap CLI), `crates/testsupport` (vendor discovery), `crates/diag` (spans, source map, diagnostics and their reference-format rendering), `crates/source` (memory-mapped loading, module and `#load` path resolution), `crates/lexer` (tokens, the lexer, the name interner, the token dump), `crates/syntax` (the AST arena, the parser, the source and tree printers), `crates/scope` (the scope tree, the program loader, the `#if` constant folder, name resolution and the scope dump), `crates/types` (the type table, layout and type printing) and `crates/sema` (declaration types, struct/enum/variant construction, constants, Match, overload resolution and body checking). The rest are still empty — M5 (IR, LLVM codegen, link) is next.

The lexer follows the reference *compiler*; where the shipped `Jai_Lexer` module disagrees with it, **C§5.1** says which of the two is right. `crates/lexer/tests/corpus.rs` lexes all 702 vendor files, and setting `OJ_TOKEN_STREAM_DIR` makes it write a per-file cross-check dump to compare against a `Jai_Lexer`-based dumper.

The AST mirrors `Code_Node.Kind` (**C§5.3**); `crates/syntax/tests/corpus.rs` parses the same 702 files and checks that each one *round-trips* — printing the tree and re-parsing it must give an identical tree, compared through the span-free tree form. That test is the parser's and the printer's contract with each other: whenever one of them learns a construct, the other has to. `crates/syntax/src/rules.rs` holds the one rule they share, which statements need a `;`.

`oj-scope` turns a root file into the scope tree of **L§4.2**: Preload at the root, a module scope per instantiation under it, a file scope per `#load`ed file, then struct/enum/procedure/block scopes. Declarations land in the file or the module scope according to the `#scope_*` directive in effect; `#import` and `using` are recorded as *edges* rather than copied names, so overload sets and `using,except(…)` filters come out right. `crates/scope/tests/corpus.rs` resolves all 702 files on their own and then every top-level `how_to` program whole. Two rules make that possible before the scheduler exists (both in `docs/spec.md` §10): a `#if` nobody can decide yet contributes *every* branch as conditional declarations with its diagnostics held back, and a lookup that misses in a scope holding an unfinished name-inserting construct is a wait rather than an error.


`oj-types` holds the interned type table: pointer, array and procedure types are structural, structs, enums, variants and `$T` variables nominal (**L§3.13**), and `LayoutBuilder` puts members where **L§3.14** and **L§8.6** say. `oj-sema` types every declaration of a resolved program on demand — asking for one resolves what it depends on first, which is what makes data scopes order-independent without the scheduler of `docs/spec.md` §6.2. A struct's *identity* is a separate step from its *body*, so a pointer or a procedure signature never waits for a layout; the only circular dependency left to report is a struct that contains itself. Anything needing a milestone the front end has not reached is the `unknown` type, which converts to everything and is never reported, so M4 does not complain about polymorphs (M7) or `#run` (M6). `crates/sema/tests/corpus.rs` types all 702 vendor files on their own and every `how_to` program whole, and pins 17 standard-module struct sizes to values measured with the reference compiler.

**Measure the reference compiler rather than trusting the docs on layout and conversion corners.** Writing a small `.jai` that prints `size_of`/`type_of` and running it through `nix run ./vendor/jai` has already corrected `docs/language.md` three times: `#no_padding` drops only *trailing* padding, `#align` is rejected on a struct declaration, and a bitwise operator whose left operand is an explicit `cast` keeps the cast's type where arithmetic would widen. When a corpus file disagrees with the implementation, that experiment is the cheapest way to find out which is wrong.

## The specs are the source of truth

Three documents, cross-referenced by section (`**L§5.10**`, `**C§6.4**`):

| File | Referenced as | Contents |
|---|---|---|
| `docs/language.md` | **L** | Normative Jai language reference: lexical structure, types/layout, scopes, expressions, procedures, polymorphism, macros, the context, modules, compile-time execution, `#asm`, Preload, grammar. §19 lists known deviations from the reference compiler. |
| `docs/compiler.md` | **C** | How the *reference* compiler behaves: pipeline, CLI + `Default_Metaprogram`, workspaces/messages, `Build_Options`, `Code_Node` kinds and exact struct layouts, the dependency scheduler, linking, diagnostic wording. |
| `docs/spec.md` | — | The orangejuice project spec: goals, non-goals, crate layout, CLI, architecture per crate, testing strategy, milestones M0–M10, definition of done. |

Before implementing anything, read the relevant **L** and **C** sections. `docs/compiler.md` §5.3 in particular is the authoritative list of AST node kinds, flags and numeric enum values — the AST in `oj-syntax` mirrors it so that exporting to metaprogram-visible `Code_*` structs is a projection, not a translation. When behavior or architecture changes, update the corresponding `docs/*.md` in the same commit.

## The vendored Jai distribution

`vendor/jai/` is **git-ignored** and holds the beta 0.2.009 reference distribution: `modules/` (the module tree orangejuice must compile unmodified), `how_to/` (the primary acceptance suite), `examples/`, `bin/jai-linux` (the reference compiler, usable to generate golden outputs, run with `nix run ./vendor/jai`). 702 `.jai` files total. `OJ_JAI_DIR` overrides the location; integration tests that need it must skip with a clear message when it is absent.

The vendor tree is both the spec's evidence base and the test corpus — when a question about semantics is not answered by **L**/**C**, read the relevant module under `vendor/jai/modules/` or how_to file rather than guessing.

**Run the reference compiler through its flake**, never `vendor/jai/bin/jai-linux` directly: the binary resolves `#library,system` names and the `-L` flags it hands to `lld` from `/etc/ld.so.conf`, `/lib`, `/usr/lib` and `/usr/lib64` — honouring neither `LD_LIBRARY_PATH` nor an rpath — so on NixOS it fails to load `libc`. `vendor/jai/flake.nix` wraps the distribution in an FHS sandbox where it works:

```
nix run ./vendor/jai -- hello.jai              # jai options verbatim; the executable lands beside the source
nix run ./vendor/jai#shell                     # a shell where jai-linux and the programs it builds run
```

The wrapper locates the distribution the way `oj-testsupport` does, from `OJ_JAI_DIR` or the nearest enclosing `vendor/jai`, so it works from any directory in the checkout. The flakeref needs its `./` — a bare `vendor/jai` is looked up in the flake registry. `flake.nix` and `flake.lock` are the only tracked files in the otherwise git-ignored vendor tree; the rest of a re-unpacked distribution must not clobber them.

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

Nix files are exactly four and no more: `flake.nix`, `nix/shell.nix`, `nix/package.nix` and `vendor/jai/flake.nix` (the reference-compiler wrapper). There is no Nix formatter.

## CLI surface

`oj` is a clap-derive CLI whose `build`/`run` subcommands accept jai-style metaprogram options verbatim, so `jai first.jai - -android` becomes `oj build first.jai - -android`:

```
oj build <file.jai> [jai options]      oj dump tokens <file.jai>
oj run   <file.jai> [jai options] [-- args]   oj dump ast    <file.jai> [--tree]
oj version                             oj dump scopes <file.jai> [--file-only]
                                       oj dump types  <file.jai> [--file-only]
                                       oj dump ir     <file.jai> [--proc NAME]
                                       oj dump asm    <file.jai> [--llvm]
```

Exit codes: 0 success, 1 compile/link failure, 2 usage error. Env: `OJ_JAI_DIR`, `OJ_LOG` (tracing filter), `OJ_THREADS`.

**Single-dash options are the reference compiler's, not ours.** Before adding or changing any option on `build`/`run`, read `docs/spec.md` §5.1 (the full table: name, arity, `Build_Options` effect), **C§2.1** (invocation, compiler-level `---`/`--` options, the exact command-line error wording) and the evidence behind both, `vendor/jai/modules/Default_Metaprogram.jai` — its `case "-…"` labels in pass 1 (lines ~65–99, plugins and the `Check` switches) and pass 2 (lines ~112–308) are the definition, and `HELP_STRING` at the end of that file is the help text `-help` must print. Names, arity, argument order, effects and error strings are copied from there; never invent a single-dash name, an alias or a `--long` form of one. `oj`'s own options are double-dash and clap-owned (`--help`, `--version`, `--tree`, `--file-only`, `--proc`, `--llvm`) and may be added freely. A lone `-` ends option processing and sends the rest to `compile_time_command_line`; `oj run`'s `--` (program arguments) is split off before clap sees the line, since the jai-style option list accepts hyphenated values.


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
