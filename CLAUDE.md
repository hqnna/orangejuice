# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repository is

orangejuice (`oj`) is a cleanroom reimplementation of the **Jai** programming language (reference beta 0.2.009) in Rust nightly, targeting **Linux x86_64 only**, with a single LLVM 19 backend via `inkwell` and LLVM ORC JIT for compile-time execution.

**Current state: M9 (FFI, `#asm`, threads) is in; M8 (the metaprogram) is most of the way in behind it.** The nix flake, the cargo workspace and the sixteen crates of `docs/spec.md` §4 exist. `oj version`, `oj help`, every `oj dump` stage and `oj build`/`oj run` work: a typechecked program is lowered to `oj-ir`, compiled through inkwell/LLVM 19 and linked into a native executable that runs, and its `#run`s and `#modify`s execute along the way. Filled in so far: `crates/cli` (the clap CLI), `crates/testsupport` (vendor discovery), `crates/diag` (spans, source map, diagnostics and their reference-format rendering), `crates/source` (memory-mapped loading, module and `#load` path resolution), `crates/lexer` (tokens, the lexer, the name interner, the token dump), `crates/syntax` (the AST arena, the parser, the source and tree printers), `crates/scope` (the scope tree, the program loader, the `#if` constant folder, name resolution and the scope dump), `crates/types` (the type table, layout and type printing), `crates/sema` (declaration types, struct/enum/variant construction, constants, Match, overload resolution, polymorph solving and instantiation, macro expansion, `for_expansion`, bakes, body checking, `#run`/`#assert`/`#modify`, and the query API the back end asks its questions through), `crates/ir` (the typed IR, lowering, the type table image and the `oj dump ir` listing), `crates/codegen` (the LLVM module, target machine and object emission), `crates/runtime` (the compile-time data segments), `crates/jit` (the ORC `LLJIT` and the engine that runs a `#run` or a `#modify`), `crates/link` (library resolution and the link line), `crates/driver` (build options, the pipeline and the workspaces a metaprogram creates) and `crates/meta` (the `#compiler` procedures, the message structs and the compile-time state a metaprogram works on).

What M7 added: a `$`-marked header is solved at each call site and instantiated per constant set, deduplicated program-wide; `#modify` runs at compile time and may accept, reject or change a specialization; a `#expand` macro is spliced into the block it was called from, seeing the caller's locals, with `` `x `` for the caller's scope and `` `return `` for the caller's return; `for_expansion` iterates a container of one's own with the loop body handed over as `Code` and `#insert`ed; polymorphic structs bake down to a type per argument set; quick lambdas, `#caller_location`, `#caller_code`, `#bake_arguments` and `#bake_constants` work, and a polymorphic procedure takes the shape of the concrete one it is given to. On the way it also brought `..T`/`..Any` varargs and `..xs` spreads, `using` of a value, pointer arithmetic, string equality, and a `Type` that survives compile-time execution.

What M8 has added so far: an `#insert` of a string is parsed and spliced into the scope it was written in — a block, a file, a struct body where it becomes members, an enum body where it becomes values — which is why `oj-scope`.s tree and unit list now grow behind a shared reference. `#compiler` procedures are answered by Rust procedures in `oj-meta`, bound by the JIT to the convention `oj_ir::abi_of` describes, so a metaprogram creates workspaces, reads and sets their `Build_Options`, adds files and strings to them, remaps or blocks their imports, chooses their output type, reports its own diagnostics and watches one compile through `compiler_wait_for_message`. What M8 still owes is the `Code_*` export of **C§5.3** and everything downstream of it — `TYPECHECKED` messages, `compiler_get_nodes`, the `Check` plugin — plus `Default_Metaprogram` as the driver, which M9 unblocked by assembling the `#asm` that importing `Compiler` reaches.

What M9 added: an `#asm` block is parsed rather than kept as text, declares its registers into the scope it stands in, places them itself — lifetime-based, no spilling, pinned where the block or the instruction's encoding requires it — and hands the back end Intel-syntax assembly that already names them, so LLVM's integrated assembler is all that is left to do (**L§15**). A variable of the *program* stays the back end's to place, which is what leaves a nine-register syscall block registers to work with. `oj-types` classifies a value the System V way, so a `#foreign` procedure taking or returning a struct by value is called correctly rather than reported, and a `#c_call` procedure is a callback a C library can call back into. **`how_to/001_first` and `how_to/900_inline_assembly` both build, run and print what the reference prints**, and nothing in the `how_to` suite reports M9 or M10 any more. On the way it also brought `initializer_of`, the `#Context` `Runtime_Support.__jai_runtime_init` builds — which is what the generated `main` now hands the program, and what a `#run` gets too — threads, the File and Socket modules, `pkg-config` for a `#library,system` name, and `#import "Compiler"`, which used to stop at the `#asm` in `Thread` and `Basic`.

What is left is M7's remaining corners, which `#asm` was hiding: `assert(x, "format %", x)` — a macro over `..Any` the back end cannot type — operator overloading on `Apollo_Time`, a labelled `break`, `operator []`, and a macro taking a `__reg`. Those, plus M8's `Code_*` export, are what the 38 `how_to` programs that do not build report. A construct a later milestone owns is reported as an error *naming that milestone*, never mis-compiled; `crates/ir/tests/lower.rs` asserts that over every `how_to` program.


**An instantiation is a key, not a copy.** A `$`-marked header is solved by unifying the parameter types with the argument types, and then read *again* with the bindings in place — that is what turns `$T` into a real type in a signature. Everything the checker memoizes about a declaration written inside a polymorphic body is keyed by the instantiation it was resolved in, so one body serves every specialization without the AST being duplicated. A macro is the same machinery with the *call site* in the key, since it expands once per site rather than once per constant set; a polymorphic struct is the same again with its arguments scope as the root; and a `#modify` runs under an instantiation that gives its variables types but no values, because inside that block they are values rather than constants. The back end generates one procedure per instantiation and none for a macro: a macro's body is spliced into whatever it expanded into, with that procedure's locals keyed by the expansion so two sites never share one.

Three rules keep the back end honest. IR generation starts at `main` — or at a `#run` — and follows calls, so only reachable code is lowered (**L§11.6**), which is what keeps a module's uninstantiated polymorphic procedures out of the back end. The calling convention is *one* function, `oj_ir::abi_of`, that a call site and a body both build from: a hidden trailing `*#Context`, aggregates by pointer, the first scalar return in registers and every other return through caller storage — except across a `#c_call` boundary, where `oj_types::classify` says which System V registers an aggregate travels in. And a compile-time module names its symbols after the declarations they came from rather than after a collision counter, because every `#run` and `#modify` of a compilation shares one JIT dylib. `docs/spec.md` §10 lists the deviations in full — the generated `main`, the `__oj_global_init` that runs unfoldable global initializers, aggregates as byte arrays in LLVM, linking through the C driver rather than `ld.lld`, and the checker-driven (rather than scheduled) order of `#run`s.

**Compile-time execution is part of typechecking.** Asking for a `#run`'s value builds and runs it then and there: `oj-sema` decides what the run produces, `oj_ir::lower_run` lowers it into a program of its own wrapped in a generated `void __oj_run_N(void *result)`, `oj-codegen` builds that in the JIT's own context, and `oj-jit` calls it and reads the answer back. A run whose expression already folded never gets that far. A run gets the same `#Context` the program does, built by `Runtime_Support.__jai_runtime_init`, so what it calls can allocate and print. Globals a run touches live in `oj-runtime`'s data segments, keyed so that two runs share one `x`; `#no_reset` is the one whose bytes the driver copies into the executable. The type table is one self-referential image whose field offsets are read out of the program's own Preload rather than written down in Rust, so `type_info(T)`, `Any` and a runtime `Type` are all addresses into it.

The lexer follows the reference *compiler*; where the shipped `Jai_Lexer` module disagrees with it, **C§5.1** says which of the two is right. `crates/lexer/tests/corpus.rs` lexes all 702 vendor files, and setting `OJ_TOKEN_STREAM_DIR` makes it write a per-file cross-check dump to compare against a `Jai_Lexer`-based dumper.

The AST mirrors `Code_Node.Kind` (**C§5.3**); `crates/syntax/tests/corpus.rs` parses the same 702 files and checks that each one *round-trips* — printing the tree and re-parsing it must give an identical tree, compared through the span-free tree form. That test is the parser's and the printer's contract with each other: whenever one of them learns a construct, the other has to. `crates/syntax/src/rules.rs` holds the one rule they share, which statements need a `;`.

`oj-scope` turns a root file into the scope tree of **L§4.2**: Preload at the root, a module scope per instantiation under it, a file scope per `#load`ed file, then struct/enum/procedure/block scopes. Declarations land in the file or the module scope according to the `#scope_*` directive in effect; `#import` and `using` are recorded as *edges* rather than copied names, so overload sets and `using,except(…)` filters come out right. `crates/scope/tests/corpus.rs` resolves all 702 files on their own and then every top-level `how_to` program whole. Two rules make that possible before the scheduler exists (both in `docs/spec.md` §10): a `#if` nobody can decide yet contributes *every* branch as conditional declarations with its diagnostics held back, and a lookup that misses in a scope holding an unfinished name-inserting construct is a wait rather than an error.


`oj-types` holds the interned type table: pointer, array and procedure types are structural, structs, enums, variants and `$T` variables nominal (**L§3.13**), and `LayoutBuilder` puts members where **L§3.14** and **L§8.6** say. `oj-sema` types every declaration of a resolved program on demand — asking for one resolves what it depends on first, which is what makes data scopes order-independent without the scheduler of `docs/spec.md` §6.2. A struct's *identity* is a separate step from its *body*, so a pointer or a procedure signature never waits for a layout; the only circular dependency left to report is a struct that contains itself. Anything needing a milestone the front end has not reached is the `unknown` type, which converts to everything and is never reported, so nothing complains about the metaprogram's half of the language (M8). `crates/sema/tests/corpus.rs` types all 702 vendor files on their own and every `how_to` program whole, and pins 17 standard-module struct sizes to values measured with the reference compiler.

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

Rust toolchain, LLVM and every tool come from the nix flake; `cargo`/`rustc` are not on `PATH` outside it. The dev shell sets `RUSTFLAGS` to an rpath over LLVM's dynamic dependencies (libffi, zlib, libxml2, ncurses, libstdc++), because nothing else puts them on a runtime search path and `oj` links `llvm-sys`; that is what lets `./target/release/oj` run outside the shell too. Linking a compiled program needs a C driver on `PATH`, which the shell provides.

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

Exit codes: 0 success, 1 compile/link failure, 2 usage error. Env: `OJ_JAI_DIR`, `OJ_LOG` (tracing filter), `OJ_THREADS`. `oj build` writes the object into `<output_path>/.build/<name>.o` and the executable beside it, then prints its path; `oj run` executes it instead and passes its exit code on. The options `oj-driver` acts on so far are `-release`, `-very_debug`, `-quiet`, `-verbose`, `-exe`, `-output_path`, `-import_dir`, `-no_color`, `-msvc_format`, `-output_ir`, `-version` and the lone `-`; the ones that need the metaprogram or the interpreter (`-plug`, `-add`, `-run`, `-context_size`, `-debugger`, the `Check` switches) parse with the right arity and then say which milestone owns them.

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
- **The metaprogram is the driver, not a plugin.** `Default_Metaprogram.jai` is compiled from the vendor tree and JIT-executed on its own thread; it creates the target workspace and sets `Build_Options`. orangejuice reimplements the *argument parsing* in Rust but still forwards to the real module. That last step is M8's remaining work; M9 unblocked it by assembling the `#asm` that `#import "Compiler"` reaches. What works now is the machinery under it — a metaprogram creates workspaces, from the distribution's own `Compiler` module or from headers it declares itself, and each one is a compilation of its own that the driver runs. A metaprogram that watches a workspace gets its messages by *replaying* the compilation, which happens inside the first `compiler_wait_for_message` rather than on another thread, so there is no back-pressure: source added in response to a message is not seen by the compilation that produced it.
- **`oj-meta` is an ABI, not an API.** `vendor/jai/modules/Compiler/Compiler.jai` is compiled unmodified; the Rust `#[repr(C)]` mirrors must match its layouts byte for byte, and `crates/meta/tests/layout.rs` measures the real structs with the compiler's own front end and compares every offset and size. A `#compiler` procedure is an `extern "C"` procedure built to `oj_ir::abi_of` — a non-scalar return through storage handed in first, a non-scalar argument by pointer, a trailing `*#Context` — that the JIT binds the symbol to; `crates/meta/tests/intrinsics.rs` checks that every symbol answered is one the distribution declares. Nothing about `Build_Options` is written down in Rust: a fresh one is what the declaration's own member defaults fold to, and the members the driver acts on are found by name at the offsets the checker computed.
- **JIT instead of an interpreter** is the one documented semantic deviation (`docs/spec.md` §6.5). The reference stalls a `#run` mid-execution on an unresolved declaration; orangejuice instead computes the `#run`'s transitive dependency closure and schedules it only when complete. `-debugger` is a stack-trace dump, not an interactive debugger.
- **Compile-time and runtime share bytes.** Globals in JIT code live in compiler-owned data segments (`oj-runtime`); the same bytes are written into the executable at the end, writable data restored from a pre-`#run` backup except `#no_reset`, pointers remapped by recorded relocation tables.
- **Diagnostics wording is a tested interface.** Error text matches the reference wherever **C§12** or **L** documents it; every documented message gets a negative test asserting wording and location.
- **The back end asks the front end questions rather than reading its notes.** `oj-sema`'s `query` module is the whole interface `oj-ir` has: the type and constant value of an expression, the declaration a node introduced, a call site's chosen callee with its arguments in parameter order, a struct's member defaults, a loop's `it`. Lowering walks the same trees the checker did and asks as it goes, which is why the two never disagree about what a name means — and why teaching the checker something new (a `for` iterator's type, say) is what makes the back end able to lower it.

## Conventions

- **Style**: 2-space indentation everywhere (Rust via `rustfmt.toml` `tab_spaces = 2`, plus Nix, Markdown, JSON, TOML). Avoid comments in code — names and tests carry the meaning; doc comments only on public crate APIs where a name cannot.
- **Dependencies**: prefer well-maintained crates over hand-rolled code (`clap`, `inkwell`, `cc`, `pkg-config`, `libloading`, `memmap2`, `rayon`/`crossbeam`, `indexmap`, `smallvec`, `insta`, `object`).
- **Commits**: conventional commits scoped by crate (`feat(lexer): …`, `fix(sema): …`, `docs: …`), one logical change each, gate above passing first.
- **No functionality lands without unit tests** in the owning crate.
