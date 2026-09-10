//
// MD5 (RFC 1321).
//
// MD5 is broken for anything security-related: collisions can be produced at
// will, and have been since 2004. It is here because file formats and
// protocols still carry MD5 fields, and because a content checksum over data
// nobody is attacking is a legitimate use. Never use it to verify that
// something has not been tampered with.
//

#import "Basic";

MD5_State :: struct {
    a: u32 = 0x6745_2301;
    b: u32 = 0xefcd_ab89;
    c: u32 = 0x98ba_dcfe;
    d: u32 = 0x1032_5476;

    // Bytes waiting for a full 64-byte block.
    buffer:   [64] u8;
    buffered: s64;
    // The total, which the padding encodes as a bit count.
    total: u64;
}

md5 :: (bytes: *u8, count: s64) -> [16] u8 {
    state: MD5_State;
    md5_update(*state, bytes, count);
    return md5_final(*state);
}

md5 :: (s: string) -> [16] u8 {
    return md5(s.data, s.count);
}

md5_update :: (state: *MD5_State, bytes: *u8, count: s64) {
    state.total += cast(u64) count;

    at := 0;
    // Fill whatever is left of a partial block first.
    if state.buffered {
        take := min(64 - state.buffered, count);
        memcpy(state.buffer.data + state.buffered, bytes, take);
        state.buffered += take;
        at += take;
        if state.buffered == 64 {
            md5_block(state, state.buffer.data);
            state.buffered = 0;
        }
    }

    while count - at >= 64 {
        md5_block(state, bytes + at);
        at += 64;
    }

    remaining := count - at;
    if remaining > 0 {
        memcpy(state.buffer.data + state.buffered, bytes + at, remaining);
        state.buffered += remaining;
    }
}

md5_update :: (state: *MD5_State, s: string) {
    md5_update(state, s.data, s.count);
}

// Appends a 1 bit, zeroes to 56 mod 64, then the length in bits as a
// little-endian u64 — the padding RFC 1321 specifies.
md5_final :: (state: *MD5_State) -> [16] u8 {
    bit_count := state.total * 8;

    padding: [72] u8;
    padding[0] = 0x80;
    // How many zero bytes take the total to 56 mod 64.
    zeroes := (56 - cast(s64) ((state.total + 1) % 64) + 64) % 64;
    md5_update(state, padding.data, 1 + zeroes);

    length: [8] u8;
    for 0..7  length[it] = cast(u8) ((bit_count >> cast(u64) (8 * it)) & 0xff);
    md5_update(state, length.data, 8);

    digest: [16] u8;
    write_u32_le(*digest, 0,  state.a);
    write_u32_le(*digest, 4,  state.b);
    write_u32_le(*digest, 8,  state.c);
    write_u32_le(*digest, 12, state.d);
    return digest;
}

// The digest as lowercase hexadecimal, which is how MD5 is nearly always
// written down.
md5_hex :: (digest: [16] u8, allocator := context.allocator) -> string {
    DIGITS :: "0123456789abcdef";
    result: string;
    result.data = cast(*u8) alloc(32, allocator);
    if !result.data  return result;
    result.count = 32;
    for 0..15 {
        result[it * 2]     = DIGITS[digest[it] >> 4];
        result[it * 2 + 1] = DIGITS[digest[it] & 0xf];
    }
    return result;
}

md5_hex :: (s: string, allocator := context.allocator) -> string {
    return md5_hex(md5(s), allocator);
}

#scope_module

// Per-round shift amounts, four groups of four repeated four times each.
SHIFTS :: u32.[
    7, 12, 17, 22,  7, 12, 17, 22,  7, 12, 17, 22,  7, 12, 17, 22,
    5,  9, 14, 20,  5,  9, 14, 20,  5,  9, 14, 20,  5,  9, 14, 20,
    4, 11, 16, 23,  4, 11, 16, 23,  4, 11, 16, 23,  4, 11, 16, 23,
    6, 10, 15, 21,  6, 10, 15, 21,  6, 10, 15, 21,  6, 10, 15, 21,
];

// floor(abs(sin(i + 1)) * 2^32) for i in 0..63, as RFC 1321 tabulates them.
K :: u32.[
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
    0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
    0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
    0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
    0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
    0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
    0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
    0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
    0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

md5_block :: (state: *MD5_State, block: *u8) {
    m: [16] u32;
    for 0..15  m[it] = read_u32_le(block, it * 4);

    a := state.a;
    b := state.b;
    c := state.c;
    d := state.d;

    for i: 0..63 {
        f: u32;
        g: s64;
        if i < 16 {
            f = (b & c) | (~b & d);
            g = i;
        } else if i < 32 {
            f = (d & b) | (~d & c);
            g = (5 * i + 1) % 16;
        } else if i < 48 {
            f = b ^ c ^ d;
            g = (3 * i + 5) % 16;
        } else {
            f = c ^ (b | ~d);
            g = (7 * i) % 16;
        }

        f = f + a + K[i] + m[g];
        a = d;
        d = c;
        c = b;
        b = b + rotate_left(f, SHIFTS[i]);
    }

    state.a += a;
    state.b += b;
    state.c += c;
    state.d += d;
}

rotate_left :: (value: u32, amount: u32) -> u32 {
    shift := amount & 31;
    if shift == 0  return value;
    return (value << shift) | (value >> (32 - shift));
}

read_u32_le :: (bytes: *u8, at: s64) -> u32 {
    return cast(u32) bytes[at]
         | (cast(u32) bytes[at + 1] << 8)
         | (cast(u32) bytes[at + 2] << 16)
         | (cast(u32) bytes[at + 3] << 24);
}

write_u32_le :: (bytes: *[16] u8, at: s64, value: u32) {
    bytes.*[at]     = cast(u8) (value & 0xff);
    bytes.*[at + 1] = cast(u8) ((value >> 8) & 0xff);
    bytes.*[at + 2] = cast(u8) ((value >> 16) & 0xff);
    bytes.*[at + 3] = cast(u8) ((value >> 24) & 0xff);
}
