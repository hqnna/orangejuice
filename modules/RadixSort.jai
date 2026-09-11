//
// A radix sort that produces an *order* rather than a sorted array.
//
// `sort` fills the `ranks` array with the indices of the input in ascending
// order and leaves the input where it was, which is what a caller with several
// parallel arrays wants: sort one, then apply the same order to the rest.
// `reorder` does that application for an array of any type.
//
// Eight bits at a time, one pass per byte of the key, counting first and then
// placing — so the work is linear in the input rather than n log n, at the
// cost of a pass over it per byte. Under 32 elements an insertion sort wins
// and is what runs.
//
// Sorting the same array again is cheap: the ranks from last time are kept and
// used as the starting order, which is nearly sorted if the data barely moved.
//
// Floats are sorted by flipping their bits into an order that compares the
// same way unsigned — the sign bit inverted for a positive number and every
// bit for a negative one — and flipping them back afterwards.
//

#import "Basic";

RadixSort :: struct {
    // The order, and the buffer the next pass builds into. They swap at the
    // end of every pass.
    ranks:  [] u32;
    ranks2: [] u32;

    // Whether `ranks` holds an order worth starting from.
    valid_ranks: bool;

    allocator: Allocator;
}

// Forgets the order without giving the storage back, so the next sort starts
// from scratch.
reset :: (using radix: *RadixSort) {
    valid_ranks = false;
}

resize :: (using radix: *RadixSort, new_size: u32) {
    size := ranks.count;
    if new_size == size  return;

    if new_size > size {
        ranks.data  = realloc(ranks.data,  new_size * size_of(u32), size * size_of(u32),, allocator);
        ranks2.data = realloc(ranks2.data, new_size * size_of(u32), size * size_of(u32),, allocator);
    }

    ranks.count  = new_size;
    ranks2.count = new_size;

    valid_ranks = false;
}

free_resources :: (using radix: *RadixSort) {
    free(ranks.data,,  allocator);
    free(ranks2.data,, allocator);
}

// The index of the `i`th smallest element of the last input sorted.
operator[] :: (using radix: RadixSort, i: int) -> u32 {
    return ranks[i];
}

rank :: (using radix: *RadixSort, i: int) -> u32 {
    return ranks[i];
}

sort :: (using radix: *RadixSort, input: *u32, count: u32) {
    if input == null || count == 0  return;
    resize(radix, count);

    if count < INSERTION_SORT_LIMIT  insertion_sort(radix, input, count);
    else                             radix_sort(radix, input, count);
}

sort :: (using radix: *RadixSort, input: *u64, count: u32) {
    if input == null || count == 0  return;
    resize(radix, count);

    if count < INSERTION_SORT_LIMIT  insertion_sort(radix, input, count);
    else                             radix_sort(radix, input, count);
}

sort :: (using radix: *RadixSort, input: *float, count: u32) {
    if input == null || count == 0  return;
    resize(radix, count);

    if count < INSERTION_SORT_LIMIT {
        // An insertion sort compares the floats themselves, so nothing has to
        // be flipped.
        insertion_sort(radix, input, count);
        return;
    }

    for 0..count - 1  float_flip(cast(*u32) *input[it]);
    radix_sort(radix, input, count);
    for 0..count - 1  inverse_float_flip(cast(*u32) *input[it]);
}

sort :: (using radix: *RadixSort, input: [] u32)   { sort(radix, input.data, xx input.count); }
sort :: (using radix: *RadixSort, input: [] u64)   { sort(radix, input.data, xx input.count); }
sort :: (using radix: *RadixSort, input: [] float) { sort(radix, input.data, xx input.count); }

// Applies the order to an array of anything, in place.
reorder :: (input: *[..] $T, radix: RadixSort) {
    output: [..] T;
    array_resize(*output, input.count);

    for * output  it.* = input.*[radix.ranks[it_index]];

    array_free(input.*);
    input.* = output;
}

#scope_file

INSERTION_SORT_LIMIT :: 32;

// Turns a float's bits into an unsigned number that orders the same way: a
// positive float only needs its sign bit set, a negative one needs every bit
// inverted, and the arithmetic shift of the sign bit produces exactly that
// mask either way.
float_flip :: (f: *u32) {
    mask: u32 = cast,no_check(u32) (cast,no_check(s32) f[0] >> 31) | 0x8000_0000;
    f[0] ^= mask;
}

inverse_float_flip :: (f: *u32) {
    mask: u32 = ((f[0] >> 31) - 1) | 0x8000_0000;
    f[0] ^= mask;
}

// Under the limit, and for a nearly-sorted second run, this beats the passes.
insertion_sort :: (using radix: *RadixSort, input: *$T, count: u32) {
    if !valid_ranks {
        ranks[0] = 0;
        for i: 1..count - 1  ranks[i] = i;
        valid_ranks = true;
    }

    for i: 1..count - 1 {
        moving := ranks[i];
        j := i;
        while j != 0 && input[moving] < input[ranks[j - 1]] {
            ranks[j] = ranks[j - 1];
            j -= 1;
        }
        if i != j  ranks[j] = moving;
    }
}

radix_sort :: (using radix: *RadixSort, input: *$T, count: u32) {
    // One pass per byte of the key.
    PASSES :: size_of(T);

    histogram: [256 * PASSES] u32;
    // Where the next index for each byte value goes.
    link: [256] *u32 = ---;

    // Every pass's counts come out of one walk over the input.
    bytes := cast(*u8) input;
    for i: 0..count - 1 {
        for pass: 0..PASSES - 1 {
            histogram[pass * 256 + bytes[i * PASSES + pass]] += 1;
        }
    }

    // Least significant byte first, so that each pass keeps the order the
    // previous ones established.
    for pass: 0..PASSES - 1 {
        counts := *histogram[pass * 256];

        // Little-endian, so byte `pass` of element `i` is at `i * PASSES +
        // pass`.
        pass_bytes := cast(*u8) input;
        pass_bytes += pass;

        // Every key has the same byte here, so the pass would only copy the
        // order across unchanged.
        if counts[pass_bytes[0]] == count  continue;

        link[0] = ranks2.data;
        for i: 1..255  link[i] = link[i - 1] + counts[i - 1];

        if !valid_ranks {
            for i: 0..count - 1 {
                link[pass_bytes[i * PASSES]].* = i;
                link[pass_bytes[i * PASSES]] += 1;
            }
            valid_ranks = true;
        } else {
            for i: 0..count - 1 {
                index := ranks[i];
                link[pass_bytes[index * PASSES]].* = index;
                link[pass_bytes[index * PASSES]] += 1;
            }
        }

        // What was just built becomes the order; the old one becomes scratch.
        ranks, ranks2 = ranks2, ranks;
    }

    // Every pass was skipped, which means every key is equal: the input's own
    // order will do.
    if !valid_ranks {
        for 0..count - 1  ranks[it] = it;
        valid_ranks = true;
    }
}
