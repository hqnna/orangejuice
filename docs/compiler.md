# Jai Compiler Implementation Specification (as of beta 0.2.009)

This document describes how the reference Jai compiler works, as far as can be reconstructed from the shipped modules (`Compiler`, `Preload`, `Runtime_Support`, `Default_Metaprogram`, `Metaprogram_Plugins`, `Jai_Lexer`, `Program_Print`, `Code_Visit`, `Check`), the `how_to/` documentation, the examples, `README.txt`, and the full `CHANGELOG.txt`. It is written as an implementation specification: an implementation that follows it will be observably compatible with the reference compiler from the point of view of Jai programs, metaprograms and the standard modules. `docs/language.md` is the companion language specification and is referenced as **L§n**. `docs/spec.md` maps this document onto the orangejuice code base.

---

## 1. Overview of the pipeline

```
jai [compiler options] file.jai [-metaprogram options] [- user args]
   │
   ▼
Default metaprogram workspace (modules/Default_Metaprogram.jai, or `--- meta X`)
   │   parses the command line, creates the TARGET workspace, sets Build_Options,
   │   registers plugins (Check by default), adds the source files, runs the message loop
   ▼
For each workspace (compiled concurrently by a thread pool):
   Preload + Runtime_Support + Default_Allocator are imported implicitly
   Lex → Parse → build scope tree (data scopes are unordered)
   Dependency-driven typecheck scheduler:
       resolve identifiers, evaluate #if, instantiate polymorphs, run #modify,
       resolve overloads, generate bytecode per procedure body,
       execute #run/#assert in the bytecode interpreter (may stall and resume),
       deliver messages to the metaprogram (FILE, IMPORT, TYPECHECKED, PHASE...)
   Dead-code elimination, bytecode inlining and deduplication
   Global data segments assembled; data reset (unless #no_reset)
   Backend: LLVM (bitcode → object files, possibly split into many modules) or x64 (direct object writing)
   Link: object files + Runtime_Support objects + libraries via the system/shipped linker (or a custom link command)
   COMPLETE message
```

Compilation is whole-program: modules are compiled from source every time; there is no incremental or separate compilation. The compiler is multi-threaded; `#run`s and workspaces run concurrently, so the order of independent `#run`s and of messages between unrelated declarations is nondeterministic. Reported throughput of the reference implementation is roughly 250,000 lines/second with the x64 backend (2021).

---

## 2. Command line and the default metaprogram

### 2.1 Invocation

`jai [compiler-level options] [--- | --] <files...> [metaprogram options] [- user args]`

The compiler itself understands very few options (all others belong to the metaprogram):

- `--- meta Module_Name` / `-- meta X`: use `Module_Name` as the metaprogram instead of `Default_Metaprogram` (`Minimal_Metaprogram` is the shipped minimal one).
- `--- import_dir Folder` (also `-- import_dir`): extra module directory, used so the metaprogram module itself can be found.
- `-version`, `-help` at the compiler level; `jai -- help` lists the developer options: `import_dir name`, `meta metaprogram_name`, `no_jobs`, `randomize`, `seed some_number`, `extra`, `chaos`.
- The delimiter of compiler-internal options is `---` or `--`; the Default_Metaprogram recognizes only `-` (a lone `-` starts the user arguments list) and ignores `--`.

Everything else, in order, is handled by the Default_Metaprogram (pass 1 collects plugin names; pass 2 handles options; non-dash arguments are source files):

| Option | Effect |
|---|---|
| `-` | remaining args → `Build_Options.compile_time_command_line` (user args; also visible to `#run`s) |
| `-plug NAME` / `-plugin NAME` (`NAME(param=value)` allowed) | load a metaprogram plugin module (`Program_Print`, `Iprof`, `Autorun`, `Icon`, `Codex`, `Polymorph_Report`, `Toolchains/Android`) |
| `-no_check`, `-no_check_bindings`, `-check_bindings` | control the default `Check` plugin (`Check` or `Check(CHECK_BINDINGS=false)` is added unless `-no_check`) |
| `-release` | `set_optimization(*options, .OPTIMIZED)`, `stack_trace = false` |
| `-very_debug` | `.VERY_DEBUG` |
| `-no_inline` | `enable_bytecode_inliner = false` |
| `-quiet` | `text_output_flags = 0` |
| `-x64` / `-llvm` | choose backend |
| `-no_cwd` | do not change the working directory to the first file's directory |
| `-no_dce` | `dead_code_elimination = .NONE` |
| `-no_split` | `llvm_options.enable_split_modules = false` |
| `-output_ir` | emit LLVM IR |
| `-debug_for` | `debug_for_expansions = true` |
| `-msvc_format`, `-natvis`, `-no_color`, `-no_backtrace_on_crash` | corresponding Build_Options |
| `-version` | print `compiler_get_version_info` and exit 0 |
| `-exe NAME` | `output_executable_name` |
| `-output_path P` | `output_path` |
| `-add "code"` | `add_build_string("code;")` into the target |
| `-run "expr"` | `add_build_string("#run expr;")` |
| `-debugger` | interactive bytecode debugger on compile-time crashes / `debug_break()` (also for the metaprogram) |
| `-context_size N` | `context_size_max` (≥ `size_of(Context_Base)`, ≤ 0x4_0000) |
| `-import_dir DIR` | prepend to `import_path` (relative to the first file's directory) |
| `-verbose`, `-help` / `-?`, `-ps5` | misc; unknown options are offered to plugins' `handle_one_option`, else "Unknown argument" and exit |


Options taking a value consume the next argument; `-help`/`-?` print `HELP_STRING` (the help text lives in `Default_Metaprogram.jai`, not in the compiler) and deliberately fall through so that module help prints too. Command-line diagnostics, all `log_error` followed by `exit(1)`:

| Situation | Message |
|---|---|
| `-exe`, `-output_path`, `-add`, `-run`, `-plug`/`-plugin`, `-context_size`, `-import_dir` without a value | `Command line: Missing argument to <option>.` |
| `-context_size` above `CONTEXT_SIZE_MAX` (`0x4_0000`) | `Command line: Invalid argument to -context_size. The context must be less than or equal to CONTEXT_SIZE_MAX, which is % (but the value provided was %). ...` |
| `-context_size` below `size_of(Context_Base)` | `Command line: Invalid argument to -context_size. The context must be at least as large as size_of(Context_Base), which is %.` |
| `-context_size` not an integer | `Command line: Unable to parse an integer argument to context size; got '%'.` |
| an option no plugin claims | `Unknown argument '%'.` then `Exiting.` |
| a plugin's `handle_one_option` returning a smaller index | `Plugin % decreased argument index. That is illegal!` |
| `-plug` consumed as another option's value (the two passes disagree) | `Plugins in pass 1 and pass 2 do not match, meaning that -plug was used as an argument to another option. This is an error.` |
| no files, `-add` or `-run` | `You need to provide an argument telling the compiler what to compile! Sorry. Pass -help for help.` |

Default_Metaprogram behavior: `#run,stallable build();` — sets `output_path` to the first file's directory (absolute), `output_executable_name` to its basename (extension appended unless `append_executable_filename_extension = false`; a directory separator in the name is rejected), calls `set_working_directory` to that directory (unless `-no_cwd`; since 0.0.046 the compiler changes to the first file's directory if it is not the CWD), makes file paths absolute, `set_build_options(options, w, loc)` with a fake location so that relative `import_path` entries resolve, then `plugin.before_intercept(&flags)` → `compiler_begin_intercept(w, flags)` → `plugin.add_source()` → `add_build_file`/`add_build_string` for each file / `-add` / `-run` → message loop (forwarding every message to each plugin's `message`) until `COMPLETE` → `compiler_end_intercept(w)` → `plugin.finish()` → `plugin.shutdown()` → `set_build_options_dc(.{do_output=false, write_added_strings=false})` for the metaprogram's own workspace.

The metaprogram is itself Jai code compiled in workspace 1 and executed by the bytecode interpreter; it never produces an executable. Workspace 2 is the target program by default. Multiple workspaces compile in parallel; the compiler exits when all are done. A workspace fails (status `.FAILED`, exit code non-zero) on any error, on linker failure, on an assertion/crash in compile-time code, or when the metaprogram sets `compiler_set_workspace_status(.FAILED)`; only the failed workspace and its children terminate — the metaprogram stays alive.

### 2.2 Program layout expected by the reference distribution

`jai/bin/jai-linux` (the compiler), `jai/modules/` (the standard module tree; `Preload.jai`, `Runtime_Support.jai`, `Default_Allocator.jai`, `Default_Metaprogram.jai`, `Compiler/`, `Basic/`, ...), `jai/how_to/`, `jai/examples/`, `jai/redist/` (redistributable libraries), `jai/editor_support/`. `compiler_get_base_path()` returns the directory containing `modules`. Programs may add a `modules` directory next to their first source file, which is searched first.

---

## 3. Workspaces and metaprograms

### 3.1 Workspace lifecycle

```
w := compiler_create_workspace("name");          // -> Workspace (s64; 0 on failure); a fresh copy of the DEFAULT Build_Options (not the parent's)
options := get_build_options(w);                 // (w = -1 means the current workspace)
copy_commonly_propagated_fields(get_build_options(), *options);   // optionally inherit optimization/check settings
... modify options ...
set_build_options(options, w, loc := #caller_location);   // legal only BEFORE any source is added; relative paths (output_path, intermediate_path, import_path) made absolute relative to loc
compiler_begin_intercept(w, flags := 0);         // optional; only before source is added; flags: Intercept_Flags
add_build_file("main.jai", w, loc := #caller_location);   // relative to loc.fully_pathed_filename
add_build_string("code", w, code := #code,null, loc);      // into the global scope, or the scope identified by a Code constant
add_build_string("code", w, message: *Message, loc);       // into the scope of a *Message_File (that file) or *Message_Import (that module); null = global scope
remap_import(w, host, import_name, replacement);           // before compilation; "" host = main program, "*" = all; replacement "" = block
while true { message := compiler_wait_for_message(); ...; if message.kind == .COMPLETE break; }   // never returns null; error if not intercepting
compiler_end_intercept(w);                       // clears pending messages; if the metaprogram leaves the loop early, the compiler exits
compiler_destroy_workspace(w);
set_build_options_dc(.{ ... }, w = -1);          // Build_Options_During_Compile: allowed while compiling, until PRE_WRITE_EXECUTABLE
compiler_set_workspace_status(.FAILED, w = -1);
```

Rules: changing `Build_Options` after source is added is an error ("workspace has already started running"); `output_executable_name` may not be empty when output is produced; a workspace with no source hangs `compiler_wait_for_message` (error since 0.1.038); `compiler_wait_for_message` without `compiler_begin_intercept` (or after all streams finished) is an error; `set_build_options_dc` accepts `do_output`, `write_added_strings`, `append_executable_filename_extension`, `interactive_bytecode_debugger`, `append_linker_arguments`, `output_executable_name`, `output_path`. Messages are generated only if a metaprogram intercepts (0.1.078); the compiler waits for the metaprogram to handle `PARSED`/`TYPECHECKED`-class messages before proceeding (back-pressure), so a metaprogram can inject code in response.

### 3.2 Messages

`Message { kind: enum u8 { UNINITIALIZED; FILE; IMPORT; FAILED_IMPORT; PHASE; TYPECHECKED; COMPLETE; DEBUG_DUMP; ERROR; PERFORMANCE_REPORT }; workspace: Workspace; }` — cast to the specific struct by kind:

| Kind | Struct | Contents / when |
|---|---|---|
| `FILE` | `Message_File { fully_pathed_filename; enclosing_import: *Message_Import; from_a_string: bool }` | once per loaded file (or added string), **before** any code from that file; the same pointer is stored on nodes as `Code_Node.enclosing_load` |
| `IMPORT` | `Message_Import { module_type: UNINITIALIZED / PRELOAD / RUNTIME_SUPPORT / MAIN_PROGRAM / FILE; module_name; fully_pathed_filename }` | once per module instantiation (per distinct parameter list), plus main program (`""`), Preload and Runtime_Support (Runtime_Support is reported twice: as RUNTIME_SUPPORT and as a FILE-type module) |
| `FAILED_IMPORT` | `Message_Failed_Import { status: BLOCKED / NOT_FOUND; host_module_name; target_module_name; import_code: *Code_Directive_Import }` | a module could not be found or was blocked; the metaprogram must call `provide_import(w, m, type, value)` (`SHORT_NAME`, `PATH_TO_FILE`, `PATH_TO_DIRECTORY`, `FULL_TEXT`; a blocked/failed import can be replaced only once) before the next wait |
| `PHASE` | `Message_Phase { phase: ALL_SOURCE_CODE_PARSED 0 / TYPECHECKED_ALL_WE_CAN 1 / ALL_TARGET_CODE_BUILT 2 / PRE_WRITE_EXECUTABLE 3 / POST_WRITE_EXECUTABLE 4 / READY_FOR_CUSTOM_LINK_COMMAND 5; executable_name; executable_write_failed: bool; linker_exit_code: s32; num_items_waiting_to_typecheck: s32; compiler_generated_object_files, support_object_files, system_libraries, user_libraries: [] string }` | phases may repeat when new code is added after `TYPECHECKED_ALL_WE_CAN` (`num_items_waiting_to_typecheck == 0` means the program will compile as is; > 0 means unresolved items — add source or fail). `READY_FOR_CUSTOM_LINK_COMMAND` arrives when `use_custom_link_command` is set; the metaprogram must run the linker and call `compiler_custom_link_command_is_complete(w)`. `POST_WRITE_EXECUTABLE` carries the object/library lists and the link result |
| `TYPECHECKED` | `Message_Typechecked { declarations: [] Typechecked(Code_Declaration); procedure_headers: [] Typechecked(Code_Procedure_Header); procedure_bodies: [] Typechecked(Code_Procedure_Body); structs: [] Typechecked(Code_Struct); others: [] Typechecked(Code_Node); all: [] Typechecked(Code_Node) }` with `Typechecked(T) :: struct { expression: *T; subexpressions: [] *Code_Node }` (subexpressions flattened in dependency order, including nodes from `#insert` expansions and `xx` casts) | a batch of things whose typechecking finished; only *toplevel* declarations are sent (struct/enum member declarations are not — inspect the struct); headers/bodies/structs are sent even when nested; macro and polymorphic *bodies* appear in `procedure_headers` (via `header.body_or_null`, untypechecked) and not in `procedure_bodies`; each concrete polymorph instantiation is sent as its own header/body; pseudo-declarations with empty `.name` exist for lambdas, anonymous structs/enums |
| `ERROR` | `Message` | an error was reported (the workspace will fail) |
| `COMPLETE` | `Message_Complete { error_code: NONE / COMPILATION_FAILED / COMPILER_SHUTDOWN }` | last message of a workspace |
| `DEBUG_DUMP` | `Message_Debug_Dump { dump_text }` | output of `#dump` |
| `PERFORMANCE_REPORT` | `Message_Performance_Report { time_report: { run_directives, bytecode_inlining, bytecode_generating: float64 }; run_directive_report: { reported; records: [] { callee; num_stalls; elapsed_time } }; polymorph_report: { num_solves_invoked; num_solves_used; reported; records: [] { source; num_call_sites; num_deduplications_by_constants; num_deduplications_by_bytecode; distinct_polymorphs: [] { polymorphed; call_site; location } } }; bytecode_report: { num_calls_inlined; num_calls_total; num_instructions_inlined; num_instructions_total } }` | with `Intercept_Flags.DO_PERFORMANCE_REPORT_POLYMORPHS / _RUNS` |

`Intercept_Flags :: enum_flags u32 { SKIP_EXPRESSIONS_WITHOUT_NOTES 1; SKIP_DECLARATIONS 2; SKIP_PROCEDURE_HEADERS 4; SKIP_PROCEDURE_BODIES 8; SKIP_STRUCTS 0x10; SKIP_OTHERS 0x20; SKIP_ALL; DO_PERFORMANCE_REPORT_POLYMORPHS 0x1000; DO_PERFORMANCE_REPORT_RUNS 0x2000 }` (SKIP_EXPRESSIONS_WITHOUT_NOTES filters root declarations with a name and no notes).

### 3.3 Compiler procedures (`#compiler`, callable only at compile time)

| Procedure | Purpose |
|---|---|
| `compiler_create_workspace(name := "") -> Workspace`, `compiler_destroy_workspace(w)`, `get_name(w = -1) -> string`, `get_current_workspace() -> Workspace` (Preload; 0 at runtime) | workspaces |
| `get_build_options(w = -1) -> Build_Options`, `set_build_options(options, w = -1, loc)`, `set_build_options_dc(options: Build_Options_During_Compile, w = -1)`, `set_optimization(*options, Optimization_Type, preserve_debug_info := true)`, `copy_commonly_propagated_fields(source, *dest)`, `get_toplevel_command_line() -> [] string`, `compiler_get_base_path() -> string`, `compiler_get_version_info(*Version_Info { major, minor, micro }) -> string` | options |
| `compiler_begin_intercept(w, flags := 0)`, `compiler_end_intercept(w)`, `compiler_wait_for_message() -> *Message` | messaging |
| `add_build_file(filename, w, loc)`, `add_build_string(text, w, code := #code,null, loc)`, `add_build_string(text, w, message: *Message, loc)` (internal name `add_build_string_scoped_by_message`), `remap_import(w, host_module_name, import_name, replacement_name)`, `provide_import(w, m: *Message_Failed_Import, type: Provided_Import_Type, value: string)` | source |
| `compiler_report(message: string, loc := #caller_location, mode := Report.ERROR)`, `compiler_report(filename, line, char, text, mode)` with `Report :: enum u8 { ERROR; ERROR_CONTINUABLE; WARNING; INFO }`, `compiler_report_errors_for_unresolved_identifiers(filename, w = -1)`, `compiler_report_errors_for_untyped_declarations_with_these_notes(w, labels: ..string)`, `compiler_set_workspace_status(status: Workspace_Status { OK; FAILED }, w = -1)` | diagnostics from metaprograms (`ERROR` stops the workspace at the next opportunity; `ERROR_CONTINUABLE` reports and continues) |
| `compiler_get_nodes(code: Code) -> (root: *Code_Node, expressions: [] *Code_Node)`, `compiler_get_code(node: *Code_Node, scope_source := #code,null) -> Code`, `get_root_type(code) -> (Get_Root_Type_Status { UNINITIALIZED; SUCCESS; INPUT_IS_CODE_NULL; CORRUPTED; NOT_TYPED }, Type)`, `compiler_modify_procedure(w, body: *Code_Procedure_Body)` (replace a body; known limitations with foreign names, `using`, nested procs/structs/macros), `compiler_make_procedure_live(w, header: *Code_Procedure_Header)` | AST |
| `compiler_get_struct_location(w, info: *Type_Info_Struct) -> Source_Code_Location`, `get_type_table(w = -1) -> [] *Type_Info`, `get_runtime_info(w = -1) -> Runtime_Info` (works at runtime through `__runtime_info: Runtime_Info #elsewhere`), `get_type(info: *Type_Info) -> Type`, `compiler_set_type_info_flags(T: Type, flags: Type_Info_Flags { NO_TYPE_INFO 1; PROCEDURES_ARE_VOID_POINTERS 2; NO_SIZE_COMPLAINT 4 })`, `is_subclass_of(info: *Type_Info_Struct, base_name: string, first_call := true) -> bool` (follows `members[0]` with `.AS`, through pointers), `make_location(node) -> Source_Code_Location`, `get_filename(node) -> string`, `get_basename_and_path(name) -> (basename, path)`, `make_integer_literal(v)`, `make_string_literal(s)` | introspection helpers |
| `add_global_data(data: [] u8, segment: Data_Segment_Index { WRITABLE 0; WRITABLE_NO_RESET 1; READ_ONLY 2; BSS 3; USER_SEGMENT 0x10 }, user_segment: *Data_Segment = null, w = -1) -> [] u8` (returns the address range the data will have in the target), `add_data_segment(section_name, characteristics := READ|WRITE (Data_Segment_Characteristics { READ 1; WRITE 2; EXECUTE 4; ZEROED 8 }), alignment: s32 = 16, w = -1) -> (segment: *Data_Segment, actual_segment_will_be_created: bool)` | global data injection |
| `compiler_custom_link_command_is_complete(w)`, `compiler_add_library_search_directory(path)` | linking |
| `compiler_set_memory_breakpoint(ptr)`, `developer_debug(*void)`, `compile_time_debug_break()`, `debug_break()` | debugging |
| `write_string`, `write_strings` (Runtime_Support, `#compiler`: synchronized with compiler output at compile time) | output |

Runtime-usable data: `Runtime_Info { type_table: [] *Type_Info; global_data_info: *Global_Data_Info { version_stamp: u64; segment_info: [] Global_Data_Segment_Info { segment_tag: BSS 0 / DATA 1 / RDATA 2 / NO_RESET 3 / USER 5; data: [] u8 } } }`.

### 3.4 Metaprogram plugins

`Metaprogram_Plugin :: struct { workspace: Workspace; before_intercept: (p: *Metaprogram_Plugin, flags: *Intercept_Flags); add_source: (p); message: (p, m: *Message); finish: (p); shutdown: (p); handle_one_option: (p, options: [] string, cursor: int) -> int; log_help: (p); }` — a module exposes `get_plugin() -> *Metaprogram_Plugin` (extending the struct with `#as using plugin: Metaprogram_Plugin`). `Metaprogram_Plugins.init_plugins(names, *plugins, w)` generates `Plugin_N :: #import "Name"(params); p_N := Plugin_N.get_plugin();` via `add_build_string` into its own module scope (identified by a `#code` constant), filling a `#placeholder PLUGIN_INIT_STRING` that `init_plugins_part_2` `#insert`s (blocking until filled). Plugins cannot `#add_context` (they are imported after the context is finalized).

Shipped plugins: `Check` (format-string and binding checks; default), `Program_Print` (dump AST as source to `pp_<file>.txt`; `#add_context program_print`), `Iprof` (instrumentation; `-modules`, `-csv`, `-min_size`), `Autorun` (runs the output), `Icon` (Windows icon), `Codex` (metrics database used by `examples/codex_view`), `Polymorph_Report`/`Performance_Report`, `Toolchains/Android`, `linux_build` (cross-link).

---

## 4. Build options reference

```
Build_Options :: struct {
    output_type: enum u8 { NO_OUTPUT; EXECUTABLE; DYNAMIC_LIBRARY; STATIC_LIBRARY; OBJECT_FILE; } = .EXECUTABLE;
    using Commonly_Propagated: struct {
        write_added_strings := true;                 // write .build/.added_strings_w<N>.jai for add_build_string text
        runtime_storageless_type_info := false;
        shorten_filenames_in_error_messages := false;
        use_visual_studio_message_format := false;
        use_natvis_compatible_types := false;
        minimum_os_version: struct { major, minor: u8; };   // macOS/iOS; darwin target triple
        lazy_foreign_function_lookups := false;
        disable_redzone := false;
        enable_bytecode_inliner := true;
        enable_bytecode_deduplication := true;
        stack_trace := true;                         // Stack_Trace_Node maintenance
        use_ansi_color := true;
        interactive_bytecode_debugger := false;
        debug_for_expansions := false;
        enable_frame_pointers := true;
        runtime_support_definitions: enum u8 { AUTO; ENTRY_POINT_AND_INIT; ONLY_INIT; OMIT; } = .AUTO;   // which Runtime_Support pieces this output defines (libraries: ONLY_INIT/OMIT to avoid duplicate symbols when linking several Jai outputs)
        backtrace_on_crash: enum u8 { OFF; ON; } = .ON;
        array_bounds_check: enum u8 { OFF; ON; ALWAYS; } = .ON;
        cast_bounds_check: enum u8 { OFF; NONFATAL; FATAL; } = .FATAL;
        null_pointer_check: enum u8 { OFF; ON; } = .ON;
        arithmetic_overflow_check: enum u8 { OFF; NONFATAL; FATAL; } = .OFF;
        dead_code_elimination: enum u8 { NONE; ALL; MODULES_ONLY; } = .MODULES_ONLY;
        max_bytecode_instructions_for_inlined_initializer: s32 = 8;
        context_size_max: s32 = 4096;
        prevent_compile_time_calls_from_runtime := false;
        info_flags: enum_flags u32 { POLYMORPH_MATCH :: 1; POLYMORPH_DEDUPLICATE :: 2; };
        text_output_flags: enum_flags u32 { OUTPUT_LINK_LINE :: 1; OUTPUT_TIMING_INFO :: 2; } = xx 3;
        os_target := OS;  cpu_target := CPU;         // .X64/.ARM64/.CUSTOM(WASM); cross-compilation supported
        backend: enum u32 { X64; LLVM; } = .LLVM;
        machine_options: [MACHINE_OPTIONS_SIZE] u8;  // Machine_X64.Machine_Options_X86 { features.leaves: [...] u32 } via get_machine_options_x86()
        emit_debug_info: enum u32 { NONE; DWARF; CODEVIEW; DEFAULT; } = .DEFAULT;
        maximum_polymorph_depth := 100;
        maximum_array_count_before_compile_time_returns_are_not_reflected_in_ast := 5000;
        x64_options: X64_Options { use_dlls := false; enable_register_allocation := false; enable_unix_runtime_frame_information := true; };
        llvm_options: Llvm_Options { bitcode_optimization_setting: { UNSET; O0; O1; O2; O3; OS; OZ }; machine_code_optimization_setting: { UNSET; NONE; LESS; DEFAULT; AGGRESSIVE }; function_sections := false; enable_tail_calls := false; enable_loop_unrolling := false; enable_slp_vectorization := false; enable_loop_vectorization := false; preserve_debug_info := true; merge_functions := false; disable_inlining := true; disable_mem2reg := false; enable_split_modules := true; output_bitcode_before_optimizations; output_llvm_ir_before_optimizations; output_bitcode; output_llvm_ir; target_system_triple: string; target_system_cpu: string; target_system_features: string; command_line: [] string; };
    }
    output_executable_name: string;   // keeps any extension given; extension appended per platform unless append_executable_filename_extension = false
    output_path: string;              // directory (relative to the CWD at the time; made absolute by set_build_options)
    intermediate_path: string;        // .build directory location
    entry_point_name: string;         // default "main"
    compile_time_command_line: [] string;
    append_executable_filename_extension := true;
    use_custom_link_command := false;
    temporary_storage_size: s32 = 32768;
    import_path: [] string;           // default: [ "<first file dir>/modules", "<compiler>/modules" ]
    additional_linker_arguments: [] string;
    user_data_u64: u64;  user_data_string: string;  user_data_pointer: *void;  user_data_pointer_size: s32;   // copied to child workspaces
}
Build_Options_During_Compile :: struct { do_output := true; write_added_strings := true; append_executable_filename_extension := true; interactive_bytecode_debugger := false; append_linker_arguments: [] string; output_executable_name: string; output_path: string; }
Optimization_Type :: enum u8 { DEBUG; VERY_DEBUG; OPTIMIZED; VERY_OPTIMIZED; OPTIMIZED_SMALL; OPTIMIZED_VERY_SMALL; }
```

`set_optimization` for non-debug levels sets bounds/cast/null/overflow checks OFF, frame pointers on (macOS/debug/preserve_debug_info), `enable_split_modules = false`, `x64_options.enable_unix_runtime_frame_information = false`, and LLVM bitcode/machine-code levels per level (`O2`/`DEFAULT` for OPTIMIZED, `O3`/`AGGRESSIVE` for VERY_OPTIMIZED, `OS`/`OZ` for the small variants); it leaves `stack_trace` on (set it to false manually for maximum speed). Bitcode optimization and machine-code optimization are independent (O1 bitcode + no machine-code optimization halves LLVM time). New workspaces get **default** options, not the parent's (since 0.1.043); `copy_commonly_propagated_fields` copies the `Commonly_Propagated` subset.

---

## 5. Front end

### 5.1 Lexer

The token set is specified by `modules/Jai_Lexer` (a port of the compiler's lexer, and the token numbering is authoritative, but see the two bugs below): `Token_Type :: enum s16` with ASCII single-character tokens as themselves, `IDENT 256`, `NUMBER 257`, `STRING 258`, then compound operators (`PLUSEQUALS 259 … TRIPLE_EQUALS 277`, `SHIFT_LEFT_EQUALS 290 … LOGICAL_OR_EQUALS 298`, `POINTER_DEREFERENCE 310`, `POSTFIX_DEREFERENCE 311`, then `ISEQUAL_FOR_SWITCH_STATEMENT`, `DOUBLE_MINUS`, `TRIPLE_MINUS`, `DOUBLE_COMMA`, `BEGIN_STRUCT_LITERAL`, `BEGIN_ARRAY_LITERAL`, keywords `KEYWORD_FOR … KEYWORD_INTERFACE`, `QUICK_LAMBDA`, `NOTE`, `END_OF_INPUT`, `POINTER_DEREFERENCE_OR_SHIFT_LEFT`, `OPERATOR_ARRAY_SUBSCRIPT 500`, `OPERATOR_ASSIGNMENT_TO_ARRAY_SUBSCRIPT 501`, `ERROR`). `Token { type; l0, c0, l1, c1: s32; union { ident_value { name; hash }; integer_value: u64; float64_value; string_value }; value_flags: { HERE_STRING 1; NUMBER 2; HEX 4; BINARY 8; FLOAT 0x10; REQUIRES_FLOAT64 0x10_0000; DEFAULTS_TO_FLOAT64 0x20_0000; REQUIRES_FLOAT64_DUE_TO_SIGNIFICANT_DIGITS 0x40_0000; OVERFLOWED 0x1000_0000 }; ident_is_backticked }`. Identifiers are interned (`Table_String`). The lexer keeps an 8-token ring buffer with 7 tokens of lookahead. Rules for numbers, strings, here-strings (handled in the lexer when it sees `#` `string`), comments, notes, backticks, backslash-in-identifier, hashbang and invisible Unicode are in **L§2**. Location information is (line, column) pairs `l0,c0`–`l1,c1` carried onto every AST node (`Code_Node.location`).

Two places where the shipped `Jai_Lexer` module disagrees with the compiler it was ported from. Both were established while a reference distribution was on hand, by lexing its whole module tree with each and by evaluating the literals with the reference compiler; orangejuice follows the compiler, not the module:

- **Exponents.** `make_number` binds `c8 := cast,no_check(u8) c` before eating the `e`, then reuses that stale `c8` after re-reading `c` (module lines 1041 and 1047), so `1.5e-3` lexes as `1.5` and `1.5e3` raises "'e' in a float literal must be followed by + or - or a numerical digit.". The compiler accepts both; 11 files of the reference module tree contained such literals (`Math/cephes.jai`'s `DP1`, `RadixSort.jai`'s `1.5e+4`, the `d3d1x` `FLT_MAX` constants, ...). The bug is upstream and was reproducible without orangejuice: the module's own `examples/lex_all_files.jai`, built and run with the reference compiler, died on that distribution's own `modules/Basic/tests.jai` after 151 files. Reading `c` afresh at both sites fixes it. orangejuice ships no `Jai_Lexer` module of its own — `oj-lexer` is the lexer, and what is recorded here is why it follows the compiler where the two disagreed.
- **Token order after `#`.** The `#` case calls `peek_next_token` to look for `string`, which composes the following token into ring slot 0 while the `#` itself lands in slot 1, so the module returns `import` before the `#` of `#import`. The compiler emits `#` first.

### 5.2 Parser

Hand-written recursive descent with precedence climbing for binary operators (0.1.082). Notable parsing rules:

- Declarations are recognized by `ident_list ':'`; a C-style declaration (`int x;`) produces a helpful parse error; `:=` vs `=` confusion has a dedicated message; a missing expression after `:` or `=` is an error.
- `<<` at expression start is a dereference; after an expression it is a shift; `<<<` and `>>>` are reassembled from `<<`/`>>` followed by `<`/`>`; `..` after a number literal is a range; `.{`/`.[` begin literals; a parenthesized type before `.[` (`(*u8).[...]`) parses as a typed array literal; `T.[...]`/`T.{...}` where `T` is a dotted path (`A.B.{}`) is supported; `[8]u8.[...]` and `*u8.[...]` parse as typed literals (0.0.100).
- `if x == {` begins a switch (`ISEQUAL_FOR_SWITCH_STATEMENT`); `case` is a keyword only inside; `#through` is a directive statement.
- Procedure headers: `(params) [-> returns] { directives } [#modify block] body`; directive order is free; `=>` after a parameter list or single identifier starts a quick lambda; a header followed directly by `->` and a body means implicit return types (lambda).
- `#if` inside procedure headers/struct parameters is forbidden; `#run`/`#assert` inside argument/return lists are forbidden.
- Semicolon rules: `ifx` used as an expression takes no semicolon of its own (a declaration ending in `ifx` needs one); a `;` after a block is an empty statement; empty statements are allowed; a `;` directly after `if cond` is an error; `#run {}` may or may not end with `;`.
- Parsing of the rejected branch of `#if` still happens (syntax must be valid); the AST keeps both branches, marking the accepted one.
- Notes are attached to the preceding declaration; struct/enum declarations may carry notes directly after the keyword.
- Struct bodies allow declarations, assignments (defaults), `#place`, `#if`, `#insert`, `using`, `#as`, nested `union {}`/`struct {}` blocks; enum bodies allow member constants, `#if`, `#insert`, `using`.
- Every AST node carries `kind`, `node_flags`, `type` (filled by typechecking), `location`, `serial`. Sizes were repeatedly shrunk (nodes are compact, 24+ bytes smaller than earlier betas).

Rules the reference does not document but that its own module tree forced, established by parsing all 702 of its files with orangejuice while a distribution was on hand:

- **`->` followed by `(`** always opens the *return list*, not a returned procedure type; only a `->` after the matching `)` makes it one. `f :: () -> (s32) #c_call { }` is a `#c_call` procedure returning one `s32`, and `-> (status: Get_Root_Type_Status) { }` is a named single return.
- **A procedure type inside a parameter list** ends its return list at the comma that separates parameters: in `(f: (idx: int) -> Key, $compare: (Key, Key) -> bool)` the `, $compare` belongs to the outer list. Multiple returns there need parentheses.
- **Directives after the parts they describe**: struct layout directives may follow the body (`} #no_padding;`), a member may carry `#align N` and `#elsewhere lib "symbol"` after its type (`x: u8 #align 64;`, `environ: *u8 #elsewhere libc "environ";`), and `#align` may also follow `= ---`.
- **`#assert(cond, "message")`** is accepted next to `#assert cond "message"`; `#run,host` marks a run that executes on the host when cross-compiling; `#no_alias` is accepted as a header directive.
- **`interface` is a keyword only in a type restriction** (`$T/interface I`); the generated bindings use it as an ordinary member name.
- **Semicolons may be omitted** after a statement whose value ends in a here-string terminator line, a braced `ifx`, a `#code { }` block, a `#module_parameters` common-code block, an anonymous `enum`/`union` behind `using`, or a `#insert -> T { }`.
- **A branch that starts with `*`, `-` or `.`** needs `then` (`ifx use_semaphores then *sync.image_available else null`), since the condition would otherwise absorb it.

### 5.3 AST (exported to metaprograms; `modules/Compiler/Compiler.jai`)

`Code_Node.Kind :: enum u8 { UNINITIALIZED 0; BLOCK 1; LITERAL 2; IDENT 3; UNARY_OPERATOR 4; BINARY_OPERATOR 5; PROCEDURE_BODY 6; PROCEDURE_CALL 7; CONTEXT 8; WHILE 9; IF 10; LOOP_CONTROL 11; CASE 12; RETURN 14; FOR 15; TYPE_DEFINITION 16; TYPE_INSTANTIATION 17; ENUM 18; PROCEDURE_HEADER 19; STRUCT 20; COMMA_SEPARATED_ARGUMENTS 21; EXTRACT 22; DIRECTIVE_BYTES 23; MAKE_VARARGS 24; DECLARATION 25; CAST 26; DIRECTIVE_IMPORT 27; DIRECTIVE_THIS 28; DIRECTIVE_THROUGH 29; DIRECTIVE_LOAD 30; DIRECTIVE_RUN 31; DIRECTIVE_CODE 32; DIRECTIVE_POKE_NAME 33; ASM 34; DIRECTIVE_BAKE 35; DIRECTIVE_MODIFY 36; DIRECTIVE_LIBRARY 37; EXPRESSION_QUERY 38; PUSH_CONTEXT 39; NOTE 40; DIRECTIVE_PLACE 41; DIRECTIVE_SCOPE 42; TYPE_QUERY 43; DIRECTIVE_LOCATION 44; DIRECTIVE_MODULE_PARAMETERS 45; DIRECTIVE_ADD_CONTEXT 46; DIRECTIVE_COMPILE_TIME 47; COMPOUND_DECLARATION 48; DEFER 49; USING 50; PLACEHOLDER 51; DIRECTIVE_INSERT 52; DIRECTIVE_PROCEDURE_NAME 53; DIRECTIVE_WILDCARD 54; DIRECTIVE_EXISTS 55; DIRECTIVE_CONTEXT_TYPE 56; RESOLVED_OVERLOAD 57; }`

`Code_Node { kind; node_flags: enum_flags u32 { EXPRESSION_IS_SPREAD 1; NO_ARRAY_BOUNDS_CHECK 2; ALLOWED_BY_CONTEXT 4; STATEMENT_IS_DEFERRED 8; IS_PARENTHESIZED 0x10; NO_ARITHMETIC_OVERFLOW_CHECK 0x80; CREATED_BY_DESUGARING 0x100 }; type: *Type_Info; #as using location: Location { enclosing_load: *Message_File; l0, c0, l1, c1: s32 }; serial: s64 }`

Node structs (all begin with `#as using base: Code_Node`):

- `Code_Block { parent; block_type: enum s32 { UNINITIALIZED; IMPERATIVE; DATA_DECLARATIONS; ARGUMENTS; RETURNS; STRUCT_ARGUMENTS; CONSTANTS }; block_flags: { NO_ARRAY_BOUNDS_CHECK 1; NO_ARITHMETIC_OVERFLOW_CHECK 2 }; belongs_to_struct: *Code_Struct; members: [] *Code_Scope_Entry; statements: [] *Code_Node; owning_statement }`; `Code_Scope_Entry { name; import_target }`.
- `Code_Literal { value_type: enum s16 { UNINITIALIZED; NUMBER 1; STRING 2; BOOLEAN 3; ARRAY 6; STRUCT 7; POINTER 8; TYPE_INFO 9 }; using values: union { _string; _float64; _s64; _u64; struct_literal_info: *Code_Struct_Literal_Info { type_expression: *Code_Type_Instantiation; arguments: [] *Code_Node }; array_literal_info: *Code_Array_Literal_Info { element_type: *Code_Type_Instantiation; alignment; array_members; array_literal_flags { WAS_UNTYPED } }; pointer_literal_info: *Code_Pointer_Literal_Info { union { global_symbol: *Code_Declaration; string_or_array_literal: *Code_Literal }; data_pointer: *u8; pointer_literal_type { GLOBAL_SYMBOL; STRING_OR_ARRAY_LITERAL_DATA_POINTER; CONSTANT_VALUE }; offset_from_symbol }; type_info_literal_defn: *Code_Type_Definition }; value_flags: { IS_A_NUMBER 1; HEX 2; FLOAT 4; BINARY 0x20; MINUS_SIGN 0x40; HERE_STRING 0x80; DEFAULTS_TO_FLOAT64 0x100; REQUIRES_FLOAT64 0x200 } }` (floats are stored as float64).
- `Code_Ident { name; resolved_declaration: *Code_Declaration; flags: { DEFINES_POLYMORPH_VARIABLE 1; IS_RHS_OF_DOT_DEREFERENCE 2; RESOLVES_ONLY_SHALLOWLY 4; GENERATED_BY_OPERATOR_OVERLOAD 8; DO_NOT_RESOLVE_DUE_TO_PARSER 0x10; CAN_RESOLVE_TO_REG 0x20; HAS_SCOPE_MODIFIER 0x40 } }`.
- `Code_Unary_Operator { operator_type: Operator_Type.loose; subexpression }`; `Code_Binary_Operator { operator_type; flags: { SHIFT_MARKED_AS_LOGICAL 1; SHIFT_MARKED_AS_SMALL 2 }; left; right }`. `Operator_Type :: enum s32`: ASCII for single-character operators; `IS_EQUAL 131; IS_NOT_EQUAL 132; LOGICAL_AND 133; LOGICAL_OR 134; LESS_OR_EQUAL 135; GREATER_OR_EQUAL 136; SHIFT_LEFT 137; SHIFT_RIGHT 138; ROTATE_LEFT 139; ROTATE_RIGHT 140; PLUS_ASSIGN 145; MINUS_ASSIGN 146; TIMES_ASSIGN 147; DIV_ASSIGN 148; MOD_ASSIGN 149; SHIFT_LEFT_ASSIGN 150; SHIFT_RIGHT_ASSIGN 151; ROTATE_LEFT_ASSIGN 152; ROTATE_RIGHT_ASSIGN 153; BITWISE_AND_ASSIGN 154; BITWISE_OR_ASSIGN 155; BITWISE_XOR_ASSIGN 156; LOGICAL_AND_ASSIGN 157; LOGICAL_OR_ASSIGN 158; POINTER_DEREFERENCE 168; POSTFIX_DEREFERENCE 169; ARRAY_SUBSCRIPT 500`. Member access is a binary `.` operator (right side an `IDENT` with `IS_RHS_OF_DOT_DEREFERENCE`); subscripts are binary `ARRAY_SUBSCRIPT`.
- `Code_Procedure_Call { procedure_expression; resolved_procedure_expression (header or struct); overloads: *[] *Code_Declaration (slot 0 = chosen); arguments_unsorted: [] Code_Argument { expression; name: *Code_Ident }; arguments_sorted: [] *Code_Node (parameter order; defaults filled); num_return_values_received; macro_expansion_block: *Code_Block; context_modification: *Context_Modification { modification_expressions }; flags: { INLINE_YES 1; INLINE_NO 2; RETURNS_PROCEDURE_POINTER_ONLY 0x10; NO_DEBUG 0x100; IS_MODULE_PARAMETERS 0x200 } }` (also used for struct instantiations and macro invocations); `Code_Make_Varargs { element_type; expressions; is_for_non_native_calling_convention }`; `Code_Extract { from; index }`; `Code_Resolved_Overload { result; source_expression }`.
- `Code_Procedure_Header { constants_block; arguments: [] *Code_Declaration; returns: [] *Code_Declaration; parameter_usings: [] *Code_Using; name; foreign_function_name; library_identifier: *Code_Ident; deprecation_string; polymorph_source_header; modify_directives: [] *Code_Directive_Modify; body_or_null: *Code_Procedure_Body (filled asynchronously); procedure_flags: { ELSEWHERE 1; COMPILE_TIME_ONLY 2; POLYMORPHIC 4; COMPILER_GENERATED 8; DEBUG_DUMP 0x10; C_CALL 0x20; TYPE_ONLY 0x40; INTRINSIC 0x80; DEPRECATED 0x100; SYNTACTICALLY_MARKED_AS_NO_CONTEXT 0x200; QUICK 0x400; QUICK_IN_BLOCK_FORM 0x800; CPP_METHOD 0x1000; NO_CALL 0x2000; MACRO 0x4000; NO_DEBUG 0x8000; ENTRY_POINT 0x1_0000; HAS_IMPLICIT_RETURN_VALUE 0x2_0000; SYMMETRIC 0x4_0000; CPP_RETURN_TYPE_IS_NON_POD 0x8_0000; ENTRY_POINT_HOOK 0x10_0000; SYNTACTICALLY_MARKED_AS_COMPILE_TIME 0x20_0000; SYNTACTICALLY_MARKED_AS_COMPILER 0x40_0000; SYNTACTICALLY_MARKED_AS_INLINE_YES 0x100_0000; SYNTACTICALLY_MARKED_AS_INLINE_NO 0x200_0000 }; notes: [] *Code_Note }`; `Code_Procedure_Body { block; header; body_flags: { ALREADY_MODIFIED 1 } }`.
- `Code_Declaration { (scope entry) name; type_inst: *Code_Type_Instantiation; expression; flags: { IS_CONSTANT 1; IS_MARKED_AS_AS 2; IS_MARKED_AS_DISCARD 4; IS_ITERATOR 0x10; NO_RESET 0x40; IS_UNINITIALIZED 0x80; COMES_FROM_ASM 0x100; IS_IMPORTED 0x200; AUTO_VALUE_BAKE 0x4000; AUTO_VALUE_BAKE_IS_REQUIRED 0x8000; MUST_BE_RECEIVED 0x10000; PROGRAM_EXPORT 0x20000; ELSEWHERE 0x40000; SCOPE_FILE 0x80000; IS_GLOBAL 0x100000; HAS_SCOPE_MODIFIER 0x200000 }; alignment_expression; notes; program_export_name: string }`; `Code_Compound_Declaration { comma_separated_assignment: *Code_Comma_Separated_Arguments { arguments: [] Code_Comma_Separated_Argument { node; modifier: NONE/DECLARE/ASSIGN } }; declaration_properties: *Code_Declaration; alignment_expression; notes; operator_type }`.
- `Code_Type_Instantiation { result: *Type_Info; type_valued_expression; must_implement (the `/R` restriction); pointer_to; type_directive_target; array_element_type; array_dimension; inst_flags: { VARARGS 1; RESIZABLE 2; TYPE_OF_PROCEDURE 4; TYPE_DIRECTIVE 8; TYPE_DIRECTIVE_DISTINCT 0x10; TYPE_DIRECTIVE_ISA 0x20; ARRAY_VIEW 0x40; INTERFACE 0x80 } }`; `Code_Type_Definition { info: *Type_Info }`; `Code_Type_Query { query_kind: SIZE_OF/TYPE_INFO/INITIALIZER_OF; type_to_query }`; `Code_Expression_Query { query_kind: TYPE_OF/IS_CONSTANT/CODE_OF; expression_to_query }`.
- `Code_Struct { modify_directives; block; arguments_block (unsupplied arguments); constants_block; notes; textual_flags; alignment: s32; defined_type: *Type_Info_Struct }`; `Code_Enum { internal_type_inst; internal_type; external_type: *Type_Info_Enum; block; notes; marked_as_complete; marked_as_specified; is_flags }`.
- `Code_Cast { target_type; expression; cast_flags: { IS_IMPLICIT 1; IS_POINTER_DEREFERENCE 2; IS_AUTO 4; NO_BOUNDS_CHECK 8; TRUNCATE 0x10; AS 0x20; ISA 0x40; FORCE 0x80; VERY_FORCE 0x100; HAS_DEREFERENCE 0x400; HAS_FUNCTION_SYNTAX 0x800; HAS_POSTFIX_SYNTAX 0x1000 } }`.
- `Code_If { condition; then_block; else_block; if_flags: { IS_SWITCH_STATEMENT 1; IS_IFX 2; IS_STATIC 4; MARKED_AS_COMPLETE 8 }; static_if_flags: { EVALUATED_AS_TRUE }; static_if_accepted_case }`; `Code_Case { condition; then_block; owning_if; marked_as_fallthrough }`; `Code_While { condition; block }`; `Code_For { iteration_expression; iteration_expression_right; block; ident_it; ident_it_index; ident_decl; index_decl; want_replacement_for_expansion; want_pointer_expression; want_reverse_expression; macro_expansion_procedure_call; for_flags: For_Flags }`; `Code_Loop_Control { control_type: BREAK/CONTINUE/REMOVE; target_ident }`; `Code_Return { arguments_unsorted; arguments_sorted; return_flags: { IS_BACKTICKED 1; AUTO_INSERTED_FOR_QUICK_LAMBDA 2 } }`; `Code_Defer { block; is_backticked }`; `Code_Using { expression; filter_type: NONE/ONLY/EXCEPT/MAP; filter_expression; no_parameters }`; `Code_Push_Context { to_push; block; push_context_flags: { IS_BACKTICKED 1; DEFER_POP 2 } }`; `Code_Context {}`.
- Directives: `Code_Directive_Run { procedure: *Code_Procedure_Header; flags: { ASSERTION 1; STALLABLE 2; SYNTACTICALLY_IMPLICIT 4; HAS_IMPLICIT_RETURN_TYPES 8 }; assertion_string }`; `Code_Directive_Code { expression; code_flags: { IMPLICITLY_GENERATED 1; NULL 2; TYPED 8 } }`; `Code_Directive_Insert { expression; scope_redirection; break_replacement; continue_replacement; remove_replacement; expansion; is_internal }`; `Code_Directive_Import { name; flags: { UNSHARED 1 }; import_type: Provided_Import_Type; module_parameters_call; program_parameters_call }`; `Code_Directive_Load { short_name; fully_pathed_filename; loaded_string; load_flags }`; `Code_Directive_Library { name; library_flags: { IS_SYSTEM_LIBRARY 1; DYNAMIC_LIBRARY_UNAVAILABLE 2; STATIC_LIBRARY_UNAVAILABLE 4; LINK_ALWAYS 8 } }`; `Code_Directive_Bake { procedure_call; bake_type: CONSTANTS_BAKE/PARAMETER_VALUE_BAKE/DYNAMIC_SPECIALIZE }`; `Code_Directive_Modify { block }`; `Code_Directive_Scope { scope_type: EXPORT/FILE/INTERNAL }`; `Code_Directive_Module_Parameters { module_parameters: *Code_Procedure_Header; program_parameters; common_code: *Code_Block }`; `Code_Directive_Location { expression; is_caller_location }`; `Code_Directive_Place { ident }`; `Code_Directive_Poke_Name { module_struct: *Code_Struct; name }`; `Code_Directive_Add_Context { expression }`; `Code_Directive_Context_Type {}`; `Code_Directive_Procedure_Name { argument }`; `Code_Directive_Exists { query_expression; sync_expression }`; `Code_Directive_Wildcard { index: s32 = -1 }`; `Code_Directive_Bytes { expression }`; `Code_Directive_Through {}`; `Code_Directive_This {}`; `Code_Directive_Compile_Time {}`; `Code_Placeholder {}`; `Code_Asm { opaque }`; `Code_Note { text; note_flags }`.

`modules/Code_Visit` enumerates the child pointers of every kind (used by metaprograms to walk trees); `modules/Program_Print` prints any node back to source and documents the exact surface syntax for each construct.

### 5.4 Desugaring performed by the front end

- `a op= b` → `a = a op b` (lvalue evaluated once; `CREATED_BY_DESUGARING`); operator overloads → procedure calls; `!=` → `!(==)`; `for` loops over native arrays → index loops; `for_expansion` → macro expansion; `push_context`, `defer`, `using` → scope entries; `#location`/`#caller_location` → `Source_Code_Location` struct literals; string comparisons → a dedicated bytecode instruction; `ifx` implicit then/else; `#run` → an anonymous procedure (`Code_Directive_Run.procedure`); `#assert` → `#run` with `ASSERTION`; `#insert -> T {}` → `#run` with `SYNTACTICALLY_IMPLICIT`; quick lambdas → polymorphic procedures with `AUTO_INSERTED_FOR_QUICK_LAMBDA` returns; struct initializers → compiler-generated `#no_context` procedures (`COMPILER_GENERATED`); array element fills → `__element_duplicate`; multi-return receivers → `Code_Extract`; varargs → `Code_Make_Varargs`; auto-dereference/`#as`/`isa`/implicit numeric casts → `Code_Cast` nodes; `Any` boxing; `context.x` accesses → `Code_Context` + member ops.

---

## 6. Typechecking and the dependency scheduler

### 6.1 Scheduling model

Every declaration, procedure body, struct, `#run`, `#if`, `#import`, `#insert`, `using`, `#placeholder` is a *work item*. The typechecker runs items in any order; an item that needs something not yet known (an identifier whose declaration is not typechecked, a struct size, a polymorph result, a `#run` value, a placeholder) *yields* and is re-queued when the dependency becomes available. Multiple threads process items concurrently. Consequences implementations must reproduce:

- Order independence of data scopes (**L§4.2**), including forward references and mutual recursion between procedures, structs referring to themselves through pointers/arrays, and `#run`s that depend on later declarations.
- **Circular dependency** detection: when no progress is possible, the compiler reports cycles (including through `#run` bodies, struct parameters, `context`, `#module_parameters`+`#if`, self-polymorphing procedures) with the chain of waiting items; undeclared identifiers are reported in bulk (sorted, aggregated across files, with placeholders and near-misses listed) rather than as false cycles.
- Name-inserting constructs (`using`, `#import`, `#insert`, compound declarations, `#if` that declares names) participate in lookup waiting: a lookup in a scope that contains an unfinished importing construct waits for it *if* that construct could provide the name; compound declarations only block names they define; `#insert` without a corresponding `#placeholder` does not block later lookups (which may then see the shadowed declaration — the documented reason to use `#placeholder`). Two `#if`s in one data scope that declare what the other reads deadlock — users must add `#placeholder`s.
- `#run`s execute when their procedure's dependencies are resolved; a `#run` that needs a not-yet-typechecked declaration blocks (stalls) the interpreter thread; deadlocks between un-stallable `#run`s are errors; `#run,stallable` participates in a priority system that allows one stallable run to wait on another.
- Dead procedures (DCE) are never body-typechecked; macros and polymorphic sources are header-typechecked only.
- Messages to the metaprogram are batched as items finish (`TYPECHECKED`), and the compiler waits for the metaprogram before continuing.
- After everything runnable ran, `TYPECHECKED_ALL_WE_CAN` is sent with `num_items_waiting_to_typecheck`; if the metaprogram adds code, typechecking resumes and the phase repeats.

### 6.2 Type inference

Bottom-up per statement with the Match operation and the downward exceptions of **L§5.10**. Integer literals carry "unhardened" values until matched; comparisons between integer constants consider their sizes; float constants keep float64 precision until hardened; `xx` nodes always get a target type. Casting distance drives overload resolution (**L§7.5**). Errors quote both types ("Type mismatch. Type wanted: int; type given: string.") plus site information ("in checking argument 2 of call to array_add", "Info: While generating a polymorph of this procedure.", "The polymorph was triggered here:", "Here is the site that caused the polymorph"), and print procedure types in detail including `#c_call`.

### 6.3 Polymorph solving and instantiation

1. For a call to a polymorphic procedure (or a struct instantiation), gather the call-site argument types (with `xx` stripped, literals unhardened, auto-deref/`#as` considered) and the header's parameter type expressions.
2. Iteratively solve `$` variables by structural matching (pointer levels, array kinds/dimensions, struct arguments incl. nested `S($A, $B)` through pointers, procedure types incl. return types of passed procedures, `type_of(other_param)`), binding each variable once; verify restrictions (`/R`, `/interface`, array-of-Types lists; `int`≡`s64`, `float`≡`float32`); bake `$`/`$$` values (must be constants; quick lambdas passed as arguments are polymorphed against the parameter type).
3. Run `#modify` (a synthesized procedure executed in the interpreter) with the solved variables as mutable values; on `false` discard the candidate with its reason.
4. Look up the (procedure/struct, constants) pair in the program-wide polymorph cache; if present reuse; else clone the AST (`polymorph_source_header`, notes and enums copied), create the constants block, and typecheck the clone as a new item.
5. After bytecode generation, hash the bytecode; instantiations with identical bytecode are merged (`enable_bytecode_deduplication`).
6. Depth limit `maximum_polymorph_depth`. Statistics are reported in `Polymorph_Report`.

### 6.4 Overload resolution

As in **L§7.5**: candidates are all visible declarations of the name (lexical scopes, `using`, unnamed imports); each candidate is scored (exact 0 < `#as`/`isa`/`using` casts < literal conversions < numeric widening < polymorphism (1 point per polymorphic parameter) < varargs); `#modify`/restriction failures exclude; unique minimum wins; ties or no match are errors that print argument types and per-candidate matching scores. Overload sets are never sealed, but a later-added overload that would change an earlier resolution is an error. `#bake_*` on a set is an error. A `#foreign` procedure may be overloaded by a Jai procedure of the same name with different arity (the OpenXR enumerate wrappers).

### 6.5 Static conditionals

`#if` conditions are constant-evaluated by the interpreter (or folded) as soon as their operands are known; the untaken branch is discarded from typechecking but retained in the AST (`static_if_accepted_case`). `#if` inside struct bodies affects member layout; inside enums, member values; inside procedure bodies, statements and declarations (no scope). `#ifx` similarly for expressions.

### 6.6 Constants and constant folding

Constant expressions (**L§5.11**) are folded at typecheck time with exact integer semantics (overflow detected, sizes respected in comparisons, no implicit sign extension of hex literals), IEEE semantics for floats (float64 precision retained), pointer arithmetic on constant pointers, string/array literal indexing, and casts with range checking (constant cast errors). `#run` results are substituted as constants once available; `type_info(T)` is a constant pointer into the type table; `#location` is a literal.

---

## 7. Compile-time execution

### 7.1 Bytecode

Every procedure body (live ones) is compiled to a register-style bytecode independent of the backend. The bytecode is the input to (a) the interpreter for compile-time execution, (b) the bytecode inliner and deduplicator, (c) the machine-code backends. Documented properties: small instruction set with typed loads/stores, calls with explicit argument/return slots, `memcpy` with constant size lowered to a fixed-size instruction, `memset(0, const)` specialized, string comparison instruction, `DUPLICATE_ARRAY_ELEMENTS` for array fills (skipped for 1 element), constant-pointer-math instruction, `&&`/`||` without branches when statically unnecessary, per-basic-block caching of constant loads, a sentinel stack-trace node, compare-and-jump forms, `a_or_zero`; instructions are printable (`#dump`, `Message_Debug_Dump`), including `#asm` opcodes.

### 7.2 Interpreter

The interpreter executes bytecode inside the compiler process with full access to the compiler's memory: compile-time global variables live in real memory; `#foreign` calls go through the loaded dynamic libraries (`dlopen`) with argument marshalling per the C ABI, including callbacks from C into bytecode, `#c_call` via function pointers, `#cpp_method`, varargs; `#asm` blocks are assembled to native code and called; `#compiler` procedures dispatch into the compiler; `debug_break()` enters the interactive bytecode debugger (`-debugger`; commands `u/up`, `d/down`, `dump`, `l/list`, `codes`, `L/List`, hex memory dumps) or reports a user-level stack trace and fails the build; array/cast/null/overflow checks and `assert` behave as at runtime, and their messages are identical; an execution can *stall* when it touches an unresolved declaration (the thread yields to the scheduler, including inside callbacks from C); `Stack_Trace_Node`s are maintained for `#compiler` built-ins and pushes/pops across `push_context` and returns; each `#run` is assigned to a workspace and reports its polymorph history on errors.

### 7.3 Results of compile-time execution

Return values are converted to AST literals as in **L§12.1**: scalars, strings (data copied into the compiler's literal storage), `Type`s (looked up in the type table), procedures (non-foreign only; foreign pointers are not mappable), pointers (mapped back to global symbols / literal data if the address lies in a known data segment or the read-only literal area; heap pointers warn), structs (recursively; unions/overlaps copied as bytes), arrays (large arrays mapping to a known segment are referenced, not copied), `Code`, `#code,null`, `#type,isa` variants, uninitialized `Type` → void (workaround). `#run` results larger than the AST threshold stay in data segments.

### 7.4 Global data during compilation

Globals are allocated in compiler-managed *data segments* (writable, writable-no-reset, read-only, BSS, user segments) that mirror the final executable's layout; `#run`s read and write them directly. Before writing the executable the writable segment is restored from a backup taken before any `#run` ran (so `#run` side effects vanish) except `#no_reset` declarations. Pointers stored in data are relocated: constant pointers to globals and to literal data are remapped; pointers stored via `#no_reset` are not. `add_global_data` returns the eventual runtime address range; `add_data_segment` creates named sections (platform permitting).

---

## 8. Middle end

- **Dead code elimination**: reachability from the entry point, `#program_export`s, `#run`s, and metaprogram-marked live procedures; `MODULES_ONLY` keeps all main-program procedures; dead bodies are neither typechecked nor compiled.
- **Bytecode inliner**: inlines calls marked `inline` (declaration or call site), respecting `no_inline`, `#asm` (not inlinable), `#compile_time`, `#discard`; debug info for inlined frames is preserved (DWARF/CodeView inline records).
- **Deduplication**: identical bytecode bodies (typically polymorphs differing only in pointer types) are merged; string literals are deduplicated at write time; some struct literals are deduplicated.
- **Struct initializers**: emitted inline when ≤ `max_bytecode_instructions_for_inlined_initializer` instructions, else as callable `#no_context` procedures; static default data is memcpy-able.
- **Calling convention lowering**: hidden context pointer, hidden return pointers, "big" arguments by pointer, varargs arrays, C ABI marshalling for `#c_call` (System V: struct classification INTEGER/SSE/MEMORY, sret for large returns, non-power-of-two sizes handled, `#cpp_return_type_is_non_pod` uses sret, XMM save/restore, stack alignment), `#no_context`, `#no_call` (naked).
- **Checks insertion**: bounds/cast/null/overflow checks as calls to the Runtime_Support failure procedures (which also work inside `#no_context`/`#c_call` code).

---

## 9. Backends

### 9.1 LLVM (default)

LLVM 19.1.7 (0.2.009; earlier 11 → 15 → 16 → 17), with LLD as the shipped linker and libclang for `Bindings_Generator`. The compiler translates bytecode to LLVM IR per procedure, emitting: integer/float constants as constants (needed for intrinsics with constant operands), a data layout matching the front end's sizes/alignments, `alloca`s for locals (`mem2reg` unless disabled), calls with the Jai convention as ordinary `fastcc`/C calls with explicit hidden parameters, `#c_call` with the platform C ABI, `#intrinsic "llvm.*"` as direct intrinsic calls, `#asm` as inline machine code that respects flag modifications, `#bytes` as raw code, frame pointers per `enable_frame_pointers`, red zone per `disable_redzone`, tail calls / loop unrolling / vectorization / function merging per `Llvm_Options`, `.eh_frame` unwind info, DWARF debug info (types, globals at start of BSS, inlined frames, for-loop by-value structs, `Any` termination, linkage name = human-readable name so `break procname` works), function sections optionally, and a *split modules* mode (`enable_split_modules`, default true, disabled for optimized builds/static libraries/object output) that partitions the program into many LLVM modules compiled in parallel. Bitcode and IR can be dumped before/after optimization. `llvm_options.command_line` feeds `cl::ParseCommandLineOptions`. Cross targets: `target_system_triple` / `target_system_cpu` / `target_system_features` (`"+lse"` for ARM64, `"+bulk-memory"` for WASM); WASM64 (`os_target = .WASM`, `cpu_target = .CUSTOM`) requires `function_sections = true`, no split modules, `--stack-first -z stack-size=N` linker args, a `walloc.o` allocator and a replaced `Default_Allocator`; iOS/Android via `.IOS`/`.ANDROID` with NDK sysroots and custom link commands.

### 9.2 x64 (native)

A fast direct code generator for x86-64 (Windows, Linux, macOS Intel): no optimization, optional rudimentary register allocation (`enable_register_allocation`, default off for debuggability; uses all 16 GPRs when on), one return per function, memory clears via XMM/GPR moves, fixed-size memcpy through registers up to 64 bytes, Stack_Trace_Node updates in 3 instructions, DWARF (Unix) or CodeView (Windows) debug info generated directly, `.pdata`/`.xdata` unwind info on Windows, `enable_unix_runtime_frame_information` (loads frame info at startup so `backtrace()` works), cast/bounds checks, `#asm` integrated natively. Object files are written directly (COFF/ELF/Mach-O writers; see `modules/executable_formats`). Debug builds with x64 are much faster to produce than LLVM builds.

---

## 10. Runtime support, entry point and outputs

- `Runtime_Support` is imported with module parameters derived from `runtime_support_definitions`: `DEFINE_SYSTEM_ENTRY_POINT` (emit `main`/`__system_entry_point`), `DEFINE_INITIALIZATION` (emit `__jai_runtime_init/fini`, first-thread context, temporary storage of `TEMPORARY_STORAGE_SIZE` bytes, type table pointer), `ENABLE_BACKTRACE_ON_CRASH`. `.AUTO` chooses ENTRY_POINT_AND_INIT for executables and ONLY_INIT for libraries. The compiler defines `TEMPORARY_STORAGE_SIZE`, `MACHINE_OPTIONS_SIZE`, `OS`, `CPU`, `IS_CROSS_COMPILING` as constants visible to these modules.
- Startup sequence (Linux): the C entry `main(argc, argv)` (`__system_entry_point`, `#program_export "main"`) → `__jai_runtime_init(argc, argv)` (stores `__command_line_arguments`, initializes the first `#Context` via `initializer_of(#Context)`, sets `context_info`, allocator, logger, temporary storage) → `push_context` → `Runtime_Support_Crash_Handler.init()` if enabled → `__instrumentation_first()` / `__instrumentation_second()` (empty unless a plugin such as Iprof fills them) → `__program_main()` (the user's `main`, bound by `#entry_point`/`entry_point_name`, called with `no_inline`) → `__jai_runtime_fini` → exit code 0. Runtime_Support does not depend on libpthread on Linux/Android; the default allocator (rpmalloc) uses `mmap`; `exit()` and `write` use raw syscalls.
- Output types: `EXECUTABLE` (linked with `crt1.o`/`crti.o`/`crtn.o`, dynamic linker, `-lc`, `-export-dynamic`, `-rpath '$ORIGIN'`), `DYNAMIC_LIBRARY` (`lib<name>.so`; Jai DLLs can be linked into Jai executables — type tables and literals are handled — and remap the caller's context via `Remap_Context`), `STATIC_LIBRARY` (`ar`/`lib.exe`; result in `executable_write_failed`/`linker_exit_code`), `OBJECT_FILE` (single object in `output_path`; split modules disabled), `NO_OUTPUT`. `output_executable_name` keeps a user-given extension; the platform extension is appended otherwise (none on Linux).
- The `.build` directory (`intermediate_path`) holds object files (named per workspace/module split), `.added_strings_w<N>.jai` (the text passed to `add_build_string`, with line numbers matching error messages; written only if non-empty and `write_added_strings`), LLVM bitcode/IR dumps, and the PDB on Windows (deleted before linking).
- Debug info: `emit_debug_info` `.DEFAULT` → DWARF on Unix, CodeView on Windows; includes types (structs with `#place`, enums signed via SLEB128, `Any` as its own type, `Code`), globals, parameters, locals in subscopes, inlined procedures, `#asm` line info per statement, for_expansion variables (`it`/`it_index` renamed), if-case blocks, macros (suppressed with `#no_debug`); `.natvis`/`.natstepfilter` support files ship in `editor_support/msvc`.

---

## 11. Linking on Linux

The compiler drives LLD (shipped) with a command line equivalent to the `linux_build` plugin's:

```
ld.lld --build-id --gdb-index -g --nostdlib
       -L<sysroot>/usr/lib -L<sysroot>/usr/lib64 [library search dirs from compiler_add_library_search_directory and #library paths]
       crt1.o crti.o crtn.o --dynamic-linker /lib64/ld-linux-x86-64.so.2 --eh-frame-hdr -export-dynamic -rpath='$ORIGIN'
       <compiler_generated_object_files> <support_object_files>
       --start-group <user libraries (.a/.so from #library)> <system libraries (-lc, -lpthread, ... from #system_library)> --end-group
       <additional_linker_arguments> <append_linker_arguments>
       -o <output_path>/<output_executable_name>
```

Library resolution: `#library "x"` → `libx.so` next to the declaring file (or `x.so`/`libx.a`; `,no_dll` forces the static archive; `,no_static_library` forces the shared one); `#system_library "x"` / `#library,system "x"` → `-lx` searched in `/etc/ld.so.conf` paths (plain name; `.so.N` suffixes auto-detected; 32-bit files skipped; `libc` is not auto-extended to `libc.so.6`); libraries used only by dead declarations are dropped (`,link_always` keeps them); shared libraries are also `dlopen`ed at compile time for `#run`s (failure reports the import chain). Custom linking: set `use_custom_link_command`, wait for `READY_FOR_CUSTOM_LINK_COMMAND` (which carries the object/library lists), run your linker, call `compiler_custom_link_command_is_complete(w)`. The link line is echoed when `text_output_flags.OUTPUT_LINK_LINE` is set; timing info with `OUTPUT_TIMING_INFO`; the workspace is `.FAILED` on non-zero linker exit.

---

## 12. Diagnostics

Errors, warnings and info are written to stderr with ANSI colors (unless `-no_color`), a highlighted source excerpt (multi-line ranges supported), a location `file:line,column`, and optional "Info:" follow-ups (polymorph chains, import chains, macro expansion sites, "used before its declaration", "redeclared identifier", "Declaration claims to be constant, but its expression is not", "Loss of information (trying to fit N bits into M bits). Can't do this without an explicit cast." with wanted/given types, "Number signedness mismatch. Type wanted: X; type given: Y.", "Number sizes don't match", "cannot implicitly coerce to bool", "Attempt to use a variable from an outer stack frame", "closures are not supported", "Mismatching levels of indirection", "Compile-time variable 'T' is needed but was not specified", "Not all control paths return a value" (warning), "Attempt to call a compile-time function at runtime", "Dynamically-computed strings do not cast to *u8; only literals do.", "Cannot take the address of a small constant", "memory base must be a gpr", "Incorrect number of arguments supplied to '%': The format string requires N arguments, but M arguments are given." (Check plugin), "Unknown argument", "This program is only meant to be run at compile-time."). `use_visual_studio_message_format` switches to `file(line,col): error:` style; `shorten_filenames_in_error_messages` trims paths. `compiler_report` lets metaprograms emit the same formats. Undeclared identifiers are batched and sorted per file. Statistics (lines, procedures, timing) print at the end unless `-quiet`; on failure statistics are suppressed.

The cast diagnostics of **L§5.6**, measured against the reference: `Casting a non-zero-sized value to void is invalid.`, `Cannot cast from a pointer to a non-fixed array type.`, `Cannot cast from one struct to another without force modifiers. Type wanted: B (8 bytes); type given: A (8 bytes)`, `This cast has inconsistent modifiers. (It is flagged both 'trunc' and 'no_check').`, `String cannot cast to this type (the target type is s64.)`.

---

## 13. Standard module conventions the compiler relies on

- `Preload` declarations named in **L§17** are referenced by the compiler by name and layout (Type_Info structs, Allocator, Context_Base fields, Stack_Trace_Node, Temporary_Storage, Any_Struct, Newstring, Array_View_64, Resizable_Array, `__reg`, For_Flags, Source_Code_Location).
- Runtime_Support hook names (`__array_bounds_check_fail`, `__cast_bounds_check_fail`, `__null_pointer_check_fail`, `__arithmetic_overflow`, `__panic_due_to_runtime_call_of_compile_time_procedure`, `__element_duplicate`, `__jai_runtime_init`, `__jai_runtime_fini`, `__system_entry_point`, `__program_main`, `__instrumentation_first/second`, `write_string`, `write_strings`, `runtime_support_default_logger`, `runtime_support_assertion_failed`, `runtime_support_default_allocator_proc`, `TEMPORARY_STORAGE_SIZE`) are fixed.
- `Basic.print` is an overload set rather than one procedure: `print :: print_to_builder;` stands beside the format-string form, so a call whose first argument is a `*String_Builder` appends to it instead of writing to standard output.
- `Default_Allocator.allocator_proc` is the heap; `Basic.temp`/`__temporary_allocator` is the arena; `Basic` adds `print_style` to the context; `Compiler` types (`Build_Options`, messages, `Code_*`) are "@Volatile with compiler_settings.h" — their layouts are shared with the compiler binary and must match exactly.
- The `Check` plugin's format checks rely on the `@PrintLike`/`@ScanLike` notes, on `Code_Make_Varargs.expressions.count`, `arguments_sorted`, `EXPRESSION_IS_SPREAD`, `call.overloads[0]`, and on the print format grammar (`%`, `%N`, `%0`, `%00`, `%%` warned, `\%`).

---

## 14. Performance characteristics worth preserving

- Whole-program compile of large games in seconds; x64 debug builds are the fast path; LLVM builds are dominated by LLVM time (mitigated by split modules and by separate bitcode/machine-code optimization levels).
- Stack-trace maintenance costs ~3 instructions per call; leaf procedures skip it; `-release` disables it.
- Per-scope arena memory in the compiler; interned identifiers; AST nodes kept small; identifier lookup via hash tables; `//` comment and identifier lexing optimized; `#run` launch overhead minimized; `#run`/`#assert` on constants skip execution; polymorph dedup by constants then by bytecode.
