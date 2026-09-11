//
// String_Builder: bytes appended without knowing the total up front.
//
// The first buffer lives *inside* the builder, so a build that stays under
// `STRING_BUILDER_BUFFER_SIZE` allocates nothing at all — which is most of
// them, and which is why `builder: String_Builder;` needs no initialisation
// before the first `append`.
//
// Past that it is a chain of buffers rather than one block that is reallocated,
// so a long build never copies what it has already written. Each buffer's bytes
// follow its header in the same allocation, which is what lets a builder be
// copied from one place to another without any pointer inside it having to be
// adjusted.
//

STRING_BUILDER_BUFFER_SIZE :: 4096 - size_of(String_Builder.Buffer);

String_Builder :: struct {
    Buffer :: struct {
        count:     s64;
        allocated: s64;

        // How far a caller promised to write when it asked for contiguous
        // space, so that `advance_through_ensured_space` can check it did.
        #if ENABLE_ASSERT  ensured_count: s64;

        next: *Buffer;
        // The bytes follow this header; `get_buffer_data` finds them. Storing
        // the pointer would mean fixing it up whenever a builder is moved.
    }

    initialized            := false;
    // Set when a buffer could not be allocated: the result is then whatever
    // was written before that, rather than a silent truncation nobody sees.
    failed                 := false;
    subsequent_buffer_size := -1;

    allocator: Allocator;

    // Null means the base buffer is the one being written to. It is not
    // pointed at `initial_bytes` so that a builder stays valid when copied.
    current_buffer: *Buffer;
    initial_bytes:  [STRING_BUILDER_BUFFER_SIZE] u8;
}

// Optional: the first `append` does this if nothing else has. `buffer_size`
// sizes the buffers taken *after* the built-in one. To choose the allocator,
// assign `builder.allocator` before appending anything.
init_string_builder :: (using builder: *String_Builder, buffer_size := -1) {
    if !allocator.proc  allocator = context.allocator;

    subsequent_buffer_size = ifx buffer_size > 0 then buffer_size else STRING_BUILDER_BUFFER_SIZE;

    base := get_base_buffer(builder);
    base.count = 0;
    base.allocated = initial_bytes.count - size_of(Buffer);
    base.next = null;

    current_buffer = null;
    initialized = true;
}

// Frees every buffer past the built-in one and empties the builder, which is
// what makes it usable again.
reset :: (using builder: *String_Builder) {
    if !initialized  return;

    base := get_base_buffer(builder);
    buffer := base.next;
    while buffer {
        next := buffer.next;
        free(buffer, allocator);
        buffer = next;
    }

    base.count = 0;
    base.next = null;
    current_buffer = null;
    failed = false;
}

// The older name for `reset`, kept because so much code says
// `defer free_buffers(*builder)`.
free_buffers :: (using builder: *String_Builder) {
    reset(builder);
}

// ------------------------------------------------------------- appending ---

append :: (using builder: *String_Builder, s: *u8, length: s64) {
    if !initialized  init_string_builder(builder);

    remaining := length;
    from := s;
    while remaining > 0 {
        buffer := get_current_buffer(builder);

        room := buffer.allocated - buffer.count;
        if room <= 0 {
            if !expand(builder) {
                failed = true;
                return;
            }
            buffer = current_buffer;
            room = buffer.allocated - buffer.count;
        }

        take := min(remaining, room);
        memcpy(get_buffer_data(buffer) + buffer.count, from, take);
        buffer.count += take;

        remaining -= take;
        from += take;
    }
}

append :: inline (builder: *String_Builder, s: string) {
    if !s.count  return;
    append(builder, s.data, s.count);
}

append :: inline (builder: *String_Builder, byte: u8) {
    value := byte;
    append(builder, *value, 1);
}

// The bytes of one value, whatever it is — for writing a struct or a number
// into a buffer that is about to become a file.
append_by_pointer :: inline (builder: *String_Builder, pointer: *$T) {
    append(builder, cast(*u8) pointer, size_of(T));
}

simple_memcpy :: inline (using builder: *String_Builder, x: $T) {
    value := x;
    append(builder, cast(*u8) *value, size_of(T));
}

// Moves everything `steal_from` holds into `builder`, taking ownership of its
// dynamically allocated buffers instead of copying them. The base buffer has
// to be copied, since it is part of the other builder's own storage.
append_and_steal_buffers :: (builder: *String_Builder, steal_from: *String_Builder) {
    if !steal_from.initialized  return;

    base := get_base_buffer(steal_from);
    if base.count  append(builder, get_buffer_data(base), base.count);

    stolen := base.next;
    if stolen {
        // Both builders must free with the same allocator, or the buffers
        // handed over could not be freed by their new owner.
        assert(builder.allocator.proc == steal_from.allocator.proc);
        assert(builder.allocator.data == steal_from.allocator.data);

        if !builder.initialized  init_string_builder(builder);
        get_current_buffer(builder).next = stolen;

        last := stolen;
        while last.next  last = last.next;
        builder.current_buffer = last;
    }

    base.count = 0;
    base.next = null;
    steal_from.current_buffer = null;
}

// ------------------------------------------------------ contiguous space ---

// Room for `bytes` bytes in one piece, so a caller can write into it directly
// rather than building the bytes elsewhere and appending them. Null when the
// request is bigger than a buffer. Follow the writing with
// `advance_through_ensured_space`.
ensure_contiguous_space :: (using builder: *String_Builder, bytes: s64) -> (pointer: *void) {
    available := ensure_contiguous_space_and_return_available_range(builder, bytes);
    if !available.count  return null;

    #if ENABLE_ASSERT {
        buffer := get_current_buffer(builder);
        buffer.ensured_count = buffer.count + bytes;
    }
    return available.data;
}

// The same, handing back everything that is free in the buffer rather than
// only what was asked for.
ensure_contiguous_space_and_return_available_range :: (using builder: *String_Builder, minimum_bytes := 1) -> [] u8 {
    if !initialized  init_string_builder(builder);

    empty: [] u8;
    buffer := get_current_buffer(builder);
    // Bigger than a buffer is not something more buffers can fix.
    if minimum_bytes > buffer.allocated  return empty;

    if buffer.allocated - buffer.count < minimum_bytes {
        if !expand(builder)  return empty;
        buffer = get_current_buffer(builder);
    }

    #if ENABLE_ASSERT  buffer.ensured_count = buffer.allocated;

    result: [] u8;
    result.data = get_buffer_data(buffer) + buffer.count;
    result.count = buffer.allocated - buffer.count;
    return result;
}

// Says how much of the ensured space was actually written.
advance_through_ensured_space :: (using builder: *String_Builder, bytes: s64) {
    buffer := get_current_buffer(builder);
    assert(buffer.allocated - buffer.count >= bytes);
    // Checking against what was promised rather than against what happens to
    // be free catches the caller who never ensured the space at all.
    #if ENABLE_ASSERT  assert(buffer.count + bytes <= buffer.ensured_count);
    buffer.count += bytes;
}

// Another buffer on the end of the chain. False when the allocator had none.
expand :: (using builder: *String_Builder) -> bool {
    if !initialized  init_string_builder(builder);

    size := ifx subsequent_buffer_size > 0 then subsequent_buffer_size else STRING_BUILDER_BUFFER_SIZE;
    memory := alloc(size_of(Buffer) + size, allocator);
    if !memory  return false;

    buffer := cast(*Buffer) memory;
    buffer.next = null;
    buffer.count = 0;
    buffer.allocated = size;

    get_current_buffer(builder).next = buffer;
    current_buffer = buffer;
    return true;
}

// --------------------------------------------------------------- reading ---

builder_string_length :: (using builder: *String_Builder) -> int {
    if !initialized  return 0;
    total := 0;
    buffer := get_base_buffer(builder);
    while buffer {
        total += buffer.count;
        buffer = buffer.next;
    }
    return total;
}

// Everything built, gathered into one allocation from `context.allocator`.
//
// `extra_bytes_to_prepend` leaves room at the front for something the caller
// did not know when it started writing — a length, most often. The builder is
// reset afterwards unless told not to, so its buffers are not leaked.
builder_to_string :: (builder: *String_Builder, extra_bytes_to_prepend := 0, do_reset := true) -> string {
    if !builder.initialized  init_string_builder(builder);

    count := builder_string_length(builder);
    if !count && !extra_bytes_to_prepend  return "";

    result: string;
    result.data = alloc(count + extra_bytes_to_prepend);
    if !result.data  return "";
    result.count = count + extra_bytes_to_prepend;

    at := result.data + extra_bytes_to_prepend;
    buffer := get_base_buffer(builder);
    while buffer {
        if buffer.count  memcpy(at, get_buffer_data(buffer), buffer.count);
        at += buffer.count;
        buffer = buffer.next;
    }

    if do_reset  reset(builder);
    return result;
}

// Writes what has been built straight out, without gathering it into one
// allocation first.
write_builder :: (using builder: *String_Builder, do_reset := true, to_standard_error := false) -> s64 {
    if !initialized  return 0;

    written := 0;
    buffer := get_base_buffer(builder);
    while buffer {
        if buffer.count {
            text: string;
            text.data = get_buffer_data(buffer);
            text.count = buffer.count;
            write_string(text, to_standard_error);
            written += buffer.count;
        }
        buffer = buffer.next;
    }

    if do_reset  reset(builder);
    return written;
}

// The buffer the builder starts with, which sits in its own storage.
get_base_buffer :: inline (using builder: *String_Builder) -> *String_Builder.Buffer {
    return cast(*String_Builder.Buffer) initial_bytes.data;
}

get_current_buffer :: inline (using builder: *String_Builder) -> *String_Builder.Buffer {
    if current_buffer  return current_buffer;
    return get_base_buffer(builder);
}

// A buffer's bytes follow its header in the same allocation.
get_buffer_data :: inline (using buffer: *String_Builder.Buffer) -> *u8 {
    return cast(*u8) buffer + size_of(String_Builder.Buffer);
}
