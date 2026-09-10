//
// Atomics: operations no other thread can see half-done.
//
// `compare_and_swap` is the intrinsic Preload declares (**L§17**); the rest is
// built out of it. A compare-and-swap loop is slower than a dedicated
// instruction where the machine has one, but it is correct on every machine
// and it is what the primitives here need.
//

#import "Basic";

// Adds and hands back what was there before, the way `fetch_add` does
// everywhere else.
atomic_add :: (pointer: *$T, amount: T) -> T {
    while true {
        old := pointer.*;
        success, _ := compare_and_swap(pointer, old, old + amount);
        if success  return old;
    }
    empty: T;
    return empty;
}

atomic_subtract :: (pointer: *$T, amount: T) -> T {
    return atomic_add(pointer, -amount);
}

atomic_increment :: (pointer: *$T) -> T {
    return atomic_add(pointer, cast(T) 1);
}

atomic_decrement :: (pointer: *$T) -> T {
    return atomic_add(pointer, cast(T) -1);
}

// Writes and hands back what was there before.
atomic_exchange :: (pointer: *$T, value: T) -> T {
    while true {
        old := pointer.*;
        success, _ := compare_and_swap(pointer, old, value);
        if success  return old;
    }
    empty: T;
    return empty;
}

atomic_or :: (pointer: *$T, bits: T) -> T {
    while true {
        old := pointer.*;
        success, _ := compare_and_swap(pointer, old, old | bits);
        if success  return old;
    }
    empty: T;
    return empty;
}

atomic_and :: (pointer: *$T, bits: T) -> T {
    while true {
        old := pointer.*;
        success, _ := compare_and_swap(pointer, old, old & bits);
        if success  return old;
    }
    empty: T;
    return empty;
}

// Writes `value` only if `expected` is what is there. `ok` says whether it
// was; `previous` is what was found either way, which is what a caller retries
// from.
atomic_compare_exchange :: (pointer: *$T, expected: T, value: T) -> (ok: bool, previous: T) {
    success, previous := compare_and_swap(pointer, expected, value);
    return success, previous;
}

// ------------------------------------------------------------------ locks ---

// A spin lock. Right where the critical section is a few instructions and
// contention is rare; wrong for anything that waits on IO, since a thread
// spinning here is a core doing nothing.
Spin_Lock :: struct {
    // 0 is free, 1 is held.
    held: s32;
}

lock :: (spin: *Spin_Lock) {
    while true {
        success, _ := compare_and_swap(*spin.held, 0, 1);
        if success  return;
        // Tell the processor this is a spin loop: it keeps the pipeline from
        // filling with speculative loads that will have to be thrown away.
        #asm { pause; }
    }
}

try_lock :: (spin: *Spin_Lock) -> bool {
    success, _ := compare_and_swap(*spin.held, 0, 1);
    return success;
}

unlock :: (spin: *Spin_Lock) {
    // A release: everything written inside the critical section has to be
    // visible before the lock reads as free.
    memory_barrier();
    spin.held = 0;
}

// Nothing the compiler or the processor does may move a memory access across
// this. `mfence` is the whole-pipeline barrier on x86-64.
memory_barrier :: () #no_context {
    #asm { mfence; }
}
