//
// UTF-8, and the code points it encodes.
//
// A Jai `string` is bytes, not characters (**L§3.4**). Anything that has to
// count characters, walk backwards, or split without cutting a character in
// half needs this.
//

#import "Basic";

// What a decode produced. `continuation` and `count` say how far to advance,
// which is 1 for invalid input so that a walk always makes progress.
Decoded :: struct {
    code_point: u32;
    count:      s64;
    valid:      bool;
}

// The value used for anything that does not decode: U+FFFD REPLACEMENT
// CHARACTER, which is what every other decoder substitutes.
REPLACEMENT :: 0xFFFD;

MAX_CODE_POINT :: 0x10FFFF;

// Decodes the character starting at `at`.
decode :: (s: string, at: s64) -> Decoded {
    result: Decoded;
    result.count = 1;
    result.code_point = REPLACEMENT;

    if at < 0 || at >= s.count  return result;

    first := s[at];

    // One byte: plain ASCII.
    if first < 0x80 {
        result.code_point = first;
        result.valid = true;
        return result;
    }

    // A continuation byte cannot start a character.
    if first < 0xC0  return result;

    wanted: s64;
    value: u32;
    if first < 0xE0 {
        wanted = 2;
        value = cast(u32) (first & 0x1F);
    } else if first < 0xF0 {
        wanted = 3;
        value = cast(u32) (first & 0x0F);
    } else if first < 0xF8 {
        wanted = 4;
        value = cast(u32) (first & 0x07);
    } else {
        return result;
    }

    if at + wanted > s.count  return result;

    for 1..wanted - 1 {
        byte := s[at + it];
        // Every following byte must be 10xxxxxx.
        if (byte & 0xC0) != 0x80  return result;
        value = (value << 6) | cast(u32) (byte & 0x3F);
    }

    // Reject what the standard calls overlong: a value encoded in more bytes
    // than it needs, which is how a naive decoder is tricked into seeing a
    // different string than a strict one does.
    shortest := 1;
    if value >= 0x80     shortest = 2;
    if value >= 0x800    shortest = 3;
    if value >= 0x10000  shortest = 4;
    if shortest != wanted  return result;

    // Surrogates are not characters; they only exist inside UTF-16.
    if value >= 0xD800 && value <= 0xDFFF  return result;
    if value > MAX_CODE_POINT  return result;

    result.code_point = value;
    result.count = wanted;
    result.valid = true;
    return result;
}

// How many bytes `code_point` takes; 0 if it is not encodable.
encoded_length :: (code_point: u32) -> s64 {
    if code_point < 0x80      return 1;
    if code_point < 0x800     return 2;
    if code_point >= 0xD800 && code_point <= 0xDFFF  return 0;
    if code_point < 0x10000   return 3;
    if code_point <= MAX_CODE_POINT  return 4;
    return 0;
}

// Writes into `buffer`, which must have room for four bytes, and returns how
// many it used. Zero means the code point cannot be encoded.
encode :: (buffer: *u8, code_point: u32) -> s64 {
    length := encoded_length(code_point);
    if length == {
        case 1;
            buffer[0] = cast(u8) code_point;
        case 2;
            buffer[0] = cast(u8) (0xC0 | (code_point >> 6));
            buffer[1] = cast(u8) (0x80 | (code_point & 0x3F));
        case 3;
            buffer[0] = cast(u8) (0xE0 | (code_point >> 12));
            buffer[1] = cast(u8) (0x80 | ((code_point >> 6) & 0x3F));
            buffer[2] = cast(u8) (0x80 | (code_point & 0x3F));
        case 4;
            buffer[0] = cast(u8) (0xF0 | (code_point >> 18));
            buffer[1] = cast(u8) (0x80 | ((code_point >> 12) & 0x3F));
            buffer[2] = cast(u8) (0x80 | ((code_point >> 6) & 0x3F));
            buffer[3] = cast(u8) (0x80 | (code_point & 0x3F));
    }
    return length;
}

// How many characters, as opposed to how many bytes. Invalid bytes count as
// one each.
character_count :: (s: string) -> s64 {
    total := 0;
    at := 0;
    while at < s.count {
        decoded := decode(s, at);
        at += decoded.count;
        total += 1;
    }
    return total;
}

// Whether every character decodes.
is_valid_utf8 :: (s: string) -> bool {
    at := 0;
    while at < s.count {
        decoded := decode(s, at);
        if !decoded.valid  return false;
        at += decoded.count;
    }
    return true;
}

// Where the character containing byte `at` begins — for a caller that has an
// arbitrary offset and must not cut a character in half.
character_start :: (s: string, at: s64) -> s64 {
    index := at;
    if index >= s.count  index = s.count - 1;
    // Walk back over continuation bytes, but never more than three: past that
    // the input is broken and stepping further would not help.
    steps := 0;
    while index > 0 && (s[index] & 0xC0) == 0x80 && steps < 3 {
        index -= 1;
        steps += 1;
    }
    return max(index, 0);
}

// The code points, as an array the caller owns.
to_code_points :: (s: string, allocator := context.allocator) -> [] u32 {
    points: [..] u32;
    points.allocator = allocator;
    at := 0;
    while at < s.count {
        decoded := decode(s, at);
        array_add(*points, decoded.code_point);
        at += decoded.count;
    }
    return points;
}

// The string those code points encode.
from_code_points :: (points: [] u32, allocator := context.allocator) -> string {
    builder: String_Builder;
    builder.allocator = temp;
    buffer: [4] u8;
    for points {
        used := encode(buffer.data, it);
        if !used  used = encode(buffer.data, REPLACEMENT);
        piece: string;
        piece.data = buffer.data;
        piece.count = used;
        append(*builder, piece);
    }
    return builder_to_string(*builder,, allocator = allocator);
}

// `for characters(s)` walks code points where `for s` walks bytes.
//
// A wrapper rather than a `for_expansion` on `string` itself: iterating a
// string is already defined to give its bytes (**L§3.4**), and quietly
// changing what that means for anyone who imports this module would be worse
// than asking for the one extra word.
Characters :: struct {
    text: string;
}

characters :: (s: string) -> Characters {
    return .{s};
}

// `it` is the code point; `it_index` is the byte it started at, which is what
// a caller slicing the original string needs.
for_expansion :: (chars: *Characters, body: Code, flags: For_Flags) #expand {
    at := 0;
    while at < chars.text.count {
        decoded := decode(chars.text, at);
        `it_index := at;
        `it := decoded.code_point;
        at += decoded.count;
        #insert body;
    }
}

// ---------------------------------------------------- the ConvertUTF names ---
//
// The names and tables from Unicode's own `ConvertUTF.c`, which a good deal of
// code was written against. They say the same things the procedures above do,
// in the shapes that reference code expects.

Unicode_Result :: enum {
    CONVERSION_OK;
    SOURCE_EXHAUSTED;
    SOURCE_ILLEGAL;
}

UNI_REPLACEMENT_CHAR :: 0x0000FFFD;
UNI_MAX_BMP          :: 0x0000FFFF;
UNI_MAX_UTF16        :: 0x0010FFFF;
UNI_MAX_UTF32        :: 0x7FFFFFFF;

// How many bytes follow a given leading byte. A continuation byte's entry is
// zero, so a resynchronising scan reads it as a one-byte sequence and moves on.
trailingBytesForUTF8 :: #run -> [256] u8 {
    table: [256] u8;
    for 0..255 {
        if      it < 0xc0  table[it] = 0;
        else if it < 0xe0  table[it] = 1;
        else if it < 0xf0  table[it] = 2;
        else if it < 0xf8  table[it] = 3;
        else if it < 0xfc  table[it] = 4;
        else               table[it] = 5;
    }
    return table;
};

// What to subtract after folding a sequence's bytes together: the accumulated
// leading-byte and continuation-byte markers.
offsetsFromUTF8 :: u32.[0x0000_0000, 0x0000_3080, 0x000E_2080, 0x03C8_2080, 0xFA08_2080, 0x8208_2080];

// The leading byte's marker bits, by sequence length.
firstByteMark :: u8.[0x00, 0x00, 0xC0, 0xE0, 0xF0, 0xF8, 0xFC];

// What is left of a leading byte once its marker is masked off.
utf8_inital_byte_mask :: u8.[0x7F, 0x1F, 0x0F, 0x07, 0x03, 0x01];

// One character from `s`, how many bytes it took, and whether it was valid.
// `strict` rejects the surrogate range and anything past the last code point.
character_utf8_to_utf32 :: (s: *u8, source_length: s64, $strict := false) -> (u32, s64, Unicode_Result) {
    if source_length <= 0  return UNI_REPLACEMENT_CHAR, 0, .SOURCE_EXHAUSTED;

    extra := trailingBytesForUTF8[s[0]];
    if cast(s64) extra + 1 > source_length  return UNI_REPLACEMENT_CHAR, source_length, .SOURCE_EXHAUSTED;

    code: u32 = 0;
    for 0..extra {
        code += s[it];
        if it != extra  code <<= 6;
    }
    code -= offsetsFromUTF8[extra];

    length := cast(s64) extra + 1;
    #if strict {
        // A surrogate half is not a character, and nothing lives past
        // UNI_MAX_UTF16.
        if code >= 0xD800 && code <= 0xDFFF  return UNI_REPLACEMENT_CHAR, length, .SOURCE_ILLEGAL;
        if code > UNI_MAX_UTF16              return UNI_REPLACEMENT_CHAR, length, .SOURCE_ILLEGAL;
    }
    return code, length, .CONVERSION_OK;
}

// Writes a character into storage the caller already has, which must hold four
// bytes. The string's count is set to how many were used.
character_utf32_to_utf8 :: (ch: u32, preallocated_string: *string) {
    written := encode(preallocated_string.data, ch);
    preallocated_string.count = written;
}

character_utf32_to_utf8 :: (ch: u32) -> string {
    result := alloc_string(4);
    result.count = encode(result.data, ch);
    return result;
}

// The start of the character after the one at `s`.
unicode_next_character :: (s: *u8) -> *u8 {
    return s + trailingBytesForUTF8[s[0]] + 1;
}

// One character off the front of `s`, consuming it.
utf8_next_character :: (s: *string) -> (code: u32, success: Unicode_Result) {
    code, bytes, success := character_utf8_to_utf32(s.data, s.count);
    s.data += bytes;
    s.count -= bytes;
    return code, success;
}

// `for utf8_iter(*s)` walks the characters, consuming the string as it goes.
utf8_iter :: (s: *string, body: Code, flags: For_Flags) #expand {
    while s.count {
        `it, ok := utf8_next_character(s);
        if ok != .CONVERSION_OK  break;
        `it_index := 0;
        #insert body;
    }
}
