//
// Atomics: operations no other thread can see half-done.
//
// Every one of these works on a *scalar* — an integer, an enum or a pointer —
// which is what `IsAtomicScalar` decides and what each `#modify` below asks
// it, so a call on anything else is not an overload that exists rather than a
// program that compiles and races.
//
// On x86-64 each is the `lock`-prefixed instruction for the job. Elsewhere it
// is a compare-and-swap loop around the same arithmetic, which is correct
// anywhere and slower where the machine had an instruction for it.
//

#import "Basic";

// Writes `new_value` and hands back what was there. `xchg` is atomic on x86
// whether or not it is asked to be, but the mnemonic carries the prefix.
atomic_swap :: (dest: *$T, new_value: T) -> (old_value: T) {
    #if CPU == .X64 {
        // Copied into a local first: a small struct passed by reference is not
        // ours to write through.
        v := new_value;
        #asm { lock_xchg?T v, [dest]; }
        return v;
    } else {
        old_value := dest.*;
        while true {
            success := compare_and_swap(dest, old_value, new_value);
            if success  break;
            old_value = dest.*;
        }
        return old_value;
    }
}

// Two adjacent 64-bit words swapped together, which `cmpxchg16b` can do and
// nothing else can.
compare_and_swap2 :: (dest: *void, old_value_1: $T1_64, old_value_2: $T2_64, new_value_1: T1_64, new_value_2: T2_64) -> bool #deprecated "Nobody is using this and it is currently x64-only so... yeah." {
    #assert size_of(T1_64) == 8 && size_of(T2_64) == 8  "Invalid size passed to compare_and_swap2; both value types must be 64-bit.";

    // `cmpxchg16b` wants the expected pair in RDX:RAX and the new pair in
    // RCX:RBX, and it writes what it found back over the expected pair.
    expected_low  := old_value_1;
    expected_high := old_value_2;

    success: bool = ---;
    #asm {
        expected_low  === a;
        expected_high === d;
        new_low: gpr === b;
        new_high: gpr === c;

        mov new_low, new_value_1;
        mov new_high, new_value_2;

        lock_cmpxchg16b [dest], new_high, new_low, expected_high, expected_low;
        setz success;
    }
    return success;
}

// Sets bit `i` and says whether it was already set. `$$i` so that a constant
// index becomes an immediate.
atomic_bit_test_and_set :: (dest: *$T, $$i: s64) -> bool {
    #if CPU == .X64 {
        was_set: bool = ---;
        index := i;
        #asm {
            lock_bts?T [dest], index;
            setc was_set;
        }
        return was_set;
    } else {
        mask := cast(T) 1 << i;
        while true {
            old := dest.*;
            if old & mask  return true;
            success := compare_and_swap(dest, old, old | mask);
            if success  return false;
        }
    }
}

// A scalar load and store are already atomic on every machine this runs on, so
// these exist to say in the source that the access is deliberate.
atomic_read :: (pointer: *$T) -> T #expand {
    return pointer.*;
}

atomic_write :: (pointer: *$T, new_value: T) #expand {
    pointer.* = new_value;
}

// What these operations accept: something the machine can write in one go. A
// variant is whatever it is a variant of.
IsAtomicScalar :: (T: Type) -> bool #compile_time {
    info := cast(*Type_Info) T;
    while info.type == .VARIANT {
        info = (cast(*Type_Info_Variant) info).variant_of;
    }

    if info.type == .INTEGER  return true;
    if info.type == .ENUM     return true;
    if info.type == .POINTER  return true;
    return false;
}

// Adds and hands back what was there before. `T` and `I` are separate so that
// a pointer may be advanced by an integer; when they differ, `I` becomes the
// integer of `T`'s own width.
atomic_add :: (dest: *$T, to_add: $I) -> T
#modify {
    if !IsAtomicScalar(T)  return false;
    if T == I  return true;

    info := cast(*Type_Info) T;
    integer, exists := integer_for_size(info.runtime_size, IsSigned(I));
    if !exists  return false;
    I = integer;
    return true;
}
{
    #if CPU == .X64 {
        // `xadd` writes the sum and leaves the old value in the source
        // operand, which is exactly what is wanted back.
        old := cast,no_check(T) to_add;
        #asm { lock_xadd?T [dest], old; }
        return old;
    } else {
        old := dest.*;
        while true {
            success := compare_and_swap(dest, old, old + cast,no_check(T) to_add);
            if success  break;
            old = dest.*;
        }
        return old;
    }
} @NoProfile

atomic_increment :: (dest: *$T)
#modify { return IsAtomicScalar(T); }
{
    #if CPU == .X64 {
        #asm { lock_inc?T [dest]; }
    } else {
        old := dest.*;
        while true {
            success := compare_and_swap(dest, old, old + 1);
            if success  break;
            old = dest.*;
        }
    }
} @NoProfile

// True when what is left is zero, which is what a reference count wants to
// know and what no separate read could tell it.
atomic_decrement :: (dest: *$T) -> bool
#modify { return IsAtomicScalar(T); }
{
    #if CPU == .X64 {
        zero: bool = ---;
        #asm {
            lock_dec?T [dest];
            setz zero;
        }
        return zero;
    } else {
        old := dest.*;
        while true {
            success := compare_and_swap(dest, old, old - 1);
            if success  break;
            old = dest.*;
        }
        return old == 1;
    }
} @NoProfile

atomic_and :: (dest: *$T, value: T)
#modify { return IsAtomicScalar(T); }
{
    #if CPU == .X64 {
        #asm { lock_and?T [dest], value; }
    } else {
        old := dest.*;
        while true {
            success := compare_and_swap(dest, old, old & value);
            if success  break;
            old = dest.*;
        }
    }
}

atomic_or :: (dest: *$T, value: T)
#modify { return IsAtomicScalar(T); }
{
    #if CPU == .X64 {
        #asm { lock_or?T [dest], value; }
    } else {
        old := dest.*;
        while true {
            success := compare_and_swap(dest, old, old | value);
            if success  break;
            old = dest.*;
        }
    }
}

atomic_xor :: (dest: *$T, value: $I)
#modify { return IsAtomicScalar(T); }
{
    #if CPU == .X64 {
        #asm { lock_xor?T [dest], value; }
    } else {
        old := dest.*;
        while true {
            success := compare_and_swap(dest, old, old ^ value);
            if success  break;
            old = dest.*;
        }
    }
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
        if !atomic_swap(*spin.held, 1)  return;
        // Tell the processor this is a spin loop: it keeps the pipeline from
        // filling with speculative loads that will have to be thrown away.
        #asm { pause; }
    }
}

try_lock :: (spin: *Spin_Lock) -> bool {
    return atomic_swap(*spin.held, 1) == 0;
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

#scope_file

// The integer type of a given width, for the `atomic_add` that advances a
// pointer by a count.
integer_for_size :: (size: s64, signed := false) -> (type: Type, exists := true) #compile_time {
    if signed {
        if size == 1  return s8;
        if size == 2  return s16;
        if size == 4  return s32;
        if size == 8  return s64;
    } else {
        if size == 1  return u8;
        if size == 2  return u16;
        if size == 4  return u32;
        if size == 8  return u64;
    }
    return void, false;
}

IsSigned :: (type: Type) -> bool #compile_time {
    info := cast(*Type_Info) type;
    if info.type != .INTEGER  return false;
    return (cast(*Type_Info_Integer) info).signed;
}
