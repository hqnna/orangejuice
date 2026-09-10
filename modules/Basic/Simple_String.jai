//
// The string operations that do not need the String module.
//
// A `string` is a count and a pointer at bytes it does not own (**L§3.4**), so
// everything here that produces one says which allocator it came from.
//

copy_string :: (s: string, allocator := context.allocator) -> string {
    result: string;
    if !s.count  return result;
    result.data = cast(*u8) alloc(s.count, allocator);
    if !result.data  return result;
    result.count = s.count;
    memcpy(result.data, s.data, s.count);
    return result;
}

// A copy in temporary storage, valid until the next reset (**L§10.4**).
tcopy_string :: (s: string) -> string {
    return copy_string(s, temp);
}

free_string :: (s: *string, allocator := context.allocator) {
    if s.data  free(s.data, allocator);
    s.data = null;
    s.count = 0;
}

// A copy with a zero after the last byte, for handing to C.
to_c_string :: (s: string, allocator := context.allocator) -> *u8 {
    memory := cast(*u8) alloc(s.count + 1, allocator);
    if !memory  return null;
    if s.count  memcpy(memory, s.data, s.count);
    memory[s.count] = 0;
    return memory;
}

// The bytes up to a zero, without copying them.
to_string :: (bytes: *u8) -> string {
    result: string;
    if !bytes  return result;
    result.data = bytes;
    while bytes[result.count]  result.count += 1;
    return result;
}

to_string :: (bytes: *u8, count: s64) -> string {
    result: string;
    result.data = bytes;
    result.count = count;
    return result;
}

// ----------------------------------------------------------- comparisons ---

equal :: (a: string, b: string) -> bool {
    if a.count != b.count  return false;
    if a.count == 0        return true;
    return memcmp(a.data, b.data, a.count) == 0;
}

equal_nocase :: (a: string, b: string) -> bool {
    if a.count != b.count  return false;
    for 0..a.count - 1 {
        if to_lower(a[it]) != to_lower(b[it])  return false;
    }
    return true;
}

// Negative when `a` sorts first, positive when `b` does, zero when they are
// the same string. A shared prefix is broken by the shorter one sorting first.
compare :: (a: string, b: string) -> s64 {
    shared := min(a.count, b.count);
    if shared {
        difference := memcmp(a.data, b.data, shared);
        if difference != 0  return cast(s64) difference;
    }
    return a.count - b.count;
}

begins_with :: (s: string, prefix: string) -> bool {
    if prefix.count > s.count  return false;
    if !prefix.count           return true;
    return memcmp(s.data, prefix.data, prefix.count) == 0;
}

ends_with :: (s: string, suffix: string) -> bool {
    if suffix.count > s.count  return false;
    if !suffix.count           return true;
    return memcmp(s.data + s.count - suffix.count, suffix.data, suffix.count) == 0;
}

contains :: (s: string, needle: string) -> bool {
    return find_index_from_left(s, needle) >= 0;
}

// ------------------------------------------------------------- searching ---

// Where `needle` starts, or -1. An empty needle is found at the beginning.
find_index_from_left :: (s: string, needle: string) -> s64 {
    if !needle.count           return 0;
    if needle.count > s.count  return -1;
    last := s.count - needle.count;
    for 0..last {
        if memcmp(s.data + it, needle.data, needle.count) == 0  return it;
    }
    return -1;
}

find_index_from_right :: (s: string, needle: string) -> s64 {
    if !needle.count           return s.count;
    if needle.count > s.count  return -1;
    at := s.count - needle.count;
    while at >= 0 {
        if memcmp(s.data + at, needle.data, needle.count) == 0  return at;
        at -= 1;
    }
    return -1;
}

find_index_from_left :: (s: string, byte: u8) -> s64 {
    for 0..s.count - 1  if s[it] == byte  return it;
    return -1;
}

find_index_from_right :: (s: string, byte: u8) -> s64 {
    at := s.count - 1;
    while at >= 0 {
        if s[at] == byte  return at;
        at -= 1;
    }
    return -1;
}

// -------------------------------------------------------------- reshaping ---

// A view of part of `s`, sharing its bytes rather than copying them. The range
// is clamped, so a slice past the end is empty rather than out of bounds.
slice :: (s: string, index: s64, count: s64) -> string {
    result: string;
    start := index;
    if start < 0        start = 0;
    if start > s.count  start = s.count;
    take := count;
    if take < 0                  take = 0;
    if start + take > s.count    take = s.count - start;
    result.data = s.data + start;
    result.count = take;
    return result;
}

trim_left :: (s: string, bytes := " \t\r\n") -> string {
    result := s;
    while result.count && is_any(result[0], bytes) {
        result.data += 1;
        result.count -= 1;
    }
    return result;
}

trim_right :: (s: string, bytes := " \t\r\n") -> string {
    result := s;
    while result.count && is_any(result[result.count - 1], bytes)  result.count -= 1;
    return result;
}

trim :: (s: string, bytes := " \t\r\n") -> string {
    return trim_right(trim_left(s, bytes), bytes);
}

join :: (parts: [] string, separator := "", allocator := context.allocator) -> string {
    total := 0;
    for parts  total += it.count;
    if parts.count > 1  total += separator.count * (parts.count - 1);

    result: string;
    if total <= 0  return result;
    result.data = cast(*u8) alloc(total, allocator);
    if !result.data  return result;
    result.count = total;

    at := 0;
    for parts {
        if it_index && separator.count {
            memcpy(result.data + at, separator.data, separator.count);
            at += separator.count;
        }
        if it.count {
            memcpy(result.data + at, it.data, it.count);
            at += it.count;
        }
    }
    return result;
}

// --------------------------------------------------------------- the bytes ---

is_any :: (byte: u8, set: string) -> bool {
    for 0..set.count - 1  if set[it] == byte  return true;
    return false;
}

is_space :: (byte: u8) -> bool {
    return byte == #char " " || byte == #char "\t" || byte == #char "\n" || byte == #char "\r";
}

is_digit :: (byte: u8) -> bool {
    return byte >= #char "0" && byte <= #char "9";
}

is_alpha :: (byte: u8) -> bool {
    return (byte >= #char "a" && byte <= #char "z") || (byte >= #char "A" && byte <= #char "Z");
}

is_alnum :: (byte: u8) -> bool {
    return is_alpha(byte) || is_digit(byte);
}

to_lower :: (byte: u8) -> u8 {
    if byte >= #char "A" && byte <= #char "Z"  return byte + 32;
    return byte;
}

to_upper :: (byte: u8) -> u8 {
    if byte >= #char "a" && byte <= #char "z"  return byte - 32;
    return byte;
}

// ------------------------------------------------------------- to numbers ---

// The leading integer, and whether there was one. The rest of the string comes
// back too, so a caller parsing several in a row does not have to count.
string_to_int :: (s: string) -> (value: s64, success: bool, remainder: string) {
    at := 0;
    negative := false;
    if at < s.count && (s[at] == #char "-" || s[at] == #char "+") {
        negative = s[at] == #char "-";
        at += 1;
    }

    start := at;
    value: s64 = 0;
    while at < s.count && is_digit(s[at]) {
        value = value * 10 + cast(s64) (s[at] - #char "0");
        at += 1;
    }
    if at == start  return 0, false, s;
    if negative  value = -value;
    return value, true, slice(s, at, s.count - at);
}
