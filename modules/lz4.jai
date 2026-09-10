//
// LZ4 block format: compression and decompression.
//
// LZ4 trades ratio for speed — it is not trying to beat gzip on size, it is
// trying to decompress at memory bandwidth. The format is a sequence of
// blocks, each a token byte, some literal bytes, and a back-reference into
// what has already been decoded.
//
// Token: high nibble is the literal count, low nibble the match length minus
// four. A nibble of 15 means "more follows", as bytes of 255 terminated by
// anything less.
//
// Written from the published block-format specification.
//

#import "Basic";

MIN_MATCH        :: 4;
// The last five bytes are always literals and the last match must end twelve
// bytes before the end. Those are the format's rules and a decoder relies on
// them, so the compressor has to honour them.
LAST_LITERALS    :: 5;
MATCH_END_MARGIN :: 12;

MAX_OFFSET :: 65535;

// The largest a block can grow to. Incompressible input costs a token per 255
// literals, so the worst case is slightly larger than the input.
compress_bound :: (size: s64) -> s64 {
    return size + size / 255 + 16;
}

// Compresses into `output`, returning how many bytes it used. `ok` is false if
// `output` was too small.
lz4_compress :: (input: [] u8, output: [] u8) -> (size: s64, ok: bool) {
    if !input.count  return 0, true;

    // A hash table from four-byte sequences to where they were last seen.
    // Missing a match costs ratio, never correctness, so the table is small
    // and never checked for collisions beyond comparing the bytes.
    HASH_BITS :: 12;
    HASH_SIZE :: 1 << HASH_BITS;
    table: [HASH_SIZE] s32;
    for * table  it.* = -1;

    out := 0;
    at := 0;
    literal_start := 0;

    // Nothing may match within the final margin.
    limit := input.count - MATCH_END_MARGIN;

    while at < limit {
        sequence := read_u32(input.data, at);
        slot := hash4(sequence, HASH_BITS);
        candidate := table[slot];
        table[slot] = cast(s32) at;

        // A candidate is only usable if it is in range and really matches.
        if candidate < 0
        || at - candidate > MAX_OFFSET
        || read_u32(input.data, candidate) != sequence {
            at += 1;
            continue;
        }

        // Extend the match as far as it goes, stopping short of the margin.
        length := MIN_MATCH;
        while at + length < limit && input[candidate + length] == input[at + length] {
            length += 1;
        }

        literal_count := at - literal_start;
        offset := at - candidate;

        if !emit_sequence(output, *out, input, literal_start, literal_count, offset, length) {
            return 0, false;
        }

        at += length;
        literal_start = at;
    }

    // Whatever is left is literals, with no match after them.
    if !emit_last_literals(output, *out, input, literal_start)  return 0, false;
    return out, true;
}

// Decompresses into `output`. `ok` is false for malformed input or an output
// buffer that is too small — a decoder must never write past its buffer
// because the input said to.
lz4_decompress :: (input: [] u8, output: [] u8) -> (size: s64, ok: bool) {
    at := 0;
    out := 0;

    while at < input.count {
        token := input[at];
        at += 1;

        // Literals.
        literal_count := cast(s64) (token >> 4);
        if literal_count == 15 {
            while true {
                if at >= input.count  return 0, false;
                more := input[at];
                at += 1;
                literal_count += cast(s64) more;
                if more != 255  break;
            }
        }

        if at + literal_count > input.count       return 0, false;
        if out + literal_count > output.count     return 0, false;
        if literal_count {
            memcpy(output.data + out, input.data + at, literal_count);
            at += literal_count;
            out += literal_count;
        }

        // The last sequence has no match after its literals.
        if at >= input.count  break;

        if at + 2 > input.count  return 0, false;
        offset := cast(s64) input[at] | (cast(s64) input[at + 1] << 8);
        at += 2;
        if offset == 0 || offset > out  return 0, false;

        match_length := cast(s64) (token & 0xf);
        if match_length == 15 {
            while true {
                if at >= input.count  return 0, false;
                more := input[at];
                at += 1;
                match_length += cast(s64) more;
                if more != 255  break;
            }
        }
        match_length += MIN_MATCH;

        if out + match_length > output.count  return 0, false;

        // Byte at a time deliberately: the ranges overlap when the offset is
        // shorter than the length, and that overlap is how LZ4 encodes runs.
        from := out - offset;
        for 0..match_length - 1 {
            output[out] = output[from];
            out += 1;
            from += 1;
        }
    }
    return out, true;
}

#scope_module

hash4 :: (sequence: u32, bits: s64) -> s64 {
    // Knuth's multiplicative hash, keeping the high bits.
    return cast(s64) ((sequence * 2654435761) >> cast(u32) (32 - bits));
}

read_u32 :: (bytes: *u8, at: s64) -> u32 {
    return cast(u32) bytes[at]
         | (cast(u32) bytes[at + 1] << 8)
         | (cast(u32) bytes[at + 2] << 16)
         | (cast(u32) bytes[at + 3] << 24);
}

// A length that does not fit its nibble is written as 255s and a remainder.
write_length :: (output: [] u8, out: *s64, value: s64) -> bool {
    remaining := value;
    while remaining >= 255 {
        if out.* >= output.count  return false;
        output[out.*] = 255;
        out.* += 1;
        remaining -= 255;
    }
    if out.* >= output.count  return false;
    output[out.*] = cast(u8) remaining;
    out.* += 1;
    return true;
}

emit_sequence :: (output: [] u8, out: *s64, input: [] u8, literal_start: s64, literal_count: s64, offset: s64, match_length: s64) -> bool {
    extra := match_length - MIN_MATCH;

    token: u8;
    token |= cast(u8) (ifx literal_count >= 15 then 15 else literal_count) << 4;
    token |= cast(u8) (ifx extra >= 15 then 15 else extra);

    if out.* >= output.count  return false;
    output[out.*] = token;
    out.* += 1;

    if literal_count >= 15 {
        if !write_length(output, out, literal_count - 15)  return false;
    }
    if literal_count {
        if out.* + literal_count > output.count  return false;
        memcpy(output.data + out.*, input.data + literal_start, literal_count);
        out.* += literal_count;
    }

    if out.* + 2 > output.count  return false;
    output[out.*]     = cast(u8) (offset & 0xff);
    output[out.* + 1] = cast(u8) ((offset >> 8) & 0xff);
    out.* += 2;

    if extra >= 15 {
        if !write_length(output, out, extra - 15)  return false;
    }
    return true;
}

emit_last_literals :: (output: [] u8, out: *s64, input: [] u8, literal_start: s64) -> bool {
    literal_count := input.count - literal_start;

    token: u8 = cast(u8) (ifx literal_count >= 15 then 15 else literal_count) << 4;
    if out.* >= output.count  return false;
    output[out.*] = token;
    out.* += 1;

    if literal_count >= 15 {
        if !write_length(output, out, literal_count - 15)  return false;
    }
    if literal_count {
        if out.* + literal_count > output.count  return false;
        memcpy(output.data + out.*, input.data + literal_start, literal_count);
        out.* += literal_count;
    }
    return true;
}
