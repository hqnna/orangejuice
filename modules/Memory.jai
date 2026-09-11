//
// Moving and comparing bytes, and the arithmetic of alignment.
//
// `memcpy`, `memset` and `memcmp` are intrinsics the compiler knows
// (**L§17**); what is here is the rest — an overlapping move, and the
// rounding every allocator does.
//

#import "Basic";

// Copies where the ranges may overlap, which `memcpy` does not promise.
// Copying backwards when the destination is inside the source is what keeps a
// forward shift from overwriting bytes it has not read yet.
memmove :: (dest: *void, source: *void, count: s64) {
    if count <= 0 || dest == source  return;

    to := cast(*u8) dest;
    from := cast(*u8) source;
    if to < from || to >= from + count {
        memcpy(dest, source, count);
        return;
    }
    index := count - 1;
    while index >= 0 {
        to[index] = from[index];
        index -= 1;
    }
}

// ------------------------------------------------------------- alignment ---

// The next multiple of `alignment` at or above `value`. `alignment` must be a
// power of two, which every allocator's is.
align_forward :: (value: s64, alignment: s64) -> s64 {
    if alignment <= 1  return value;
    return (value + alignment - 1) & ~(alignment - 1);
}

align_forward :: (pointer: *void, alignment: s64) -> *void {
    return cast(*void) align_forward(cast(s64) pointer, alignment);
}

is_aligned :: (pointer: *void, alignment: s64) -> bool {
    if alignment <= 1  return true;
    return (cast(s64) pointer & (alignment - 1)) == 0;
}

// ---------------------------------------------------------------- copying ---

// Storage holding a copy of the bytes, from the given allocator.
copy_memory :: (source: *void, count: s64, allocator := context.allocator) -> *void {
    if count <= 0  return null;
    memory := alloc(count, allocator);
    if memory  memcpy(memory, source, count);
    return memory;
}

// Whether two ranges hold the same bytes.
memory_equal :: (a: *void, b: *void, count: s64) -> bool {
    if count <= 0  return true;
    return memcmp(a, b, count) == 0;
}

// Where `byte` first occurs, or -1.
find_byte :: (haystack: *u8, count: s64, byte: u8) -> s64 {
    for 0..count - 1  if haystack[it] == byte  return it;
    return -1;
}

// Bytes from one place to another. The ranges must not overlap; use
// `memmove`-style copying when they might.
copy :: (destination: *void, source: *void, count: s64) {
    if count <= 0  return;
    memcpy(destination, source, count);
}
