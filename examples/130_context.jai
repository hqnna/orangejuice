//
// The implicit `context`: what every Jai procedure is handed without being
// asked (**L§10**).
//

#import "Basic";
Default_Allocator :: #import "Default_Allocator";

// A module adds its own member, and it is laid out after everything already
// there.
#add_context depth: int;

nested :: () {
    for 1..context.depth  print("  ");
    print("at depth %\n", context.depth);
    if context.depth < 3 {
        // `push_context` installs a changed context for a block and puts the
        // old one back after it.
        c := context;
        c.depth += 1;
        push_context c {
            nested();
        }
    }
}

// A `#no_context` procedure is handed nothing, which is what a callback a C
// library invokes has to be.
no_context_add :: (a: int, b: int) -> int #no_context {
    return a + b;
}

counting_allocator :: (mode: Allocator_Mode, size: s64, old_size: s64, old: *void, data: *void) -> *void {
    counter := cast(*int) data;
    if mode == .ALLOCATE || mode == .RESIZE  counter.* += 1;
    return Default_Allocator.allocator_proc(mode, size, old_size, old, null);
}

main :: () {
    nested();
    print("%\n", no_context_add(2, 3));

    // The allocator is a context member, so replacing it changes where every
    // library that allocates gets its memory — without a single one of them
    // being told.
    allocations := 0;
    {
        c := context;
        c.allocator = .{counting_allocator, *allocations};
        push_context c {
            array: [..] int;
            for 1..100  array_add(*array, it);
            array_free(array);
        }
    }
    print("allocations through ours: %\n", allocations > 0);
    print("and the context is back: %\n", context.allocator.proc != counting_allocator);
}
