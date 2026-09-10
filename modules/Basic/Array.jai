//
// Resizable arrays (**L§3.3**).
//
// A `[..] T` is a count, a pointer, how much room it has, and the allocator
// that gave it that room — so an array grown in one procedure can be freed in
// another without anyone passing the allocator along.
//

array_add :: (array: *[..] $T, value: T) {
    array_ensure_room(array, 1);
    if array.count >= array.allocated  return;
    array.data[array.count] = value;
    array.count += 1;
}

// The new slot's address, so the caller can fill it in place rather than
// building a `T` and copying it.
array_add :: (array: *[..] $T) -> *T {
    array_ensure_room(array, 1);
    if array.count >= array.allocated  return null;
    result := *array.data[array.count];
    array.count += 1;
    memset(result, 0, size_of(T));
    return result;
}

// Several at once. Overload resolution picks the single-item form for a
// single item, so this only takes over when there really are several.
array_add :: (array: *[..] $T, to_append: ..T) {
    if !to_append.count  return;
    array_reserve(array, array.count + to_append.count);
    for to_append  array_add(array, it);
}

array_reserve :: (array: *[..] $T, wanted: s64) {
    if wanted <= array.allocated  return;
    array_grow(array, wanted);
}

array_resize :: (array: *[..] $T, count: s64) {
    if count > array.allocated  array_grow(array, count);
    if count > array.count && array.data {
        memset(array.data + array.count, 0, (count - array.count) * size_of(T));
    }
    if count <= array.allocated  array.count = count;
}

// Keeps the order of what is left, at the cost of moving everything after the
// hole down one.
array_ordered_remove_by_index :: (array: *[..] $T, index: s64) {
    if index < 0 || index >= array.count  return;
    moving := array.count - index - 1;
    if moving  memcpy(array.data + index, array.data + index + 1, moving * size_of(T));
    array.count -= 1;
}

// Fills the hole with the last element, which is cheaper and reorders.
array_unordered_remove_by_index :: (array: *[..] $T, index: s64) {
    if index < 0 || index >= array.count  return;
    array.data[index] = array.data[array.count - 1];
    array.count -= 1;
}

array_ordered_remove_by_value :: (array: *[..] $T, value: T) -> bool {
    index := array_find_index(array.*, value);
    if index < 0  return false;
    array_ordered_remove_by_index(array, index);
    return true;
}

array_unordered_remove_by_value :: (array: *[..] $T, value: T) -> bool {
    index := array_find_index(array.*, value);
    if index < 0  return false;
    array_unordered_remove_by_index(array, index);
    return true;
}

array_find_index :: (array: [] $T, value: T) -> s64 {
    for array  if it == value  return it_index;
    return -1;
}

// Whether it is there, and where. The index is -1 when it is not, so a caller
// wanting only the answer can ignore the second value.
array_find :: (array: [] $T, value: T) -> (found: bool, index: s64) {
    index := array_find_index(array, value);
    return index >= 0, index;
}

// Adds it unless it is already there. Returns whether it was newly added, and
// where it sits either way.
array_add_if_unique :: (array: *[..] $T, item: T) -> (added: bool, index: s64) {
    found, index := array_find(array.*, item);
    if found  return false, index;
    array_add(array, item);
    return true, array.count - 1;
}

// The last element, removed. The caller is responsible for there being one:
// popping an empty array is a bug in the caller, not something to paper over.
pop :: (array: *[] $T) -> T {
    result := array.*[array.count - 1];
    array.count -= 1;
    return result;
}

pop :: (array: *[..] $T) -> T {
    result := array.*[array.count - 1];
    array.count -= 1;
    return result;
}

// Keeps the storage; only the count goes to zero.
array_reset_keeping_memory :: (array: *[..] $T) {
    array.count = 0;
}

array_reset :: (array: *[..] $T) {
    array_free(array);
}

array_free :: (array: *[..] $T) {
    if array.data  free(array.data, array_allocator(array));
    array.data = null;
    array.count = 0;
    array.allocated = 0;
}

array_copy :: (array: [] $T, allocator := context.allocator) -> [] T {
    result: [] T;
    if !array.count  return result;
    result.data = cast(*T) alloc(array.count * size_of(T), allocator);
    if !result.data  return result;
    result.count = array.count;
    memcpy(result.data, array.data, array.count * size_of(T));
    return result;
}

// A view over the same elements; nothing is copied.
array_view :: (array: [] $T, index: s64, count: s64) -> [] T {
    result: [] T;
    start := index;
    if start < 0            start = 0;
    if start > array.count  start = array.count;
    take := count;
    if take < 0                   take = 0;
    if start + take > array.count take = array.count - start;
    result.data = array.data + start;
    result.count = take;
    return result;
}

// Puts `item` at `index`, moving everything from there along. An index past
// the end appends.
array_insert_at :: (array: *[..] $T, item: T, index: s64) {
    at := index;
    if at < 0            at = 0;
    if at > array.count  at = array.count;

    array_ensure_room(array, 1);
    if array.count >= array.allocated  return;

    moving := array.count - at;
    if moving  memmove_bytes(array.data + at + 1, array.data + at, moving * size_of(T));
    array.data[at] = item;
    array.count += 1;
}

// A slice's storage back to the allocator. A `[] T` does not remember which
// allocator gave it its bytes, so this takes the context's unless told.
array_free :: inline (array: [] $T, allocator := context.allocator) {
    if array.data  free(array.data, allocator);
}

// A resizable array by value: it carries its own allocator, so nothing has to
// be passed. The caller's copy still holds the stale pointer afterwards, which
// is why the pointer form above is the one to reach for when the array lives
// on past the free.
array_free :: inline (array: [..] $T) {
    if array.data  free(array.data, ifx array.allocator.proc then array.allocator else context.allocator);
}

// Replaces what `dest` holds with a copy of `source`, reusing `dest`'s storage
// when it is already big enough.
array_copy :: (dest: *[..] $T, source: [] T) {
    array_reset_keeping_memory(dest);
    if !source.count  return;
    array_reserve(dest, source.count);
    if dest.allocated < source.count  return;
    memcpy(dest.data, source.data, source.count * size_of(T));
    dest.count = source.count;
}

#scope_module

array_allocator :: (array: *[..] $T) -> Allocator {
    if array.allocator.proc  return array.allocator;
    return context.allocator;
}

array_ensure_room :: (array: *[..] $T, extra: s64) {
    wanted := array.count + extra;
    if wanted <= array.allocated  return;
    array_grow(array, wanted);
}

FIRST_ALLOCATION :: 8;

// Doubling, so that adding one element at a time costs a constant amount each
// on average rather than growing quadratically.
array_grow :: (array: *[..] $T, wanted: s64) {
    allocator := array_allocator(array);

    allocated := array.allocated;
    if allocated < FIRST_ALLOCATION  allocated = FIRST_ALLOCATION;
    while allocated < wanted  allocated *= 2;

    old_bytes := array.allocated * size_of(T);
    new_bytes := allocated * size_of(T);

    memory: *void;
    if array.data  memory = realloc(array.data, new_bytes, old_bytes, allocator);
    else           memory = alloc(new_bytes, allocator);
    if !memory  return;

    array.data = cast(*T) memory;
    array.allocated = allocated;
    array.allocator = allocator;
}

// The ranges overlap, so this copies backwards.
memmove_bytes :: (destination: *void, source: *void, count: s64) {
    to := cast(*u8) destination;
    from := cast(*u8) source;
    if to < from || to >= from + count {
        memcpy(destination, source, count);
        return;
    }
    index := count - 1;
    while index >= 0 {
        to[index] = from[index];
        index -= 1;
    }
}
