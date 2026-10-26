//
// The string operations Basic itself needs, and no more.
//
// A `string` is a count and a pointer at bytes it does not own (**L§3.4**), so
// everything here that produces one says which allocator it came from.
//
// Comparing, searching, slicing and splitting are the `String` module's, not
// this one's — the reference draws the line in the same place, and a program
// that imports both would otherwise find two of each.
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
copy_temporary_string :: (s: string) -> string {
    return copy_string(s, temp);
}

// Storage for a string of `count` bytes, uninitialised.
alloc_string :: (count: s64, allocator := context.allocator) -> string {
    result: string;
    if count <= 0  return result;
    result.data = cast(*u8) alloc(count, allocator);
    if result.data  result.count = count;
    return result;
}

talloc_string :: (count: s64) -> string {
    return alloc_string(count, temp);
}

// A copy with a zero after the last byte, for handing to C.
to_c_string :: (s: string, allocator := context.allocator) -> *u8 {
    memory := cast(*u8) alloc(s.count + 1, allocator);
    if !memory  return null;
    if s.count  memcpy(memory, s.data, s.count);
    memory[s.count] = 0;
    return memory;
}

// The same in temporary storage, which is what most calls into C want: the
// callee is done with the bytes before the next reset.
temp_c_string :: (s: string) -> *u8 {
    return to_c_string(s, temp);
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

// ------------------------------------------------------------- advancing ---

// Moves a string forward over bytes already dealt with. The in-place form is
// what a parser walking its input wants.
advance :: inline (s: *string, amount: s64 = 1) {
    step := amount;
    if step > s.count  step = s.count;
    if step < 0        step = 0;
    s.data  += step;
    s.count -= step;
}

advance :: inline (s: string, amount: s64 = 1) -> string #must {
    result := s;
    advance(*result, amount);
    return result;
}

// --------------------------------------------------------------- the bytes ---

is_space :: (byte: u8) -> bool {
    return byte == #char " " || byte == #char "\t" || byte == #char "\n" || byte == #char "\r";
}

is_digit :: (byte: u8) -> bool {
    return byte >= #char "0" && byte <= #char "9";
}

is_alpha :: (byte: u8) -> bool {
    return (byte >= #char "a" && byte <= #char "z") || (byte >= #char "A" && byte <= #char "Z");
}

// An identifier's byte, so `_` counts: jai-utils continues an identifier with
// `is_alnum` alone and starts one with `is_alpha(c) || c == "_"`, and
// toml-jai's bare keys, which allow `_`, are checked with it.
is_alnum :: (byte: u8) -> bool {
    return is_alpha(byte) || is_digit(byte) || byte == #char "_";
}

// A byte that shows as itself on a terminal, space included — which is what
// decides whether a dump prints the character or its number.
is_print :: (byte: u8) -> bool {
    return byte >= #char " " && byte < 127;
}

to_lower :: (c: u8) -> u8 {
    if c >= #char "A" && c <= #char "Z"  return c + 32;
    return c;
}

to_upper :: (c: u8) -> u8 {
    if c >= #char "a" && c <= #char "z"  return c - 32;
    return c;
}

// ------------------------------------------------------------- to numbers ---

// The leading integer, and whether there was one. The rest of the string comes
// back too, so a caller parsing several in a row does not have to count.
//
// `base` may be anything from 2 to 16; the digits above 9 are `a`-`f` in
// either case. `T` says which integer type comes back, so a caller wanting a
// `u16` writes `string_to_int(s, T = u16)` rather than casting afterwards.
string_to_int :: (s: string, base := 10, $T := int) -> (value: T, success: bool, remainder: string) {
    if base < 2 || base > 16  return 0, false, s;

    // Leading white space is skipped, the way the reference's parser does.
    at := skip_spaces(s);
    negative := false;
    if at < s.count && (s[at] == #char "-" || s[at] == #char "+") {
        negative = s[at] == #char "-";
        at += 1;
    }

    start := at;
    value: s64 = 0;
    while at < s.count {
        digit := digit_value(s[at]);
        if digit < 0 || digit >= base  break;
        value = value * base + digit;
        at += 1;
    }
    if at == start  return 0, false, s;
    if negative  value = -value;
    return cast,no_check(T) value, true, string_from(s, at);
}

// The same, as whichever integer type the caller is filling, with the range
// checked — which is what an options parser reading text into a `u8` needs.
to_integer :: (s: string, $T := int, base := 10) -> (value: T, success: bool, remainder: string) {
    wide, ok, rest := string_to_int(s, base);
    if !ok  return 0, false, s;

    narrow := cast,no_check(T) wide;
    // A value that does not survive the round trip did not fit.
    if cast(s64) narrow != wide  return 0, false, s;
    return narrow, true, rest;
}

string_to_float :: (s: string) -> (result: float32, success: bool, remainder: string) {
    wide, ok, rest := string_to_float64(s);
    return cast(float32) wide, ok, rest;
}

// The leading float, and what follows it. Handles a sign, a decimal point and
// an exponent; anything else stops the parse where it stands. The digits are
// found here and converted by the C library, which rounds correctly.
string_to_float64 :: (s: string) -> (result: float64, success: bool, remainder: string) {
    at := skip_spaces(s);
    first := at;
    if at < s.count && (s[at] == #char "-" || s[at] == #char "+")  at += 1;

    start := at;
    while at < s.count && is_digit(s[at])  at += 1;
    digits := at - start;
    if at < s.count && s[at] == #char "." {
        at += 1;
        while at < s.count && is_digit(s[at]) { at += 1; digits += 1; }
    }

    // Nothing but a sign and a point is not a number.
    if !digits  return 0, false, s;

    if at < s.count && (s[at] == #char "e" || s[at] == #char "E") {
        after := at + 1;
        if after < s.count && (s[after] == #char "-" || s[after] == #char "+")  after += 1;
        if after < s.count && is_digit(s[after]) {
            while after < s.count && is_digit(s[after])  after += 1;
            at = after;
        }
    }

    return float_from_text(string_from(s, first), at - first), true, string_from(s, at);
}

// The newer spelling, with the success first. It reads the same numbers
// `string_to_float64` does, and also `inf`, `infinity` and `nan` in either
// case with an optional sign.
string_to_float64_new :: (s: string) -> (success: bool, result: float64, remainder: string) {
    value, ok, rest := string_to_float64(s);
    if ok  return true, value, rest;

    at := 0;
    negative := false;
    if at < s.count && (s[at] == #char "-" || s[at] == #char "+") {
        negative = s[at] == #char "-";
        at += 1;
    }
    word := string_from(s, at);
    for name: string.["infinity", "inf", "nan"] {
        if word.count < name.count  continue;
        matches := true;
        for i: 0..name.count - 1 {
            c := word[i];
            if c >= #char "A" && c <= #char "Z"  c += #char "a" - #char "A";
            if c != name[i] { matches = false; break; }
        }
        if !matches  continue;
        result: float64 = ifx name[0] == #char "n" then 0h7FF8000000000000 else 0h7FF0000000000000;
        if negative  result = -result;
        return true, result, string_from(word, name.count);
    }
    return false, 0, s;
}

string_to_float_new :: (s: string) -> (success: bool, result: float32, remainder: string) {
    ok, value, rest := string_to_float64_new(s);
    return ok, cast(float32) value, rest;
}

#scope_module

skip_spaces :: (s: string) -> s64 {
    at := 0;
    while at < s.count && (s[at] == #char " " || s[at] == #char "\t" || s[at] == #char "\n" || s[at] == #char "\r")  at += 1;
    return at;
}

float_from_text :: (s: string, count: s64) -> float64 {
    buffer: [128] u8 = ---;
    text := buffer.data;
    if count >= buffer.count  text = alloc(count + 1,, temp);
    memcpy(text, s.data, count);
    text[count] = 0;
    return strtod(text, null);
}

// -1 for a byte that is not a digit in any base up to 16.
digit_value :: (byte: u8) -> s64 {
    if byte >= #char "0" && byte <= #char "9"  return byte - #char "0";
    if byte >= #char "a" && byte <= #char "f"  return byte - #char "a" + 10;
    if byte >= #char "A" && byte <= #char "F"  return byte - #char "A" + 10;
    return -1;
}

// What is left of `s` from `index` on. `String.slice` is the general form;
// this is the one the parsers above need, and having it here is what keeps
// Basic from depending on String.
string_from :: (s: string, index: s64) -> string {
    result: string;
    at := index;
    if at < 0        at = 0;
    if at > s.count  at = s.count;
    result.data = s.data + at;
    result.count = s.count - at;
    return result;
}

#scope_export
