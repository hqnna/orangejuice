//
// A dense array of bits.
//
// Sixty-four of them to a slot, so a set of a few thousand flags costs a few
// hundred bytes and a whole-array operation runs a word at a time rather than
// a bit at a time.
//
// `for` over one visits every bit as a `bool`, with `it_index` its position.
// `only_set` and `only_unset` visit just the bits of one kind, skipping whole
// empty slots as they go — which is what makes walking a sparse array cheap.
//

#import "Basic";
#import "Bit_Operations";

Bit_Array :: struct {
    slots: [] s64;
    // How many bits the caller asked for, which is not the same as how many
    // the slots hold.
    count: int;

    allocator: Allocator;
}

// Sets the array up with room for `count` bits, all clear. Not to be called
// twice on the same array.
init_bit_array :: (array: *Bit_Array, count: int) {
    if !array.allocator.proc  array.allocator = context.allocator;

    slots := (count + 63) / 64;
    array.slots = NewArray(slots, s64,, array.allocator);
    array.count = count;
}

make_bit_array :: (count: int) -> Bit_Array #must {
    result: Bit_Array;
    init_bit_array(*result, count);
    return result;
}

deinit :: (using array: *Bit_Array) {
    if slots.data  free(slots.data,, allocator = allocator);
    slots.data = null;
    slots.count = 0;
    count = 0;
}

// ------------------------------------------------------------- one bit ---

set_bit :: (using array: *Bit_Array, i: int) {
    assert(i >= 0 && i < count);
    slots[i >> 6] |= (cast(s64) 1 << (i & 63));
}

clear_bit :: (using array: *Bit_Array, i: int) {
    assert(i >= 0 && i < count);
    slots[i >> 6] &= ~(cast(s64) 1 << (i & 63));
}

toggle_bit :: (using array: *Bit_Array, i: int) {
    assert(i >= 0 && i < count);
    slots[i >> 6] ^= (cast(s64) 1 << (i & 63));
}

set_bit_to :: (using array: *Bit_Array, i: int, value: bool) {
    if value  set_bit(array, i);
    else      clear_bit(array, i);
}

operator [] :: (a: Bit_Array, i: int) -> bool {
    assert(i >= 0 && i < a.count);
    return (a.slots[i >> 6] & (cast(s64) 1 << (i & 63))) != 0;
}

operator []= :: inline (a: *Bit_Array, i: int, value: bool) {
    set_bit_to(a, i, value);
}

// ------------------------------------------------------------ all of them ---
//
// A slot at a time. The bits past `count` in the last slot are set along with
// the rest; nothing reads them, and masking them would cost a branch on every
// whole-array operation.

clear_all_bits :: (using array: *Bit_Array) {
    for * slots  it.* = 0;
}

set_all_bits :: (using array: *Bit_Array) {
    for * slots  it.* = -1;
}

toggle_all_bits :: (using array: *Bit_Array) {
    for * slots  it.* = ~it.*;
}

// -------------------------------------------------------------- iterating ---

for_expansion :: (array: *Bit_Array, body: Code, flags: For_Flags) #expand {
    for index: 0..array.count - 1 {
        `it_index := index;
        `it := (array.slots[index >> 6] & (cast(s64) 1 << (index & 63))) != 0;
        #insert (break = break index, continue = continue index) body;
    }
}

for_expansion :: (array: Bit_Array, body: Code, flags: For_Flags) #expand {
    copy := array;
    for :for_expansion *copy {
        `it_index := it_index;
        `it := it;
        #insert body;
    }
}

// `for only_set(*array)` visits the positions of the set bits and nothing
// else: a slot with none of them is skipped whole, so a sparse array costs
// about what its contents cost rather than what its length does.
only_set   :: #bake_arguments only_set_or_unset(target_value = true);
only_unset :: #bake_arguments only_set_or_unset(target_value = false);

#scope_module

only_set_or_unset :: (array: *Bit_Array, body: Code, flags: For_Flags, target_value: bool) #expand {
    #assert !flags;

    // `it` is the kind of bit being visited, so it never varies; `it_index` is
    // what the caller is actually after.
    `it := target_value;

    for slot, slot_index: array.slots {
        // Looking for clear bits is the same walk over the complement.
        #if !target_value  slot = ~slot;
        if !slot  continue;

        remaining := slot;
        while remaining {
            // The lowest set bit, and then clear it: `x & (x - 1)` drops it
            // without a shift.
            bit := bit_scan_forward(cast,no_check(u64) remaining) - 1;
            remaining &= remaining - 1;

            index := slot_index * 64 + bit;
            if index >= array.count  break;

            `it_index := index;
            #insert body;
        }
    }
}
