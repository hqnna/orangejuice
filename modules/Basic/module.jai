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
#load "Int128.jai";
#load "Apollo_Time.jai";

#module_parameters (ENABLE_ASSERT := true);

#scope_module

Linux :: #import "Linux";

// `timespec`, `tm` and the clock and calendar calls Apollo_Time is built on.
#import "POSIX";

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

#scope_module

// Zeroed storage, which is what a declaration without a value gets
// (**L§4.6**) and what most callers want. The reference has no such name, so
// it stays inside Basic; callers outside use `New` and `NewArray`.
alloc_zeroed :: (size: s64, allocator := context.allocator) -> *void {
    memory := alloc(size, allocator);
    if memory  memset(memory, 0, size);
    return memory;
}

#scope_export

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
New :: ($T: Type, $initialized := true) -> *T {
    memory := alloc(size_of(T));
    #if initialized {
        // Rolled in here rather than deferred to `Initialize`, since `New` is
        // called often enough that the extra polymorph would show.
        ini :: initializer_of(T);
        #if ini  inline ini(memory);
        else     memset(memory, 0, size_of(T));
    }
    return cast(*T) memory;
}

// The count comes first, which is the order the reference reads in:
// `NewArray(n, Node)`.
// `alignment` asks for a base that is a multiple of it. The allocator is not
// asked for one: the extra bytes are taken here and the pointer bumped
// forward, which every allocator can do. The second result is what was really
// allocated, and what has to be freed when an alignment was asked for.
NewArray :: (count: s64, $T: Type, $initialized := true, alignment: s32 = -1) -> (result: [] T, unaligned_base: *void) {
    if count <= 0  return .[], null;

    extra: s32 = 0;
    if alignment > 0  extra = alignment;

    original_memory := alloc(size_of(T) * count + extra);
    if !original_memory  return .[], null;

    memory := original_memory;
    if alignment > 0  memory = cast(*void) align_forward(cast(s64) original_memory, alignment);

    result: [] T = ---;
    result.count = count;
    result.data  = memory;

    #if initialized {
        ini :: initializer_of(T);
        #if ini {
            at := memory;
            for 0..count - 1 {
                inline ini(at);
                at += size_of(T);
            }
        } else {
            memset(memory, 0, count * size_of(T));
        }
    }

    return result, original_memory;
}

// The program's own argument list, as strings rather than the C pointers the
// runtime was handed. `[0]` is how the program was invoked, the way `argv`
// has it.
get_command_line_arguments :: () -> [] string {
    raw := __command_line_arguments;
    result := NewArray(raw.count, string);
    for raw  result[it_index] = to_string(it);
    return result;
}

// A string's bytes go back to the allocator. It is the `data` that was
// allocated, so this is `free` on the pointer with the count dropped.
free :: inline (s: string) {
    free(s.data);
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

// The reference spells the same allocator these two ways as well; all three
// name one thing, so a module written against any of them works.
temporary_allocator :: temp;
__temporary_allocator :: temp;
// The bytes freed storage is stamped with when a program asks for it, so that
// reading through a stale temporary pointer goes wrong loudly.
RESET_STORAGE_STAMP    :: 0xdb;
FREE_PAGE_STAMP        :: 0xdc;
SET_STORAGE_MARK_STAMP :: 0xda;

// Gives the arena back all at once. Anything `temp` handed out before this is
// no longer valid (**L§10.4**).
reset_temporary_storage :: () {
    ts := context.temporary_storage;
    if !ts  return;

    free_pages_down_to(ts, null);

    ts.data = ts.original_data;
    ts.size = ts.original_size;
    ts.current_page_bytes_occupied = 0;
    ts.total_bytes_occupied = 0;
}

// Where temporary storage had got to, so that everything taken after this
// point can be given back without disturbing what was there before.
Temporary_Storage_State :: struct {
    top_overflow_page: *Temporary_Storage.Overflow_Page;
    current_page_bytes_occupied: s64;
    total_bytes_occupied: s64;
}

get_temporary_storage_mark :: inline () -> Temporary_Storage_State {
    empty: Temporary_Storage_State;
    ts := context.temporary_storage;
    if !ts  return empty;

    state: Temporary_Storage_State;
    state.current_page_bytes_occupied = ts.current_page_bytes_occupied;
    state.total_bytes_occupied        = ts.total_bytes_occupied;
    state.top_overflow_page           = ts.overflow_pages;
    return state;
}

// The usual shape is
//
//     mark := get_temporary_storage_mark();
//     defer set_temporary_storage_mark(mark);
//
// or the `auto_release_temp` macro, which is those two lines.
set_temporary_storage_mark :: inline (state: Temporary_Storage_State, loc := #caller_location) {
    ts := context.temporary_storage;
    if !ts  return;

    free_pages_down_to(ts, state.top_overflow_page);
    ts.last_set_mark_location = loc;

    if state.top_overflow_page {
        page := state.top_overflow_page;
        ts.data = cast(*u8) page + size_of(Temporary_Storage.Overflow_Page);
        ts.size = page.size;
    } else {
        ts.data = ts.original_data;
        ts.size = ts.original_size;
    }

    ts.current_page_bytes_occupied = state.current_page_bytes_occupied;
    ts.total_bytes_occupied        = state.total_bytes_occupied;
}

auto_release_temp :: (loc := #caller_location) #expand {
    __mark := get_temporary_storage_mark();
    `defer set_temporary_storage_mark(__mark, loc);
}

talloc :: (size: s64) -> *void {
    return alloc(size, temp);
}

// ------------------------------------------------------------ allocators ---

// A heap of its own from an allocator that can make one. An allocator that
// cannot hands back itself, which is the answer that keeps a caller working.
create_heap :: (a: Allocator) -> Allocator {
    if !a.proc  return a;
    data := a.proc(.CREATE_HEAP, 0, 0, null, a.data);
    if !data  return a;
    result: Allocator;
    result.proc = a.proc;
    result.data = data;
    return result;
}

destroy_heap :: (b: Allocator) {
    if !b.proc  return;
    b.proc(.DESTROY_HEAP, 0, 0, null, b.data);
}

// What an allocator says it can do, and what it calls itself. An allocator
// that does not answer the question is treated as having no special powers.
get_capabilities :: (b: Allocator) -> (caps: Allocator_Caps, name: string) {
    if !b.proc  return 0, "";
    answer := b.proc(.CAPS, 0, 0, null, b.data);
    return cast,no_check(Allocator_Caps) answer, "";
}

// Stores the context's allocator in a data structure that will outlive the
// call that made it, so that it can free what it allocated without the caller
// having to push the same allocator again. `[..]` arrays, Hash_Table and
// Bucket_Array all do this.
remember_allocators :: (data: *$T) {
    if context.allocator.proc  data.allocator = context.allocator;
}

// A program with no memory debugger has nothing to tell it, so these are the
// no-ops that let code annotate itself either way.
Memory_Debugger_Interface :: struct {
    report_allocation:   (memory: *void, size: s64) -> ();
    report_deallocation: (memory: *void) -> ();
}

this_allocation_is_not_a_leak :: (memory: *void) {
}

this_allocation_leaks_todo :: (memory: *void) {
}

this_allocation_does_not_propagate_leak_ownership :: (memory: *void) {
}

memory_visualizer_per_frame_update :: () {
}


// Makes `allocator` the one the rest of this block allocates from, putting
// back whatever was there when the block ends. It is a macro rather than a
// procedure precisely so that the `defer` belongs to the *caller's* block
// (**L§7.5**).
push_allocator :: (allocator: Allocator) #expand {
    // The saved allocator is the macro's own local, not a backticked one, so
    // two pushes in one block do not collide over the name.
    old_allocator := context.allocator;
    context.allocator = allocator;
    `defer context.allocator = old_allocator;
}

// The same, spelled as the two halves of an `Allocator` — which is what a
// caller pushing a pool writes: `push_allocator(pool_allocator_proc, *pool)`.
push_allocator :: (proc: Allocator_Proc, data: *void) #expand {
    old_allocator := context.allocator;
    context.allocator.proc = proc;
    context.allocator.data = data;
    `defer context.allocator = old_allocator;
}

// Bytes straight from the arena, without going through an `Allocator`.
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

#scope_module

TEMPORARY_ALIGNMENT :: 8;

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

// ------------------------------------------------------------- about enums ---
//
// The names and values a `Type_Info_Enum` holds, reached from the type rather
// than from the info — which is what lets a caller write `enum_names(T)` and
// iterate the members of an enum it was handed as a constant.

enum_names :: ($T: Type) -> [] string {
    info := cast(*Type_Info_Enum) type_info(T);
    if info.type != .ENUM  return .[];
    return info.names;
}

enum_values_as_s64 :: ($T: Type) -> [] s64 {
    info := cast(*Type_Info_Enum) type_info(T);
    if info.type != .ENUM  return .[];
    return info.values;
}

min :: (a: $T, b: T) -> T { return ifx a < b then a else b; }
max :: (a: $T, b: T) -> T { return ifx a > b then a else b; }

// The capitalised spellings the reference also uses.
Min :: min;
Max :: max;

// `x` brought inside `a`..`b`.
clamp :: (x: $T, a: T, b: T) -> T {
    if x < a  return a;
    if x > b  return b;
    return x;
}

// Rounds up to the next multiple of `alignment`, which is what placing a value
// after another one in a buffer needs.
align_forward :: (orig_size: s64, alignment: s64) -> s64 {
    if alignment <= 1  return orig_size;
    return (orig_size + alignment - 1) & ~(alignment - 1);
}

// --------------------------------------------------------- about integers ---

// Whether `T` is an integer at all, and if so whether it is signed and how
// wide — which is what a generic routine deciding how to read a value wants.
is_integer_type :: ($T: Type) -> (is_integer: bool, is_signed: bool, bits: u8) {
    info := type_info(T);
    if info.type != .INTEGER  return false, false, 0;
    integer := cast(*Type_Info_Integer) info;
    return true, integer.signed, cast(u8) (integer.runtime_size * 8);
}

// The lowest and highest value `T` holds, in `T` itself.
get_integer_range :: ($T: Type) -> (low: T, high: T) {
    info := cast(*Type_Info_Integer) type_info(T);
    bits := info.runtime_size * 8;
    #if T == u64 {
        return 0, 0xffff_ffff_ffff_ffff;
    } else {
        if info.signed {
            high := (cast(s64) 1 << (bits - 1)) - 1;
            return cast,no_check(T) ~high, cast,no_check(T) high;
        }
        return 0, cast,no_check(T) ((cast(u64) 1 << bits) - 1);
    }
}

// `value` brought inside whatever `TargetType` can hold, rather than wrapped.
clamp_to_another_integer_type :: (value: $SourceType, $TargetType: Type) -> TargetType {
    low, high := get_integer_range(TargetType);
    wide := cast,no_check(s64) value;
    if wide < cast,no_check(s64) low   return low;
    if wide > cast,no_check(s64) high  return high;
    return cast,no_check(TargetType) value;
}

// Reads an integer of `size_in_bytes` through a pointer, widened to 64 bits.
// The signed form sign-extends; the unsigned one does not.
get_s64_from_void_pointer :: (pointer: *void, size_in_bytes: int) -> s64 {
    if size_in_bytes == {
        case 1; return (cast(*s8)  pointer).*;
        case 2; return (cast(*s16) pointer).*;
        case 4; return (cast(*s32) pointer).*;
        case;   return (cast(*s64) pointer).*;
    }
}

get_u64_from_void_pointer :: (pointer: *void, size_in_bytes: int) -> u64 {
    if size_in_bytes == {
        case 1; return (cast(*u8)  pointer).*;
        case 2; return (cast(*u16) pointer).*;
        case 4; return (cast(*u32) pointer).*;
        case;   return (cast(*u64) pointer).*;
    }
}

// ------------------------------------------------------------ about enums ---

enum_range_given_info :: (info: *Type_Info_Enum) -> (low: s64, high: s64) {
    if !info.values.count  return 0, 0;
    low := info.values[0];
    high := info.values[0];
    for info.values {
        if it < low   low = it;
        if it > high  high = it;
    }
    return low, high;
}

enum_range :: ($t: Type) -> (low: s64, high: s64) {
    return enum_range_given_info(cast(*Type_Info_Enum) type_info(t));
}

enum_highest_value :: ($t: Type) -> s64 {
    low, high := enum_range(t);
    return high;
}

// The members as values of the enum itself rather than as numbers, which is
// what a `for` over an enum's members wants.
enum_values_as_enum :: ($T: Type) -> [] T {
    info := cast(*Type_Info_Enum) type_info(T);
    result := NewArray(info.values.count, T,, temp);
    for info.values  result[it_index] = cast,no_check(T) it;
    return result;
}

// ---------------------------------------------------------- about structs ---

// The member with that name and where it sits. Only the struct's own members
// are searched, so a name brought in by `using` is found through the member it
// came through.
get_field :: (info: *Type_Info_Struct, name: string) -> (member: *Type_Info_Struct_Member, offset_in_bytes: s64) {
    if !info || info.type != .STRUCT  return null, 0;
    for * member: info.members {
        if member.name == name  return member, member.offset_in_bytes;
    }
    return null, 0;
}

// A type's name the way `print("%", T)` writes it, in temporary storage.
type_to_string :: (info: *Type_Info) -> string {
    builder: String_Builder;
    builder.allocator = temp;
    append_type(*builder, info);
    return builder_to_string(*builder,, allocator = temp);
}

// ----------------------------------------------------------- about arrays ---

// The last element, without removing it. The caller is responsible for there
// being one.
peek :: inline (array: [] $T) -> T {
    return array[array.count - 1];
}

peek_pointer :: inline (array: [] $T) -> *T {
    return *array[array.count - 1];
}

// A view turned into a resizable array over the same elements. Nothing is
// copied, so the result must not outlive what it was made from, and it must
// not be grown unless `set_allocators` gave it one to grow with.
resizable :: (a: [] $T, set_allocators := true) -> [..] T {
    result: [..] T;
    result.data = a.data;
    result.count = a.count;
    result.allocated = a.count;
    if set_allocators  result.allocator = context.allocator;
    return result;
}


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

// Frees every overflow page above `page`, leaving `page` itself in place.
// Passing null frees them all.
free_pages_down_to :: (ts: *Temporary_Storage, page: *Temporary_Storage.Overflow_Page) {
    current := ts.overflow_pages;
    while current != page {
        if !current  break;
        next := current.next;
        allocator := current.allocator;
        free(current, allocator);
        current = next;
    }
    ts.overflow_pages = page;
}

#scope_export

// -------------------------------------------------------- stack traces ---
//
// Each of these walks the chain `context.stack_trace` heads, innermost first.
// A node whose `info` is null belongs to a procedure the build did not keep
// information for, and is skipped rather than printed as a blank.

print_stack_trace :: (node: *Stack_Trace_Node, prefix: string = "", to_standard_error := true) {
    walk := node;
    while walk {
        if walk.info {
            if prefix.count  write_string(prefix, to_standard_error);
            write_string("'", to_standard_error);
            if walk.info.name  write_string(walk.info.name, to_standard_error);
            write_string("' at ", to_standard_error);
            if walk.info.location.fully_pathed_filename  write_string(walk.info.location.fully_pathed_filename, to_standard_error);
            write_string(":", to_standard_error);
            write_string(tprint("%", walk.line_number), to_standard_error);
            write_string("\n", to_standard_error);
        }
        walk = walk.next;
    }
}

log_stack_trace :: (node: *Stack_Trace_Node, log_flags := Log_Flags.ERROR, prefix := "") {
    walk := node;
    while walk {
        if walk.info {
            log("%'%' at %:%\n", prefix, walk.info.name, walk.info.location.fully_pathed_filename, walk.line_number, flags = log_flags);
        }
        walk = walk.next;
    }
}

get_stack_trace_string :: (node: *Stack_Trace_Node) -> string {
    if !node  return "";

    builder: String_Builder;
    walk := node;
    while walk {
        if walk.info {
            print_to_builder(*builder, "'%' at %:%\n", walk.info.name, walk.info.location.fully_pathed_filename, walk.line_number);
        }
        walk = walk.next;
    }
    return builder_to_string(*builder);
}

// The chain flattened into an array, with the `next` pointers rewired to point
// along it — so the copy outlives the frames the original nodes lived in.
pack_stack_trace :: () -> [] Stack_Trace_Node {
    return pack_stack_trace(context.stack_trace);
}

pack_stack_trace :: (node: *Stack_Trace_Node) -> [] Stack_Trace_Node {
    result: [..] Stack_Trace_Node;
    walk := node;
    while walk {
        array_add(*result, walk.*);
        walk = walk.next;
    }

    for * result {
        // The last one was already null and stays null.
        if it.next  it.next = it + 1;
    }
    return result;
}

// --------------------------------------------------------- the odds and ends ---

// `T`'s own initializer where it has one, and zeroes otherwise — which is what
// a declaration without a value does (**L§4.6**).
Initialize :: (memory: *$T) {
    ini :: initializer_of(T);
    #if ini  inline ini(memory);
    else     memset(memory, 0, size_of(T));
}

// Whether a type is one `min`, `max` and `clamp` can compare, which is what
// their `#modify` blocks ask.
IsScalar :: (T: Type) -> bool {
    info := cast(*Type_Info) T;
    return info.type == .INTEGER || info.type == .FLOAT;
}

swap :: inline (a: $T, b: T) -> (T #must, T #must) #deprecated "Just use a1, b1 := b0, a0;" {
    return b, a;
}

array_swap :: (array_a: *[..] $T, array_b: *[..] T) #deprecated "Use Swap() instead." {
    Swap(array_a, array_b);
}

// The assertion machinery `assert` expands into, which a program can call to
// report a failure the same way without writing the condition out.
assert_helper :: (message := "", args: .. Any, loc := #caller_location) -> bool {
    if context.handling_assertion_failure  return false;
    context.handling_assertion_failure = true;

    text := ifx args.count then tprint(message, ..args) else message;
    Runtime_Support.assertion_failed(loc, text);

    context.handling_assertion_failure = false;
    return true;
}

sleep_milliseconds :: (milliseconds: s32) {
    if milliseconds <= 0  return;
    request: timespec;
    request.tv_sec = milliseconds / 1000;
    request.tv_nsec = (cast(s64) milliseconds % 1000) * 1_000_000;
    remaining: timespec;
    // A signal can cut the sleep short, and then what is left is slept again.
    while nanosleep(*request, *remaining) != 0 {
        if errno() != EINTR  break;
        request = remaining;
    }
}

// Where the process is running, or "" when it could not be found out.
get_working_directory :: () -> (result: string) {
    buffer: [4096] u8;
    if !getcwd(buffer.data, cast(u64) buffer.count)  return "";
    return copy_string(to_string(buffer.data));
}

set_working_directory :: (s: string) -> bool {
    return chdir(to_c_string(s, temp)) == 0;
}

// Leading spaces off the front, for a caller walking a line by hand.
eat_spaces :: (_s: string) -> string {
    result := _s;
    while result.count && (result[0] == #char " " || result[0] == #char "\t") {
        result.data += 1;
        result.count -= 1;
    }
    return result;
}

my_is_digit :: (c: u8) -> bool {
    return c >= #char "0" && c <= #char "9";
}

// The digits starting at `cursor`, and where they stopped.
parse_number :: (s: string, cursor: s64) -> (end_cursor: s64, sum: s64) {
    at := cursor;
    sum := 0;
    while at < s.count && my_is_digit(s[at]) {
        sum = sum * 10 + cast(s64) (s[at] - #char "0");
        at += 1;
    }
    return at, sum;
}

// Takes `length` bytes off the front of `input` into `s`, when there are that
// many. The mirror of appending a length-prefixed run.
consume_u8_and_length :: (input: *string, s: *u8, length: s64) -> bool {
    if input.count < length  return false;
    if length  memcpy(s, input.data, length);
    input.data += length;
    input.count -= length;
    return true;
}
