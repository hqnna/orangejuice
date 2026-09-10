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

// The filename with its extension taken off.
path_strip_extension :: (path: string) -> string {
    name := path_filename(path);
    at := find_index_from_right(name, #char ".");
    if at <= 0  return name;
    return slice(name, 0, at);
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

