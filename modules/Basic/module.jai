//
// Basic: what a program gets for importing one module (**L§10.4**, **L§10.5**).
//
// Allocation through the context, a temporary arena reset in a batch, the
// logger, assertions, and `print`. Everything here goes through
// `context.allocator` rather than the C heap directly, so a program that
// replaces the allocator replaces it for this module too.
//
// orangejuice's own distribution.
//

#load "Print.jai";
#load "Array.jai";
#load "Simple_String.jai";
#load "String_Builder.jai";

#module_parameters (ENABLE_ASSERT := true);

#scope_module

Linux :: #import "Linux";

// `debug_break` and the check-failure procedures live there; **L§17** puts
// them in Runtime_Support rather than here.
Runtime_Support :: #import "Runtime_Support";

#scope_export

// ------------------------------------------------------------ allocation ---

alloc :: (size: s64, allocator := context.allocator) -> *void {
    if size <= 0  return null;
    proc := ifx allocator.proc then allocator.proc else context.allocator.proc;
    data := ifx allocator.proc then allocator.data else context.allocator.data;
    return proc(.ALLOCATE, size, 0, null, data);
}

// Zeroed storage, which is what a declaration without a value gets
// (**L§4.6**) and what most callers want.
alloc_zeroed :: (size: s64, allocator := context.allocator) -> *void {
    memory := alloc(size, allocator);
    if memory  memset(memory, 0, size);
    return memory;
}

realloc :: (memory: *void, new_size: s64, old_size: s64, allocator := context.allocator) -> *void {
    proc := ifx allocator.proc then allocator.proc else context.allocator.proc;
    data := ifx allocator.proc then allocator.data else context.allocator.data;
    return proc(.RESIZE, new_size, old_size, memory, data);
}

free :: (memory: *void, allocator := context.allocator) {
    if !memory  return;
    proc := ifx allocator.proc then allocator.proc else context.allocator.proc;
    data := ifx allocator.proc then allocator.data else context.allocator.data;
    proc(.FREE, 0, 0, memory, data);
}

// One `T`, zeroed unless the type has member defaults, in which case the
// initializer the compiler wrote fills them in (**L§17**).
New :: ($T: Type, $initialized := true, allocator := context.allocator) -> *T {
    memory := cast(*T) alloc_zeroed(size_of(T), allocator);
    #if initialized {
        if memory {
            info := cast(*Type_Info_Struct) type_info(T);
            if info.type == .STRUCT && info.initializer  info.initializer(memory);
        }
    }
    return memory;
}

NewArray :: ($T: Type, count: s64, allocator := context.allocator) -> [] T {
    result: [] T;
    if count <= 0  return result;
    result.data = cast(*T) alloc_zeroed(size_of(T) * count, allocator);
    if result.data  result.count = count;
    return result;
}

// ----------------------------------------------------- temporary storage ---

// The allocator behind `temp`. A bump pointer over the block
// `__jai_runtime_init` set aside; when a request does not fit, a page is taken
// from the overflow allocator and chained on (**L§10.4**).
temporary_allocator_proc :: (mode: Allocator_Mode, requested_size: s64, old_size: s64, old_memory: *void, allocator_data: *void) -> *void {
    if #complete mode == {
        case .ALLOCATE;
            return temporary_alloc(requested_size);

        case .RESIZE;
            // The arena does not resize in place; a bigger block is taken and
            // what was there is copied into it.
            memory := temporary_alloc(requested_size);
            if memory && old_memory && old_size > 0 {
                memcpy(memory, old_memory, min(old_size, requested_size));
            }
            return memory;

        // Nothing is given back one piece at a time: the whole arena is reset.
        case .FREE;
            return null;

        case .CAPS;
            caps := Allocator_Caps.HINT_I_AM_A_FAST_BUMP_ALLOCATOR | .HINT_I_AM_PER_FRAME_TEMPORARY_STORAGE;
            return cast(*void) caps;

        case .STARTUP;       #through;
        case .SHUTDOWN;      #through;
        case .THREAD_START;  #through;
        case .THREAD_STOP;   #through;
        case .CREATE_HEAP;   #through;
        case .DESTROY_HEAP;  #through;
        case .IS_THIS_YOURS;
            return null;
    }
}

temp :: Allocator.{temporary_allocator_proc, null};

// Gives the arena back all at once. Anything `temp` handed out before this is
// no longer valid (**L§10.4**).
reset_temporary_storage :: () {
    ts := context.temporary_storage;
    if !ts  return;

    page := ts.overflow_pages;
    while page {
        next := page.next;
        allocator := page.allocator;
        free(page, allocator);
        page = next;
    }
    ts.overflow_pages = null;

    ts.data = ts.original_data;
    ts.size = ts.original_size;
    ts.current_page_bytes_occupied = 0;
    ts.total_bytes_occupied = 0;
}

talloc :: (size: s64) -> *void {
    return alloc(size, temp);
}

#scope_module

TEMPORARY_ALIGNMENT :: 8;

temporary_alloc :: (requested_size: s64) -> *void {
    if requested_size <= 0  return null;
    ts := context.temporary_storage;
    if !ts  return null;

    size := (requested_size + TEMPORARY_ALIGNMENT - 1) & ~(TEMPORARY_ALIGNMENT - 1);
    if ts.current_page_bytes_occupied + size > ts.size {
        if !temporary_add_page(ts, size)  return null;
    }

    memory := ts.data + ts.current_page_bytes_occupied;
    ts.current_page_bytes_occupied += size;
    ts.total_bytes_occupied += size;
    if ts.total_bytes_occupied > ts.high_water_mark  ts.high_water_mark = ts.total_bytes_occupied;
    return memory;
}

TEMPORARY_PAGE_SIZE :: 65536;

temporary_add_page :: (ts: *Temporary_Storage, needed: s64) -> bool {
    allocator := ts.overflow_allocator;
    if !allocator.proc  allocator = Context_Base.default_allocator;

    header := size_of(Temporary_Storage.Overflow_Page);
    size := max(needed + header, TEMPORARY_PAGE_SIZE);
    memory := alloc(size, allocator);
    if !memory  return false;

    page := cast(*Temporary_Storage.Overflow_Page) memory;
    page.next = ts.overflow_pages;
    page.allocator = allocator;
    page.size = size;
    ts.overflow_pages = page;

    ts.data = cast(*u8) memory + header;
    ts.size = size - header;
    ts.current_page_bytes_occupied = 0;
    return true;
}

#scope_export

// ------------------------------------------------------------- utilities ---

min :: (a: $T, b: T) -> T { return ifx a < b then a else b; }
max :: (a: $T, b: T) -> T { return ifx a > b then a else b; }

Swap :: (a: *$T, b: *T) {
    temporary := a.*;
    a.* = b.*;
    b.* = temporary;
}

// --------------------------------------------------------------- logging ---

log :: (format: string, args: ..Any, loc := #caller_location, flags := Log_Flags.NONE, user_flags: u32 = 0, section: *Log_Section = null) {
    message := tprint(format, ..args);
    info: Log_Info;
    info.source_identifier = context.log_source_identifier;
    info.location = loc;
    info.common_flags = flags;
    info.user_flags = user_flags;
    info.section = section;
    if context.logger  context.logger(message, context.logger_data, info);
}

log_error :: (format: string, args: ..Any, loc := #caller_location) {
    log(format, ..args, loc = loc, flags = .ERROR);
}

// ------------------------------------------------------------ assertions ---

// `#expand` so that the location is the caller's and the condition is not
// evaluated when assertions are off (**L§10.5**).
assert :: (condition: bool, format := "", args: ..Any, loc := #caller_location) #expand {
    #if ENABLE_ASSERT {
        if !condition {
            if context.handling_assertion_failure  return;
            context.handling_assertion_failure = true;
            message := ifx format.count then tprint(format, ..args) else "";
            should_break := context.assertion_failed(loc, message);
            context.handling_assertion_failure = false;
            if should_break  Runtime_Support.debug_break();
        }
    }
}

// ------------------------------------------------------------- the process ---

exit :: (status: s32) {
    Linux.exit(status);
}
