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
default_allocator :: Allocator.{allocator_proc, null};

// What has been allocated through here, when tracking is on. All zero when it
// is not, rather than wrong.
get_stats :: () -> Allocation_Stats {
    return stats;
}

reset_stats :: () {
    stats = .{};
}

#scope_module

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
