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

array_find :: (array: [] $T, value: T) -> bool {
    return array_find_index(array, value) >= 0;
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
