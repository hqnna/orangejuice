//
// String: comparing, searching, slicing and splitting text.
//
// Basic keeps only what it needs itself — copying, converting and the byte
// predicates. Everything a program reaches for when it is actually working
// with text is here, which is where the reference draws the line too.
//
// Two conventions run through the whole module. Nothing allocates unless it
// says so: a slice, a trim, a split's *pieces* and every path routine hand
// back views into the string they were given. And where something does
// allocate, the doc comment on it says which allocator — `split`'s array and
// `to_lower_copy`'s bytes come from temporary storage, `join`'s and
// `replace`'s from the context.
//

#import "Basic";

DEFAULT_DELIMITERS :: ";=()";
DEFAULT_SPACES     :: " \t\r\n";

PATH_SEPARATOR :: #char "/";

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

// The same semantics as C's `strcmp`: -1 when `a` sorts first, 1 when `b`
// does, 0 when they are the same string. A shared prefix is broken by the
// shorter one sorting first.
compare :: (a: string, b: string) -> int {
    for 0..min(a.count, b.count) - 1 {
        if a[it] > b[it]  return 1;
        if a[it] < b[it]  return -1;
    }
    if a.count > b.count  return 1;
    if a.count < b.count  return -1;
    return 0;
}

compare_nocase :: (a: string, b: string) -> int {
    for 0..min(a.count, b.count) - 1 {
        left := to_lower(a[it]);
        right := to_lower(b[it]);
        if left > right  return 1;
        if left < right  return -1;
    }
    if a.count > b.count  return 1;
    if a.count < b.count  return -1;
    return 0;
}

begins_with :: (s: string, prefix: string) -> bool {
    if prefix.count > s.count  return false;
    if !prefix.count           return true;
    return memcmp(s.data, prefix.data, prefix.count) == 0;
}

begins_with_nocase :: (s: string, prefix: string) -> bool {
    if prefix.count > s.count  return false;
    return equal_nocase(slice(s, 0, prefix.count), prefix);
}

ends_with :: (s: string, suffix: string) -> bool {
    if suffix.count > s.count  return false;
    if !suffix.count           return true;
    return memcmp(s.data + s.count - suffix.count, suffix.data, suffix.count) == 0;
}

ends_with_nocase :: (s: string, suffix: string) -> bool {
    if suffix.count > s.count  return false;
    return equal_nocase(slice(s, s.count - suffix.count, suffix.count), suffix);
}

// The other spelling of `begins_with`; both names are in use.
starts_with :: begins_with;

// ------------------------------------------------------------- searching ---

// Where `substring` starts, at or after `start_index`, or -1. An empty
// substring is never found — which is the reference's answer, and the one that
// keeps a scanning loop from standing still.
find_index_from_left :: (s: string, substring: string, start_index := 0) -> s64 {
    if !substring.count  return -1;
    for index: start_index..s.count - substring.count {
        if index < 0  continue;
        if memcmp(s.data + index, substring.data, substring.count) == 0  return index;
    }
    return -1;
}

find_index_from_left :: (s: string, byte: u8, start_index := 0) -> s64 {
    for index: start_index..s.count - 1 {
        if index < 0  continue;
        if s[index] == byte  return index;
    }
    return -1;
}

find_index_from_right :: (s: string, substring: string) -> s64 {
    if !substring.count           return -1;
    if substring.count > s.count  return -1;
    at := s.count - substring.count;
    while at >= 0 {
        if memcmp(s.data + at, substring.data, substring.count) == 0  return at;
        at -= 1;
    }
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

// Like the single-byte `find_index_from_left`, but any of `bytes` matches. The
// argument types are the same as the substring form, so it is worth being sure
// which one a call means.
find_index_of_any_from_left :: (s: string, bytes: string, start_index := 0) -> s64 {
    for index: start_index..s.count - 1 {
        if index < 0  continue;
        if is_any(s[index], bytes)  return index;
    }
    return -1;
}

find_index_of_any_from_right :: (s: string, bytes: string) -> s64 {
    at := s.count - 1;
    while at >= 0 {
        if is_any(s[at], bytes)  return at;
        at -= 1;
    }
    return -1;
}

index_of_string_nocase :: (s: string, substring: string) -> (index: int, found: bool) {
    if !substring.count           return -1, false;
    if substring.count > s.count  return -1, false;
    for index: 0..s.count - substring.count {
        if equal_nocase(slice(s, index, substring.count), substring)  return index, true;
    }
    return -1, false;
}

// Whether `substring` occurs in `s`, and what follows the first occurrence —
// which lets a caller both test and continue in one call:
//
//     found, rest := contains(line, "=");
//
contains :: (s: string, substring: string) -> (found: bool, remainder: string) {
    at := find_index_from_left(s, substring);
    if at < 0  return false, "";
    return true, string_from(s, at + substring.count);
}

contains :: (s: string, char: u8) -> (found: bool, remainder: string) {
    at := find_index_from_left(s, char);
    if at < 0  return false, "";
    return true, string_from(s, at + 1);
}

contains_nocase :: (s: string, substring: string) -> (found: bool, remainder: string) {
    at, found := index_of_string_nocase(s, substring);
    if !found  return false, "";
    return true, string_from(s, at + substring.count);
}

contains_any_character :: (s: string, chars: string) -> (found: bool, remainder: string) {
    at := find_index_of_any_from_left(s, chars);
    if at < 0  return false, "";
    return true, string_from(s, at + 1);
}

// The first of `substrings` to occur anywhere in `s` wins, and what follows it
// comes back. Each candidate is searched in turn, so the earliest *candidate*
// decides rather than the earliest position.
contains_any_string :: (s: string, substrings: .. string) -> (found: bool, remainder: string) {
    for substrings {
        found, rest := contains(s, it);
        if found  return true, rest;
    }
    return false, "";
}

is_any :: (c: u8, chars: string) -> bool {
    for 0..chars.count - 1  if chars[it] == c  return true;
    return false;
}

// -------------------------------------------------------------- reshaping ---

// A view of part of `s`, sharing its bytes rather than copying them. The range
// is clamped, so a slice past the end is empty rather than out of bounds.
slice :: inline (s: string, index: s64, count: s64) -> string {
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

trim_left :: (s: string, chars := DEFAULT_SPACES) -> string {
    result := s;
    while result.count && is_any(result[0], chars) {
        result.data += 1;
        result.count -= 1;
    }
    return result;
}

trim_right :: (s: string, chars := DEFAULT_SPACES) -> string {
    result := s;
    while result.count && is_any(result[result.count - 1], chars)  result.count -= 1;
    return result;
}

trim :: (s: string, chars := DEFAULT_SPACES) -> string {
    return trim_right(trim_left(s, chars), chars);
}

// Everything up to the first byte that is any of `bytes`, or all of `s` when
// none of them occurs.
stop_at_any :: (s: string, bytes: string) -> string {
    at := find_index_of_any_from_left(s, bytes);
    if at < 0  return s;
    return slice(s, 0, at);
}

eat_trailing_spaces :: (_s: string) -> string {
    return trim_right(_s, DEFAULT_SPACES);
}

// Everything from the first space or tab to the end. The *remainder*, not the
// word: what came before it is the caller subtracting one count from the other.
eat_until_space :: (_s: string) -> string {
    s := _s;
    while s {
        if s[0] == #char " "  break;
        if s[0] == 9          break;
        advance(*s, 1);
    }
    return s;
}

// `\r\n` becomes `\n`, in place. The string shortens, so this takes a pointer:
// the bytes it does not need are left where they were.
normalize_line_endings :: (s: *string) {
    read := 0;
    write := 0;
    while read < s.count {
        c := s.data[read];
        if c == #char "\r" && read + 1 < s.count && s.data[read + 1] == #char "\n" {
            read += 1;
            continue;
        }
        s.data[write] = c;
        write += 1;
        read += 1;
    }
    s.count = write;
}

// The same into a new buffer, which the caller owns.
normalize_line_endings :: (s: string) -> string #must {
    copy := copy_string(s);
    normalize_line_endings(*copy);
    return copy;
}

// Every byte of `s` that is any of `chars` becomes `replacement`, in place.
replace_chars :: (s: string, chars: string, replacement: u8) {
    for 0..s.count - 1 {
        if is_any(s[it], chars)  s[it] = replacement;
    }
}

// --------------------------------------------------------------- splitting ---

// Splits `s` at every occurrence of `separator`, keeping empty pieces, so that
// `split("a,,b", ",")` is three of them. The pieces are views into `s`; the
// array holding them comes from temporary storage.
split :: (s: string, separator: $T) -> (result: [] string) {
    pieces: [..] string;
    pieces.allocator = temp;

    #if T == string {
        width := separator.count;
        if !width {
            array_add(*pieces, s);
            return pieces;
        }
    } else {
        width := 1;
    }

    rest := s;
    while true {
        at := find_index_from_left(rest, separator);
        if at < 0  break;
        array_add(*pieces, slice(rest, 0, at));
        rest = string_from(rest, at + width);
    }
    array_add(*pieces, rest);
    return pieces;
}

// The two halves either side of the first occurrence, without the separator
// itself. On a miss the whole string is on the left: `(false, s, "")`.
split_from_left :: (s: string, byte: u8) -> (found: bool, left: string, right: string) {
    at := find_index_from_left(s, byte);
    if at < 0  return false, s, "";
    return true, slice(s, 0, at), string_from(s, at + 1);
}

split_from_left :: (s: string, separator: string) -> (found: bool, left: string, right: string) {
    at := find_index_from_left(s, separator);
    if at < 0  return false, s, "";
    return true, slice(s, 0, at), string_from(s, at + separator.count);
}

// The same from the other end. A miss puts the whole string on the *right*,
// `(false, "", s)`, which is the reference's answer and the mirror of the
// left-hand form.
split_from_right :: (s: string, byte: u8) -> (found: bool, left: string, right: string) {
    at := find_index_from_right(s, byte);
    if at < 0  return false, "", s;
    return true, slice(s, 0, at), string_from(s, at + 1);
}

split_from_right :: (s: string, separator: string) -> (found: bool, left: string, right: string) {
    at := find_index_from_right(s, separator);
    if at < 0  return false, "", s;
    return true, slice(s, 0, at), string_from(s, at + separator.count);
}

// Split at the first byte that is any of `bytes`. The argument types match the
// substring forms above, so it is worth being sure which one a call means.
split_from_left_by_any :: (s: string, bytes: string) -> (found: bool, left: string, right: string) {
    at := find_index_of_any_from_left(s, bytes);
    if at < 0  return false, s, "";
    return true, slice(s, 0, at), string_from(s, at + 1);
}

split_from_right_by_any :: (s: string, bytes: string) -> (found: bool, left: string, right: string) {
    at := find_index_of_any_from_right(s, bytes);
    if at < 0  return false, "", s;
    return true, slice(s, 0, at), string_from(s, at + 1);
}

// ---------------------------------------------------------------- building ---

// Each of `inputs` with `separator` between them; `before_first` and
// `after_last` put one on the ends as well, which is what building a path or a
// delimited record wants. The result comes from `context.allocator`.
join :: (inputs: .. string, separator := "", before_first := false, after_last := false) -> string {
    parts := inputs;

    separators := 0;
    if parts.count > 1  separators += parts.count - 1;
    if before_first     separators += 1;
    if after_last       separators += 1;

    total := separators * separator.count;
    for parts  total += it.count;

    result: string;
    if total <= 0  return result;
    result.data = cast(*u8) alloc(total);
    if !result.data  return result;
    result.count = total;

    at := 0;
    put :: (result: string, at: *s64, piece: string) #expand {
        if !piece.count  return;
        memcpy(result.data + at.*, piece.data, piece.count);
        at.* += piece.count;
    }

    if before_first  put(result, *at, separator);
    for parts {
        if it_index  put(result, *at, separator);
        put(result, *at, it);
    }
    if after_last  put(result, *at, separator);
    return result;
}

// A new string with every occurrence of `old` replaced by `new`, and how many
// there were. Scanning is left to right and does not consider overlaps; the
// result comes from `context.allocator`.
replace :: (s: string, old: string, new: string) -> (result: string, ocurrences: int) {
    if !old.count  return copy_string(s), 0;

    count := 0;
    at := 0;
    while at + old.count <= s.count {
        if memcmp(s.data + at, old.data, old.count) == 0 {
            count += 1;
            at += old.count;
        } else {
            at += 1;
        }
    }
    if !count  return copy_string(s), 0;

    total := s.count + count * (new.count - old.count);
    result: string;
    result.data = cast(*u8) alloc(total);
    if !result.data  return result, 0;
    result.count = total;

    read := 0;
    write := 0;
    while read < s.count {
        if read + old.count <= s.count && memcmp(s.data + read, old.data, old.count) == 0 {
            if new.count  memcpy(result.data + write, new.data, new.count);
            write += new.count;
            read += old.count;
            continue;
        }
        result.data[write] = s.data[read];
        write += 1;
        read += 1;
    }
    return result, count;
}

// ------------------------------------------------------------------- case ---

// A copy with every ASCII upper-case byte lowered. Anything outside A-Z is
// left alone, this being bytes rather than Unicode. The copy comes from
// `context.allocator` — the reference's doc comment says temporary storage,
// but its implementation does this, and the implementation is what programs
// were written against.
to_lower_copy :: (s: string) -> string {
    result := alloc_string(s.count);
    for 0..result.count - 1  result[it] = to_lower(s[it]);
    return result;
}

to_upper_copy :: (s: string) -> string {
    result := alloc_string(s.count);
    for 0..result.count - 1  result[it] = to_upper(s[it]);
    return result;
}

to_lower_copy_new :: (s: string) -> string #deprecated "Just call to_lower_copy() now, as it's been updated." {
    return to_lower_copy(s);
}

to_upper_copy_new :: (s: string) -> string #deprecated "Just call to_upper_copy() now, as it's been updated." {
    return to_upper_copy(s);
}

// Modifies `s` where it stands. A string literal lives in read-only memory, so
// calling this on one is a crash rather than a no-op.
to_lower_in_place :: ($$ s: string) {
    #if is_constant(s)  #assert false "You cannot call to_lower_in_place on constant strings, as they are stored in read-only memory. They must be copied if you want to change them.";
    for 0..s.count - 1  s[it] = to_lower(s[it]);
}

to_upper_in_place :: ($$ s: string) {
    #if is_constant(s)  #assert false "You cannot call to_upper_in_place on constant strings, as they are stored in read-only memory. They must be copied if you want to change them.";
    for 0..s.count - 1  s[it] = to_upper(s[it]);
}

// -------------------------------------------------------------- the paths ---

// A path taken apart: `path_decomp("/home/wizard/rezrov.foo")` is
// `("/home/wizard/", "rezrov", "foo", "rezrov.foo")`. Every piece is a view.
path_decomp :: (name: string) -> (path: string, basename: string, extension: string, basename_with_extension: string) {
    path := path_strip_filename(name);

    basename_with_extension := advance(name, path.count);
    extension := path_extension(basename_with_extension);
    basename := basename_with_extension;
    basename.count -= extension.count;

    // A trailing dot leaves an empty extension, and the dot itself still has
    // to come off the basename.
    if extension.count || (basename.count && basename[basename.count - 1] == #char ".") {
        basename.count -= 1;
    }
    return path, basename, extension, basename_with_extension;
}

decomp_filename :: path_decomp;

// "/home/wizard/rezrov.foo" -> "rezrov.foo".
path_filename :: (path: string) -> string {
    at := find_index_of_any_from_right(path, "\\/");
    if at < 0  return path;
    return string_from(path, at + 1);
}

// The filename without its extension: "rezrov.foo" -> "rezrov".
basename :: (s: string) -> string {
    _, name := path_decomp(s);
    return name;
}

path_basename :: basename;

// "/home/wizard/rezrov.foo" -> ("foo", true). A name with no dot after the
// last separator has no extension, and `found_extension` says so.
path_extension :: (path: string) -> (extension: string, found_extension: bool) {
    name := path_filename(path);
    at := find_index_from_right(name, #char ".");
    if at < 0  return "", false;
    return string_from(name, at + 1), true;
}

// "/home/wizard/rezrov.foo" -> "/home/wizard/rezrov".
path_strip_extension :: (path: string) -> string {
    name := path_filename(path);
    at := find_index_from_right(name, #char ".");
    if at < 0  return path;
    return slice(path, 0, path.count - name.count + at);
}

// "/home/wizard/rezrov.foo" -> "/home/wizard/". The separator stays on.
path_strip_filename :: (path: string) -> string {
    at := find_index_of_any_from_right(path, "\\/");
    if at < 0  return "";
    return slice(path, 0, at + 1);
}

// The path with `extension` on the end unless it is already there. The result
// comes from `context.allocator` when anything had to be added, and is the
// original view when nothing did.
path_ensure_extension :: (path: string, extension: string) -> string {
    found, has := path_extension(path);
    if has && equal_nocase(found, extension)  return path;
    return join(path, ".", extension);
}

// Every separator in `path` becomes `separator`, in place. Written for the
// backslash/forward-slash annoyance rather than for anything Linux needs.
path_overwrite_separators :: (path: string, separator: u8 = PATH_SEPARATOR) {
    replace_chars(path, "\\/", separator);
}

// A path that names its location outright rather than relative to anywhere.
is_absolute_path :: (path: string) -> bool {
    if !path.count  return false;
    return path[0] == #char "/" || path[0] == #char "\\";
}

// ------------------------------------------------------------- wildcards ---

// `*` matches any run, `?` any one byte, and `[abc]`/`[a-z]` a set. Nothing
// else is special — no alternation, no negation.
wildcard_match :: (s: string, pat: string) -> bool {
    return wildcard_from(s, 0, pat, 0);
}

// ---------------------------------------------------------------- parsing ---
//
// These pass `delimiters` and `spaces` down to the leaves, which makes for a
// confusing parser; the reference keeps them for the programs that already use
// them and recommends against reaching for them in new code.

parse_token :: (line: *string, delimiters := DEFAULT_DELIMITERS, spaces := DEFAULT_SPACES) -> (value: string, success: bool) {
    text := trim_left(line.*, spaces);
    if !text.count {
        line.* = text;
        return "", false;
    }

    // A delimiter standing on its own is a token.
    if is_any(text[0], delimiters) {
        line.* = string_from(text, 1);
        return slice(text, 0, 1), true;
    }

    at := 0;
    while at < text.count && !is_any(text[at], spaces) && !is_any(text[at], delimiters)  at += 1;
    line.* = string_from(text, at);
    return slice(text, 0, at), true;
}

// Consumes the next token only when it is `match`, leaving `line` alone
// otherwise.
match_token :: (line: *string, match: string, delimiters := DEFAULT_DELIMITERS, spaces := DEFAULT_SPACES) -> (value: string, success: bool) {
    rest := line.*;
    token, ok := parse_token(*rest, delimiters, spaces);
    if !ok || !equal(token, match)  return "", false;
    line.* = rest;
    return token, true;
}

match_token_nocase :: (line: *string, match: string, delimiters := DEFAULT_DELIMITERS, spaces := DEFAULT_SPACES) -> (value: string, success: bool) {
    rest := line.*;
    token, ok := parse_token(*rest, delimiters, spaces);
    if !ok || !equal_nocase(token, match)  return "", false;
    line.* = rest;
    return token, true;
}

parse_bool :: (line: *string, delimiters := DEFAULT_DELIMITERS, spaces := DEFAULT_SPACES) -> (value: bool, success: bool) {
    token, ok := parse_token(line, delimiters, spaces);
    if !ok  return false, false;
    if equal_nocase(token, "true")  || equal(token, "1")  return true, true;
    if equal_nocase(token, "false") || equal(token, "0")  return false, true;
    return false, false;
}

parse_enum :: (line: *string, $T: Type, delimiters := DEFAULT_DELIMITERS, spaces := DEFAULT_SPACES) -> (value: T, success: bool) {
    token, ok := parse_token(line, delimiters, spaces);
    if !ok  return cast,no_check(T) 0, false;

    info := cast(*Type_Info_Enum) type_info(T);
    for info.names {
        if equal(it, token)  return cast,no_check(T) info.values[it_index], true;
    }
    return cast,no_check(T) 0, false;
}

parse_float :: (line: *string, delimiters := DEFAULT_DELIMITERS, spaces := DEFAULT_SPACES) -> (value: float, success: bool) {
    token, ok := parse_token(line, delimiters, spaces);
    if !ok  return 0, false;
    value, parsed := string_to_float(token);
    return value, parsed;
}

parse_int :: (line: *string, $T: Type = int, spaces := " \t", delimiters := DEFAULT_DELIMITERS) -> (value: T, success: bool) {
    text := trim_left(line.*, spaces);
    value, ok, rest := string_to_int(text);
    if !ok  return 0, false;
    line.* = rest;
    return cast,no_check(T) value, true;
}

// C's own float parser, for a caller that wants exactly its rounding.
atof :: (s: *u8) -> float64 #foreign crt_string;

// ---------------------------------------------------------------- scanning ---

// Matches `text` against `format`, where `%f`, `%i`/`%d`, `%b` and `%s` stand
// for a float, an integer, `true`/`false` and a quoted string, and `%%` for a
// literal percent. Everything else in the format must appear in the text
// exactly. Both have to run out together for the match to succeed.
//
// The values come back as `Any`s in temporary storage, in the order they were
// read.
scan :: (format: string, text: string) -> (success: bool, values: [] Any) {
    result: [..] Any;
    result.allocator = temp;

    pattern := format;
    rest := text;

    while true {
        if !pattern.count || !rest.count  break;

        if pattern[0] != #char "%" {
            if rest[0] != pattern[0]  return false, result;
            advance(*pattern);
            advance(*rest);
        } else {
            if pattern.count == 1  return false, result;

            success := false;
            value: Any;
            if pattern[1] == {
                case #char "f"; success, value = scan_float(*rest);
                case #char "d"; #through;
                case #char "i"; success, value = scan_int(*rest);
                case #char "b"; success, value = scan_bool(*rest);
                case #char "s"; success, value = scan_quoted(*rest);
                case #char "%";
                    if rest[0] != #char "%"  return false, result;
                    advance(*rest);
                    success = true;
                case;
                    return false, result;
            }
            if !success  return false, result;
            // A `%%` matched a character rather than read a value.
            if value.type  array_add(*result, scan_keep(value));
            advance(*pattern, 2);
        }

        if !pattern.count && !rest.count  return true, result;
        if !pattern.count || !rest.count  return false, result;
    }
    return false, result;
}

// The same idea driven by the arguments' types rather than by letters: each
// `%` reads whatever the matching argument points at.
//
//     v: Vector3;
//     scan2(text, "(% % %)", *v.x, *v.y, *v.z);
//
// Every argument must be a pointer, since this writes through them.
scan2 :: (text: string, format: string, args: .. Any) -> bool {
    pattern := format;
    rest := text;
    next := 0;

    while true {
        if !pattern.count || !rest.count  break;

        if pattern[0] != #char "%" {
            if rest[0] != pattern[0]  return false;
            advance(*pattern);
            advance(*rest);
        } else if pattern.count > 1 && pattern[1] == #char "%" {
            if rest[0] != #char "%"  return false;
            advance(*pattern, 2);
            advance(*rest);
        } else {
            if next >= args.count             return false;
            if !args[next].type               return false;
            if args[next].type.type != .POINTER  return false;

            slot: Any;
            slot.type = (cast(*Type_Info_Pointer) args[next].type).pointer_to;
            slot.value_pointer = (cast(**void) args[next].value_pointer).*;

            if !Reflection.set_value_from_string(slot, *rest)  return false;
            advance(*pattern);
            next += 1;
        }

        if !pattern.count && !rest.count  return true;
        if !pattern.count || !rest.count  return false;
    }
    return false;
}

#scope_module

crt_string :: #library,system "libc";

// The tail of `s` from `index` on, which several things here want and which
// `slice` would need a count for.
string_from :: (s: string, index: s64) -> string {
    return slice(s, index, s.count - index);
}

// Wildcards, matched by walking both strings together. A `*` tries every
// length it could stand for, which is what makes the match backtrack.
wildcard_from :: (s: string, si: s64, pat: string, pi: s64) -> bool {
    at := si;
    p := pi;

    while p < pat.count {
        c := pat[p];

        if c == #char "*" {
            // A run of stars is one star.
            while p < pat.count && pat[p] == #char "*"  p += 1;
            if p == pat.count  return true;
            for take: at..s.count {
                if wildcard_from(s, take, pat, p)  return true;
            }
            return false;
        }

        if at >= s.count  return false;

        if c == #char "?" {
            at += 1;
            p += 1;
            continue;
        }

        if c == #char "[" {
            end := p + 1;
            while end < pat.count && pat[end] != #char "]"  end += 1;
            if end >= pat.count  return false;
            if !wildcard_set(slice(pat, p + 1, end - p - 1), s[at])  return false;
            at += 1;
            p = end + 1;
            continue;
        }

        if s[at] != c  return false;
        at += 1;
        p += 1;
    }
    return at == s.count;
}

// Whether `byte` is in a `[...]` class, which may hold `a-z` ranges.
wildcard_set :: (set: string, byte: u8) -> bool {
    at := 0;
    while at < set.count {
        if at + 2 < set.count && set[at + 1] == #char "-" {
            if byte >= set[at] && byte <= set[at + 2]  return true;
            at += 3;
            continue;
        }
        if set[at] == byte  return true;
        at += 1;
    }
    return false;
}

// -------------------------------------------------------- scanning's parts ---
//
// Each reads one value off the front of `text`, consuming what it read. The
// value comes back as an `Any` pointing at a local, so `scan` copies it into
// temporary storage before keeping it.

scan_float :: (text: *string) -> (success: bool, value: Any) {
    result, ok, rest := string_to_float(text.*);
    if !ok  return false, Any.{};
    text.* = rest;
    return true, result;
}

scan_int :: (text: *string) -> (success: bool, value: Any) {
    result, ok, rest := string_to_int(text.*);
    if !ok  return false, Any.{};
    text.* = rest;
    return true, result;
}

scan_bool :: (text: *string) -> (success: bool, value: Any) {
    if begins_with(text.*, "true") {
        advance(text, 4);
        yes := true;
        return true, yes;
    }
    if begins_with(text.*, "false") {
        advance(text, 5);
        no := false;
        return true, no;
    }
    return false, Any.{};
}

// A `"`-delimited run. Escape sequences are not handled, which is what the
// reference does too.
scan_quoted :: (text: *string) -> (success: bool, value: Any) {
    if !text.count || text.*[0] != #char "\""  return false, Any.{};
    advance(text, 1);

    at := 0;
    while at < text.count {
        if text.*[at] == #char "\"" {
            result := slice(text.*, 0, at);
            advance(text, at + 1);
            return true, result;
        }
        at += 1;
    }
    return false, Any.{};
}

// The value copied into temporary storage, since what `scan_*` handed back
// points at a local of theirs.
scan_keep :: (value: Any) -> Any {
    if !value.type || !value.value_pointer  return value;
    kept: Any;
    kept.type = value.type;
    kept.value_pointer = alloc(value.type.runtime_size, temp);
    if kept.value_pointer  memcpy(kept.value_pointer, value.value_pointer, value.type.runtime_size);
    return kept;
}

Reflection :: #import "Reflection";
