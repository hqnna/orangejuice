//
// Runtime_Support: what generated code calls into (**L§17**, **C§13**).
//
// The compiler emits calls to the procedures here without anyone importing the
// module: an out-of-bounds subscript calls `__array_bounds_check_fail`, the
// generated entry point calls `__jai_runtime_init` for the `#Context` it hands
// the program, and `Context_Base` is the head every `#Context` begins with.
// The signatures marked @Volatile are the ones the compiler emits against.
//
// orangejuice's own distribution. Written against `docs/language.md` §10.3,
// §10.4, §10.5 and §17.
//

#scope_module

Linux :: #import "Linux";

#scope_export

// ------------------------------------------------------------- the context ---

// @Volatile: the layout must match what the compiler builds a `#Context` from
// (**L§10.3**). Members a program adds with `#add_context` follow this.
Context_Base :: struct {
    context_info: *Type_Info_Struct;
    thread_index: u32;

    allocator := default_allocator;

    logger                := runtime_support_default_logger;
    logger_data:             *void;
    log_source_identifier:   u64;
    log_level:               Log_Level;

    temporary_storage: *Temporary_Storage;
    stack_trace:       *Stack_Trace_Node;

    assertion_failed := runtime_support_assertion_failed;
    handling_assertion_failure := false;

    default_allocator :: Allocator.{runtime_support_default_allocator_proc, null};
}

// ------------------------------------------------------- temporary storage ---

// @Volatile: the compiler knows this layout.
//
// A bump arena reset wholesale rather than freed piecemeal. When a request
// does not fit in what is left, a page is taken from `overflow_allocator` and
// chained onto `overflow_pages`; resetting gives those back and restores the
// original block.
Temporary_Storage :: struct {
    data: *u8;
    size: s64;

    current_page_bytes_occupied: s64;
    total_bytes_occupied:        s64;
    high_water_mark:             s64;
    last_set_mark_location:      Source_Code_Location;

    overflow_allocator: Allocator;

    overflow_pages: *Overflow_Page;
    original_data:  *u8;
    original_size:  s64;

    Overflow_Page :: struct {
        next:      *Overflow_Page;
        allocator: Allocator;
        size:      s64;
    }
}

set_initial_data :: (ts: *Temporary_Storage, count: s64, data: *u8) #no_context {
    ts.data = data;
    ts.size = count;
    ts.original_data = data;
    ts.original_size = count;
}

// ------------------------------------------------------------- the output ---

// @Volatile: `#compiler` because compile-time code writes through the
// compiler's own stdout rather than the program's (**C§3.3**). The body is
// what a program that is *running* uses — a `#compiler` procedure that has one
// keeps it in the executable and is only intercepted while compile-time code
// is what is executing.
write_string :: (s: string, to_standard_error := false) #no_context #compiler {
    write_string_unsynchronized(s, to_standard_error);
}

write_strings :: (strings: ..string, to_standard_error := false) #no_context #compiler {
    for strings  write_string_unsynchronized(it, to_standard_error);
}

// What `write_string` comes to in a program that is running rather than being
// compiled. A short write is not an error, so this loops until the whole
// string is out or the descriptor stops accepting it.
write_string_unsynchronized :: (s: string, to_standard_error := false) #no_context {
    if !s.count  return;

    fd: s32 = ifx to_standard_error then cast(s32) Linux.STDERR_FILENO else cast(s32) Linux.STDOUT_FILENO;
    written := 0;
    while written < s.count {
        n := Linux.write(fd, s.data + written, cast(u64) (s.count - written));
        if n <= 0  break;
        written += n;
    }
}

// The digits of a number, written without allocating: the check-failure
// procedures run when the program is already in trouble and must not depend on
// the allocator or on temporary storage.
write_number :: (n: s64, to_standard_error := false) #no_context {
    if n < 0 {
        write_string("-", to_standard_error);
        write_nonnegative_number(cast(u64) -n, to_standard_error);
    } else {
        write_nonnegative_number(cast(u64) n, to_standard_error);
    }
}

write_nonnegative_number :: (n: u64, to_standard_error := false) #no_context {
    // 20 digits holds any u64.
    buffer: [20] u8;
    value := n;
    at := buffer.count;
    if value == 0 {
        at -= 1;
        buffer[at] = #char "0";
    }
    while value > 0 {
        at -= 1;
        buffer[at] = cast(u8) (#char "0" + value % 10);
        value /= 10;
    }
    text: string;
    text.data = buffer.data + at;
    text.count = buffer.count - at;
    write_string(text, to_standard_error);
}

c_style_strlen :: (str: *u8) -> s64 #no_context {
    if !str  return 0;
    count := 0;
    while str[count]  count += 1;
    return count;
}

to_string :: (str: *u8) -> string #no_context {
    result: string;
    result.data = str;
    result.count = c_style_strlen(str);
    return result;
}

// ---------------------------------------------------------------- failures ---

// The head every check failure prints: where in the program it happened.
// `filename` is a C string because the call site has no `string` to hand over.
write_location :: (line_number: s64, filename: *u8) #no_context {
    write_string(to_string(filename), true);
    write_string(":", true);
    write_number(line_number, true);
    write_string(": ", true);
}

// @Volatile: emitted by a subscript the compiler could not prove in range.
__array_bounds_check_fail :: (index: s64, limit: s64, line_number: s64, filename: *u8) #no_context {
    write_location(line_number, filename);
    write_string("Array bounds check failed. (The attempted index is ", true);
    write_number(index, true);
    if limit > 0 {
        write_string(", but the highest valid index is ", true);
        write_number(limit - 1, true);
        write_string("). Array index out of bounds.\n", true);
    } else {
        write_string(", but the array has no elements). Array index out of bounds.\n", true);
    }
    runtime_support_die();
}

// @Volatile: emitted by a cast whose value the compiler could not prove fits.
__cast_bounds_check_fail :: (pre_value: s64, pre_flags: u32, post_value: s64, post_flags: u32, fatal: u8, line_number: s64, filename: *u8) #no_context {
    write_location(line_number, filename);
    write_string("Cast bounds check failed. Number must be in [", true);
    write_number(low_bound(post_flags), true);
    write_string(", ", true);
    write_number(high_bound(post_flags), true);
    write_string("]; it was ", true);
    write_number(pre_value, true);
    write_string(".\n", true);
    if fatal  runtime_support_die();
}

// @Volatile: emitted by a dereference the compiler could not prove non-null.
__null_pointer_check_fail :: (index: s64, line_number: s64, filename: *u8) #no_context {
    write_location(line_number, filename);
    write_string("Null pointer check failed. (Argument number ", true);
    write_number(index, true);
    write_string(" is null.)\n", true);
    runtime_support_die();
}

// @Volatile: emitted by arithmetic the compiler was asked to check.
__arithmetic_overflow :: (left: s64, right: s64, type_code: u16, line_number: s64, filename: *u8) #no_context {
    write_location(line_number, filename);
    write_string("Arithmetic overflow.\n", true);
    runtime_support_die();
}

// @Volatile: a `#compile_time` procedure that survived into the executable.
__panic_due_to_runtime_call_of_compile_time_procedure :: (line_number: s64, filename: *u8) #no_context {
    write_location(line_number, filename);
    write_string("Attempt to call a compile-time-only procedure at runtime.\n", true);
    runtime_support_die();
}

// @Volatile: fills an array literal's remaining elements from its first one.
__element_duplicate :: (start: *u8, num_elements: s64, size: s64) #c_call #no_aoc {
    at := start + size;
    count := 1;
    while count < num_elements {
        memcpy(at, start, size);
        at += size;
        count += 1;
    }
}

#scope_module

runtime_support_die :: () #no_context {
    __runtime_support_disable_stack_trace = true;
    Linux.exit(1);
}

// The bounds a cast's target type admits, decoded from the flag word the
// compiler passes: bit 0 says the type is signed, and the low byte of the
// upper half says how many bits wide it is.
CAST_FLAG_SIGNED :: 0x1;

cast_bits :: (flags: u32) -> s64 #no_context {
    bits := cast(s64) ((flags >> 8) & 0xff);
    return ifx bits then bits else 64;
}

low_bound :: (flags: u32) -> s64 #no_context {
    if !(flags & CAST_FLAG_SIGNED)  return 0;
    bits := cast_bits(flags);
    if bits >= 64  return S64_MIN;
    return -(cast(s64) 1 << (bits - 1));
}

high_bound :: (flags: u32) -> s64 #no_context {
    bits := cast_bits(flags);
    if flags & CAST_FLAG_SIGNED {
        if bits >= 64  return S64_MAX;
        return (cast(s64) 1 << (bits - 1)) - 1;
    }
    if bits >= 64  return S64_MAX;
    return (cast(s64) 1 << bits) - 1;
}

S64_MIN :: -9_223_372_036_854_775_808;
S64_MAX ::  9_223_372_036_854_775_807;

#scope_export

// ----------------------------------------------------------- the defaults ---

// @Volatile: `Context_Base.assertion_failed` starts as this. Returning true
// asks the caller to `debug_break()`; this one never returns at all.
runtime_support_assertion_failed :: (loc: Source_Code_Location, message: string) -> bool {
    write_string(loc.fully_pathed_filename, true);
    write_string(":", true);
    write_number(loc.line_number, true);
    write_string(",", true);
    write_number(loc.character_number, true);
    write_string(": Assertion failed", true);
    if message.count {
        write_string(": ", true);
        write_string(message, true);
    }
    write_string("\n", true);
    runtime_support_die();
    return false;
}

// @Volatile: `Context_Base.logger` starts as this. Errors go to stderr, and a
// message that does not end in a newline gets one (**L§10.5**).
runtime_support_default_logger :: (message: string, data: *void, info: Log_Info) {
    to_standard_error := (info.common_flags & .ERROR) != 0;
    write_string(message, to_standard_error);
    if message.count == 0 || message[message.count - 1] != #char "\n" {
        write_string("\n", to_standard_error);
    }
}

// @Volatile: `Context_Base.allocator` starts as this.
//
// A thin wrapper over the C library's heap. The reference ships a stripped
// rpmalloc here; this is the same interface over `malloc`, which is enough for
// a program that has not asked for anything better.
runtime_support_default_allocator_proc :: (mode: Allocator_Mode, requested_size: s64, old_size: s64, old_memory: *void, allocator_data: *void) -> *void {
    if #complete mode == {
        case .ALLOCATE;
            if requested_size <= 0  return null;
            return Linux.malloc(cast(u64) requested_size);

        case .RESIZE;
            if requested_size <= 0 {
                Linux.free(old_memory);
                return null;
            }
            return Linux.realloc(old_memory, cast(u64) requested_size);

        case .FREE;
            Linux.free(old_memory);
            return null;

        case .CAPS;
            caps := Allocator_Caps.FREE | .ACTUALLY_RESIZE | .MULTIPLE_THREADS | .HINT_I_AM_A_GENERAL_HEAP_ALLOCATOR;
            return cast(*void) caps;

        case .STARTUP;      #through;
        case .SHUTDOWN;     #through;
        case .THREAD_START; #through;
        case .THREAD_STOP;
            return null;

        case .CREATE_HEAP;   #through;
        case .DESTROY_HEAP;  #through;
        case .IS_THIS_YOURS;
            return null;
    }
}

// A hook the compiler answers while compile-time code is running, so a
// debugger can be attached to it (**L§17**).
compile_time_debug_break :: () #compiler #no_context;

// Stops in a debugger. `int3` is the breakpoint instruction on x86-64; a
// program running without a debugger attached takes SIGTRAP and dies, which is
// what asking to break means.
debug_break :: () #no_context {
    #asm { int3; }
}

// --------------------------------------------------------- starting up ---

#scope_module

// The context the program runs in, and the storage behind it. These are
// globals rather than locals of `__jai_runtime_init` because the pointer it
// returns has to outlive the call (**C§13**).
runtime_support_context: #Context;
runtime_support_temporary_storage: Temporary_Storage;

TEMPORARY_STORAGE_SIZE :: 32768;
runtime_support_temporary_storage_data: [TEMPORARY_STORAGE_SIZE] u8;

#scope_export

// @Volatile: the generated entry point calls this with `argc` and `argv` and
// hands what comes back to the program as its context (**C§13**).
__jai_runtime_init :: (argc: s32, argv: **u8) -> *#Context #c_call {
    __command_line_arguments.count = cast(s64) argc;
    __command_line_arguments.data  = argv;

    ts := *runtime_support_temporary_storage;
    set_initial_data(ts, TEMPORARY_STORAGE_SIZE, runtime_support_temporary_storage_data.data);

    // Every member of a `#Context` declared with a default has one, including
    // the ones modules added with `#add_context` (**L§10.2**) — the allocator
    // and the logger from `Context_Base`, `Basic.print_style` after them. A
    // declaration without a value is what applies them (**L§4.6**), so the
    // context starts as one of those rather than as the zeroed global.
    fresh: #Context;
    runtime_support_context = fresh;

    ctx := *runtime_support_context;
    ctx.context_info      = type_info(#Context);
    ctx.temporary_storage = ts;
    return ctx;
}

// @Volatile: called after `main` returns (**C§13**).
__jai_runtime_fini :: (_context: *void) #c_call {
}
