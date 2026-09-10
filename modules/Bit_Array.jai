//
// Bit_Array: one bit per element, packed into words.
//
// A `[..] bool` spends a byte on each; this spends an eighth of one, which
// matters once there are millions. Set operations are word-at-a-time, so a
// union over a million bits is sixteen thousand `or`s.
//

#import "Basic";
#import "Bit_Operations";

Bit_Array :: struct {
    BITS_PER_WORD :: 64;
    // Typed, because an untyped literal this wide has no type the front end
    // can settle on inside a shift.
    ALL_ONES : u64 : 0xffff_ffff_ffff_ffff;

    words: [..] u64;
    // How many bits the caller asked for, which is not the same as how many
    // the words hold.
    count: s64;
}

init :: (array: *Bit_Array, count: s64, allocator := context.allocator) {
    array.words.allocator = allocator;
    array_reset_keeping_memory(*array.words);
    array.count = max(count, 0);
    array_resize(*array.words, words_for(array.count));
    for * array.words  it.* = 0;
}

deinit :: (array: *Bit_Array) {
    array_free(*array.words);
    array.count = 0;
}

// Grows with zeroes; shrinking drops the bits past the new end.
resize :: (array: *Bit_Array, count: s64) {
    wanted := max(count, 0);
    needed := words_for(wanted);
    old_words := array.words.count;
    array_resize(*array.words, needed);
    for index: old_words..needed - 1  array.words[index] = 0;
    array.count = wanted;
    clear_tail(array);
}

set :: (array: *Bit_Array, index: s64) {
    if index < 0 || index >= array.count  return;
    array.words[index / array.BITS_PER_WORD] |= mask(index);
}

unset :: (array: *Bit_Array, index: s64) {
    if index < 0 || index >= array.count  return;
    array.words[index / array.BITS_PER_WORD] &= ~mask(index);
}

// Sets or clears according to `value`.
set :: (array: *Bit_Array, index: s64, value: bool) {
    if value  set(array, index);
    else      unset(array, index);
}

toggle :: (array: *Bit_Array, index: s64) {
    if index < 0 || index >= array.count  return;
    array.words[index / array.BITS_PER_WORD] ^= mask(index);
}

// Out of range reads as false rather than as an error, so a caller probing
// past the end does not have to guard first.
get :: (array: *Bit_Array, index: s64) -> bool {
    if index < 0 || index >= array.count  return false;
    return (array.words[index / array.BITS_PER_WORD] & mask(index)) != 0;
}

set_all :: (array: *Bit_Array) {
    for * array.words  it.* = Bit_Array.ALL_ONES;
    clear_tail(array);
}

unset_all :: (array: *Bit_Array) {
    for * array.words  it.* = 0;
}

// How many bits are set.
count_set :: (array: *Bit_Array) -> s64 {
    total := 0;
    for array.words  total += popcount(it);
    return total;
}

// The lowest set bit at or after `from`, or -1. Skipping whole empty words is
// what makes walking a sparse array cheap.
next_set_bit :: (array: *Bit_Array, from: s64 = 0) -> s64 {
    start := max(from, 0);
    if start >= array.count  return -1;

    word_index := start / array.BITS_PER_WORD;
    // Ignore the bits below `start` in the word it falls in.
    word := array.words[word_index] & (Bit_Array.ALL_ONES << cast(u64) (start % array.BITS_PER_WORD));
    while true {
        if word {
            found := word_index * array.BITS_PER_WORD + count_trailing_zeros(word);
            return ifx found < array.count then found else -1;
        }
        word_index += 1;
        if word_index >= array.words.count  return -1;
        word = array.words[word_index];
    }
    return -1;
}

// ------------------------------------------------------ set operations ---

// Each writes into `a`, and treats a shorter `b` as zeroes beyond its end.
or_with :: (a: *Bit_Array, b: *Bit_Array) {
    shared := min(a.words.count, b.words.count);
    for index: 0..shared - 1  a.words[index] |= b.words[index];
    clear_tail(a);
}

and_with :: (a: *Bit_Array, b: *Bit_Array) {
    shared := min(a.words.count, b.words.count);
    for index: 0..shared - 1  a.words[index] &= b.words[index];
    for index: shared..a.words.count - 1  a.words[index] = 0;
}

xor_with :: (a: *Bit_Array, b: *Bit_Array) {
    shared := min(a.words.count, b.words.count);
    for index: 0..shared - 1  a.words[index] ^= b.words[index];
    clear_tail(a);
}

// Everything in `a` that is not in `b`.
subtract :: (a: *Bit_Array, b: *Bit_Array) {
    shared := min(a.words.count, b.words.count);
    for index: 0..shared - 1  a.words[index] &= ~b.words[index];
}

#scope_module

words_for :: (count: s64) -> s64 {
    return (count + Bit_Array.BITS_PER_WORD - 1) / Bit_Array.BITS_PER_WORD;
}

mask :: (index: s64) -> u64 {
    return cast(u64) 1 << cast(u64) (index % Bit_Array.BITS_PER_WORD);
}

// The last word holds bits past `count`. They must stay zero or `count_set`
// and `next_set_bit` would see bits nobody asked for.
clear_tail :: (array: *Bit_Array) {
    if !array.words.count  return;
    spare := array.words.count * Bit_Array.BITS_PER_WORD - array.count;
    if spare <= 0  return;
    array.words[array.words.count - 1] &= Bit_Array.ALL_ONES >> cast(u64) spare;
}
