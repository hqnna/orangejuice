//
// Base64 (RFC 4648).
//
// Three bytes become four characters of six bits each; input that does not
// divide by three is padded with `=`. The URL-safe alphabet swaps `+/` for
// `-_`, which is what a filename or a query parameter needs.
//

#import "Basic";

STANDARD_ALPHABET :: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
URL_ALPHABET      :: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

base64_encode :: (bytes: [] u8, allocator := context.allocator, url_safe := false, pad := true) -> string {
    result: string;
    if !bytes.count  return result;

    alphabet := ifx url_safe then URL_ALPHABET else STANDARD_ALPHABET;

    groups := (bytes.count + 2) / 3;
    size := groups * 4;
    if !pad {
        // Without padding the last group is only as long as it needs to be.
        leftover := bytes.count % 3;
        if leftover == 1  size -= 2;
        if leftover == 2  size -= 1;
    }

    result.data = cast(*u8) alloc(size, allocator);
    if !result.data  return result;
    result.count = size;

    at := 0;
    index := 0;
    while index < bytes.count {
        // Gather up to three bytes into one 24-bit accumulator.
        have := min(3, bytes.count - index);
        accumulator: u32 = 0;
        for 0..2 {
            accumulator <<= 8;
            if it < have  accumulator |= bytes[index + it];
        }

        // Four six-bit digits, of which only `have + 1` carry data.
        for 0..3 {
            if it <= have {
                digit := (accumulator >> cast(u32) (18 - 6 * it)) & 0x3f;
                result[at] = alphabet[digit];
            } else {
                if !pad  break;
                result[at] = #char "=";
            }
            at += 1;
            if at >= size  break;
        }
        index += 3;
    }
    return result;
}

base64_encode :: (s: string, allocator := context.allocator, url_safe := false, pad := true) -> string {
    bytes: [] u8;
    bytes.data = s.data;
    bytes.count = s.count;
    return base64_encode(bytes, allocator, url_safe, pad);
}

// Accepts both alphabets and tolerates missing padding; `ok` is false when a
// character belongs to neither.
base64_decode :: (text: string, allocator := context.allocator) -> (bytes: [] u8, ok: bool) {
    result: [] u8;
    if !text.count  return result, true;

    // At most three bytes per four characters.
    capacity := (text.count / 4 + 1) * 3;
    storage := cast(*u8) alloc(capacity, allocator);
    if !storage  return result, false;

    accumulator: u32 = 0;
    bits := 0;
    at := 0;

    for 0..text.count - 1 {
        c := text[it];
        if c == #char "=" || is_space(c)  continue;

        value := base64_value(c);
        if value < 0 {
            free(storage, allocator);
            return result, false;
        }

        accumulator = (accumulator << 6) | cast(u32) value;
        bits += 6;
        // Every time eight bits have accumulated, a byte falls out.
        if bits >= 8 {
            bits -= 8;
            storage[at] = cast(u8) ((accumulator >> cast(u32) bits) & 0xff);
            at += 1;
        }
    }

    result.data = storage;
    result.count = at;
    return result, true;
}

#scope_module

// The six-bit value of a character in either alphabet, or -1.
base64_value :: (c: u8) -> s64 {
    if c >= #char "A" && c <= #char "Z"  return cast(s64) (c - #char "A");
    if c >= #char "a" && c <= #char "z"  return cast(s64) (c - #char "a") + 26;
    if c >= #char "0" && c <= #char "9"  return cast(s64) (c - #char "0") + 52;
    if c == #char "+" || c == #char "-"  return 62;
    if c == #char "/" || c == #char "_"  return 63;
    return -1;
}
