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
