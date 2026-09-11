//
// The allocator a program gets unless it says otherwise.
//
// The reference ships a stripped rpmalloc here. This is the C heap with
// optional accounting on top: `malloc` is a good general-purpose allocator and
// reimplementing one badly is worse than using it.
//
// A program that wants something else replaces this module —
// `remap_import(w, "*", "Default_Allocator", "My_Allocator")` — or just sets
// `context.allocator` and never comes here at all.
//

#import "Basic";
#import "POSIX";

// Counting costs an atomic add per allocation, so it is off unless asked for.
#module_parameters (TRACK_ALLOCATIONS := false);

Allocation_Stats :: struct {
    live_bytes:       s64;
    live_count:       s64;
    total_bytes:      s64;
    total_count:      s64;
    high_water_mark:  s64;
}

// @Volatile: `Runtime_Support` names this as the context's starting allocator,
// so the signature is the one **L§10.4** fixes.
allocator_proc :: (mode: Allocator_Mode, requested_size: s64, old_size: s64, old_memory: *void, allocator_data: *void) -> *void {
    if #complete mode == {
        case .ALLOCATE;
            if requested_size <= 0  return null;
            memory := malloc(cast(u64) requested_size);
            #if TRACK_ALLOCATIONS  if memory  note_allocation(requested_size);
            return memory;

        case .RESIZE;
            if requested_size <= 0 {
                free(old_memory);
                #if TRACK_ALLOCATIONS  if old_memory  note_free(old_size);
                return null;
            }
            memory := realloc(old_memory, cast(u64) requested_size);
            #if TRACK_ALLOCATIONS {
                if memory {
                    if old_memory  note_free(old_size);
                    note_allocation(requested_size);
                }
            }
            return memory;

        case .FREE;
            if old_memory {
                free(old_memory);
                #if TRACK_ALLOCATIONS  note_free(old_size);
            }
            return null;

        case .CAPS;
            caps := Allocator_Caps.FREE | .ACTUALLY_RESIZE | .MULTIPLE_THREADS | .HINT_I_AM_A_GENERAL_HEAP_ALLOCATOR;
            return cast(*void) caps;

        case .STARTUP;      #through;
        case .SHUTDOWN;     #through;
        case .THREAD_START; #through;
        case .THREAD_STOP;
            return null;

        // Sub-heaps are not something the C heap offers, and claiming a
        // pointer is not something it can be asked.
        case .CREATE_HEAP;   #through;
        case .DESTROY_HEAP;  #through;
        case .IS_THIS_YOURS;
            return null;
    }
}

// An `Allocator` value for handing to anything that takes one.
allocator :: Allocator.{allocator_proc, null};

// The older spelling.
default_allocator :: allocator;

// The heap a call with no `allocator_data` uses. The C heap has one and it is
// not ours to name, so this stays null and every call goes to it.
allocator_default_heap: *Heap;

// Nothing to set up: the C heap initializes itself on first use. These exist
// because a program written against the reference calls them.
initialize_if_needed :: inline () -> (did_init: bool) {
    return false;
}

is_initialized :: inline () -> bool {
    return true;
}

finalize :: () {
}

unsafe_reset :: inline () {
}

// A heap the C allocator gave out cannot be freed wholesale, and it cannot be
// asked whether a pointer is its own.
heap_free_all :: inline (heap: *Heap) {
}

is_this_yours :: (heap: *Heap, memory: *void) -> bool {
    return false;
}

// How many bytes the allocation really has, which is at least what was asked
// for and often more.
usable_size :: inline (pointer: *void) -> s64 {
    if !pointer  return 0;
    return cast(s64) malloc_usable_size(pointer);
}

// The reference's own statistics are a dummy in the build that ships; ours are
// what `TRACK_ALLOCATIONS` counted, in the shape a caller expects.
Global_Statistics :: struct {
    mapped:          s64;
    mapped_peak:     s64;
    cached:          s64;
    huge_alloc:      s64;
    huge_alloc_peak: s64;
    mapped_total:    s64;
    unmapped_total:  s64;
}

Heap_Statistics :: struct {
    sizecache: s64;
    spancache: s64;
}

global_statistics :: () -> Global_Statistics {
    result: Global_Statistics;
    result.mapped       = stats.live_bytes;
    result.mapped_peak  = stats.high_water_mark;
    result.mapped_total = stats.total_bytes;
    return result;
}

heap_statistics :: (heap: *Heap) -> Heap_Statistics {
    return .{};
}

// What has been allocated through here, when tracking is on. All zero when it
// is not, rather than wrong.
get_stats :: () -> Allocation_Stats {
    return stats;
}

reset_stats :: () {
    stats = .{};
}

#scope_module

// A heap of the C allocator's own, which it never hands out: every call goes
// to the one heap there is. It is a type only so that the procedures a program
// written against the reference calls can take one.
Heap :: struct {}

malloc_usable_size :: (pointer: *void) -> u64 #foreign libc_default_allocator;

libc_default_allocator :: #system_library "c";

stats: Allocation_Stats;


note_allocation :: (size: s64) {
    stats.live_bytes += size;
    stats.live_count += 1;
    stats.total_bytes += size;
    stats.total_count += 1;
    if stats.live_bytes > stats.high_water_mark  stats.high_water_mark = stats.live_bytes;
}

note_free :: (size: s64) {
    stats.live_bytes -= size;
    stats.live_count -= 1;
}
