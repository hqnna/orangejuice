//
// String: the operations that want an allocator or a result array.
//
// The comparison and search primitives live in Basic, since so much depends on
// them; what is here is the rest — splitting, replacing, changing case, and
// the paths a filename is made of.
//

#import "Basic";

// ------------------------------------------------------------- splitting ---

// Every piece between occurrences of `separator`, including empty ones, so
// that `split("a,,b", ",")` is three pieces. The pieces are views into `s` and
// own nothing; the array holding them is the caller's.
split :: (s: string, separator: string, allocator := context.allocator) -> [] string {
    pieces: [..] string;
    pieces.allocator = allocator;

    if !separator.count {
        array_add(*pieces, s);
        return pieces;
    }

    start := 0;
    at := 0;
    while at <= s.count - separator.count {
        if memcmp(s.data + at, separator.data, separator.count) == 0 {
            array_add(*pieces, slice(s, start, at - start));
            at += separator.count;
            start = at;
            continue;
        }
        at += 1;
    }
    // Whatever follows the last separator, which is the empty string when the
    // input ended on one.
    array_add(*pieces, slice(s, start, s.count - start));
    return pieces;
}

split :: (s: string, separator: u8, allocator := context.allocator) -> [] string {
    one := separator;
    text: string;
    text.data = *one;
    text.count = 1;
    return split(s, text, allocator);
}

// The two halves either side of the first `separator`. `found` says whether
// there was one; when there was not, `left` is the whole string.
split_once :: (s: string, separator: string) -> (left: string, right: string, found: bool) {
    at := find_index_from_left(s, separator);
    if at < 0  return s, "", false;
    return slice(s, 0, at), slice(s, at + separator.count, s.count), true;
}

// Splits on runs of whitespace, dropping empty pieces — what a command line or
// a config line usually wants.
split_on_whitespace :: (s: string, allocator := context.allocator) -> [] string {
    pieces: [..] string;
    pieces.allocator = allocator;

    at := 0;
    while at < s.count {
        while at < s.count && is_space(s[at])  at += 1;
        if at >= s.count  break;
        start := at;
        while at < s.count && !is_space(s[at])  at += 1;
        array_add(*pieces, slice(s, start, at - start));
    }
    return pieces;
}

// ------------------------------------------------------------- rewriting ---

replace :: (s: string, old: string, new: string, allocator := context.allocator) -> string {
    if !old.count  return copy_string(s, allocator);

    builder: String_Builder;
    init_string_builder(*builder, temp);

    at := 0;
    while at < s.count {
        if at <= s.count - old.count && memcmp(s.data + at, old.data, old.count) == 0 {
            append(*builder, new);
            at += old.count;
            continue;
        }
        append(*builder, s[at]);
        at += 1;
    }
    return builder_to_string(*builder, allocator);
}

to_lower_copy :: (s: string, allocator := context.allocator) -> string {
    result := copy_string(s, allocator);
    for 0..result.count - 1  result[it] = to_lower(result[it]);
    return result;
}

to_upper_copy :: (s: string, allocator := context.allocator) -> string {
    result := copy_string(s, allocator);
    for 0..result.count - 1  result[it] = to_upper(result[it]);
    return result;
}

// In place, which is what a string the caller already owns usually wants.
to_lower_in_place :: (s: string) {
    for 0..s.count - 1  s[it] = to_lower(s[it]);
}

to_upper_in_place :: (s: string) {
    for 0..s.count - 1  s[it] = to_upper(s[it]);
}

// ---------------------------------------------------------------- counting ---

count_occurrences :: (s: string, needle: string) -> s64 {
    if !needle.count  return 0;
    total := 0;
    at := 0;
    while at <= s.count - needle.count {
        if memcmp(s.data + at, needle.data, needle.count) == 0 {
            total += 1;
            at += needle.count;
            continue;
        }
        at += 1;
    }
    return total;
}

// ------------------------------------------------------------ file paths ---

// Everything after the last separator: `path_filename("a/b/c.jai")` is
// "c.jai". A path ending in a separator has no filename.
path_filename :: (path: string) -> string {
    at := find_index_from_right(path, #char "/");
    if at < 0  return path;
    return slice(path, at + 1, path.count);
}

// Everything before the last separator, without it. A path with none is "".
path_directory :: (path: string) -> string {
    at := find_index_from_right(path, #char "/");
    if at < 0  return "";
    return slice(path, 0, at);
}

// The part after the last dot of the filename, without the dot. A name whose
// only dot starts it — ".config" — has no extension.
path_extension :: (path: string) -> string {
    name := path_filename(path);
    at := find_index_from_right(name, #char ".");
    if at <= 0  return "";
    return slice(name, at + 1, name.count);
}

// The path with its extension taken off, directory and all:
// `path_strip_extension("/home/w/rezrov.foo")` is "/home/w/rezrov".
path_strip_extension :: (path: string) -> string {
    name := path_filename(path);
    at := find_index_from_right(name, #char ".");
    if at <= 0  return path;
    // The dot is at `at` within the filename, so its index in the whole path
    // is that plus wherever the filename began.
    return slice(path, 0, (name.data - path.data) + at);
}

// The directory a path names, *with* its trailing separator:
// `path_strip_filename("/home/w/rezrov.foo")` is "/home/w/".
path_strip_filename :: (path: string) -> string {
    at := find_index_from_right(path, #char "/");
    if at < 0  return "";
    return slice(path, 0, at + 1);
}

// Where any of `bytes` last occurs, or -1.
find_index_of_any_from_right :: (s: string, bytes: string) -> s64 {
    at := s.count - 1;
    while at >= 0 {
        if is_any(s[at], bytes)  return at;
        at -= 1;
    }
    return -1;
}

// Joins with a single separator between parts, however many the parts came
// with: `path_join("a/", "/b")` is "a/b".
path_join :: (parts: .. string, allocator := context.allocator) -> string {
    builder: String_Builder;
    init_string_builder(*builder, temp);

    written := 0;
    for parts {
        piece := it;
        if written  piece = trim_left(piece, "/");
        piece = trim_right(piece, "/");
        if !piece.count  continue;
        if written  append(*builder, #char "/");
        append(*builder, piece);
        written += 1;
    }
    return builder_to_string(*builder, allocator);
}


// The last place `separator` occurs splits the string. `found` is false when
// it does not occur at all, and then `left` is the whole string and `right` is
// empty — which is what a caller peeling a suffix off wants.
//
// Nothing is allocated: both halves are views into `s`.
split_from_right :: (s: string, separator: string) -> (found: bool, left: string, right: string) {
    at := find_index_from_right(s, separator);
    if at < 0  return false, s, "";
    return true, slice(s, 0, at), slice(s, at + separator.count, s.count - at - separator.count);
}

split_from_right :: (s: string, separator: u8) -> (found: bool, left: string, right: string) {
    at := find_index_from_right(s, separator);
    if at < 0  return false, s, "";
    return true, slice(s, 0, at), slice(s, at + 1, s.count - at - 1);
}

// The first occurrence, split the same way.
split_from_left :: (s: string, separator: string) -> (found: bool, left: string, right: string) {
    at := find_index_from_left(s, separator);
    if at < 0  return false, s, "";
    return true, slice(s, 0, at), slice(s, at + separator.count, s.count - at - separator.count);
}

split_from_left :: (s: string, separator: u8) -> (found: bool, left: string, right: string) {
    at := find_index_from_left(s, separator);
    if at < 0  return false, s, "";
    return true, slice(s, 0, at), slice(s, at + 1, s.count - at - 1);
}

// The other spelling of `begins_with`; both names are in use.
starts_with :: begins_with;

// Whether `substring` occurs in `s`, and what follows the first occurrence —
// which lets a caller both test and continue in one call:
//
//     found, rest := contains(line, "=");
//
contains :: (s: string, substring: string) -> (found: bool, remainder: string) {
    at := find_index_from_left(s, substring);
    if at < 0  return false, "";
    return true, slice(s, at + substring.count, s.count - at - substring.count);
}

contains :: (s: string, char: u8) -> (found: bool, remainder: string) {
    at := find_index_from_left(s, char);
    if at < 0  return false, "";
    return true, slice(s, at + 1, s.count - at - 1);
}
