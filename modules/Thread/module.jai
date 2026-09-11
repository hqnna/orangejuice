//
// Threads and the things that coordinate them.
//
// `primitives.jai` has the raw ones — `Thread`, `Mutex`, `Condition_Variable`,
// `Semaphore`. `thread_group.jai` has `Thread_Group`, a pool of workers with a
// queue each, which is what most programs actually want and which is loaded
// unless a caller asks otherwise.
//
// `LOAD_THREAD_GROUP` is off for a program importing this after the context is
// finalized, since the group adds nothing to the context but its `Thread`
// carries a pointer into it.
//

#module_parameters (LOAD_THREAD_GROUP := true)(CACHE_LINE_SIZE: s32 = 64);

#scope_module

#load "primitives.jai";

#if LOAD_THREAD_GROUP {
    #load "thread_group.jai";
}

Basic :: #import "Basic";

operator- :: Basic.operator-;

assert    :: Basic.assert;
array_add :: Basic.array_add;

// Thread zero is the one the program started on, so the first one made is one.
next_thread_index: u32 = 1;

#import "Atomics";
#import "POSIX";

#scope_export
