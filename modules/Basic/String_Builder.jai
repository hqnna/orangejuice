//
// String_Builder: bytes appended without knowing the total up front.
//
// A chain of fixed buckets rather than one block that is reallocated, so a
// pointer handed out by an earlier append stays valid and a long build does
// not copy what it has already written.
//

String_Builder :: struct {
    BUCKET_SIZE :: 4096;

    Bucket :: struct {
        next:     *Bucket;
        occupied: s64;
        size:     s64;
        // The bytes follow the header in the same allocation.
    }

    first:     *Bucket;
    current:   *Bucket;
    allocator: Allocator;
    // Set when a bucket could not be allocated: the result is then whatever
    // was written before that, rather than a silent truncation nobody sees.
    failed: bool;
}

init_string_builder :: (builder: *String_Builder, allocator := context.allocator) {
    builder.first = null;
    builder.current = null;
    builder.allocator = allocator;
    builder.failed = false;
}

reset :: (builder: *String_Builder) {
    bucket := builder.first;
    while bucket {
        next := bucket.next;
        free(bucket, builder.allocator);
        bucket = next;
    }
    builder.first = null;
    builder.current = null;
    builder.failed = false;
}

append :: (builder: *String_Builder, s: string) {
    if !s.count  return;
    written := 0;
    while written < s.count {
        bucket := builder_bucket_for(builder, s.count - written);
        if !bucket  return;
        room := bucket.size - bucket.occupied;
        take := min(room, s.count - written);
        memcpy(bucket_data(bucket) + bucket.occupied, s.data + written, take);
        bucket.occupied += take;
        written += take;
    }
}

append :: (builder: *String_Builder, byte: u8) {
    one := byte;
    text: string;
    text.data = *one;
    text.count = 1;
    append(builder, text);
}

builder_string_length :: (builder: *String_Builder) -> s64 {
    total := 0;
    bucket := builder.first;
    while bucket {
        total += bucket.occupied;
        bucket = bucket.next;
    }
    return total;
}

// The bytes as one string, in storage the given allocator owns.
builder_to_string :: (builder: *String_Builder, allocator := context.allocator) -> string {
    result: string;
    total := builder_string_length(builder);
    if total <= 0  return result;

    result.data = cast(*u8) alloc(total, allocator);
    if !result.data  return result;
    result.count = total;

    at := 0;
    bucket := builder.first;
    while bucket {
        if bucket.occupied {
            memcpy(result.data + at, bucket_data(bucket), bucket.occupied);
            at += bucket.occupied;
        }
        bucket = bucket.next;
    }
    return result;
}

// Writes what has been built straight out, without gathering it into one
// allocation first.
write_builder :: (builder: *String_Builder, to_standard_error := false) {
    bucket := builder.first;
    while bucket {
        if bucket.occupied {
            text: string;
            text.data = bucket_data(bucket);
            text.count = bucket.occupied;
            write_string(text, to_standard_error);
        }
        bucket = bucket.next;
    }
}

// The buckets go back to the allocator. `builder_to_string` has already copied
// what was in them, which is why the usual shape is
//
//     builder: String_Builder;
//     defer free_buffers(*builder);
//
free_buffers :: (builder: *String_Builder) {
    reset(builder);
}

#scope_module

bucket_data :: (bucket: *String_Builder.Bucket) -> *u8 {
    return cast(*u8) bucket + size_of(String_Builder.Bucket);
}

// The bucket the next `wanted` bytes start in: the current one while it has
// room, otherwise a fresh one at least big enough to make progress.
builder_bucket_for :: (builder: *String_Builder, wanted: s64) -> *String_Builder.Bucket {
    if builder.failed  return null;
    if builder.current && builder.current.occupied < builder.current.size  return builder.current;

    allocator := builder.allocator;
    if !allocator.proc  allocator = context.allocator;

    size := max(wanted, String_Builder.BUCKET_SIZE);
    memory := alloc(size_of(String_Builder.Bucket) + size, allocator);
    if !memory {
        builder.failed = true;
        return null;
    }

    bucket := cast(*String_Builder.Bucket) memory;
    bucket.next = null;
    bucket.occupied = 0;
    bucket.size = size;

    if builder.current  builder.current.next = bucket;
    else                builder.first = bucket;
    builder.current = bucket;
    builder.allocator = allocator;
    return bucket;
}
