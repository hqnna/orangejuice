//
// xxHash (XXH64), Yann Collet's non-cryptographic hash.
//
// Much faster than FNV on anything longer than a few bytes: it consumes four
// 8-byte lanes at a time in the main loop, so the multiplies pipeline instead
// of serialising. Written from the published specification and checked
// against its test vectors.
//
// Not cryptographic. Never use it where an attacker chooses the input and
// collisions matter.
//

PRIME64_1 : u64 : 11400714785074694791;
PRIME64_2 : u64 : 14029467366897019727;
PRIME64_3 : u64 :  1609587929392839161;
PRIME64_4 : u64 :  9650029242287828579;
PRIME64_5 : u64 :  2870177450012600261;

xxhash64 :: (bytes: *u8, count: s64, seed: u64 = 0) -> u64 {
    hash: u64;
    at := 0;

    if count >= 32 {
        // Four independent lanes, so the multiply chains do not depend on one
        // another and the processor can overlap them.
        v1 := seed + PRIME64_1 + PRIME64_2;
        v2 := seed + PRIME64_2;
        v3 := seed;
        v4 := seed - PRIME64_1;

        while count - at >= 32 {
            v1 = round(v1, read_u64(bytes, at));       at += 8;
            v2 = round(v2, read_u64(bytes, at));       at += 8;
            v3 = round(v3, read_u64(bytes, at));       at += 8;
            v4 = round(v4, read_u64(bytes, at));       at += 8;
        }

        hash = rotate_left(v1, 1) + rotate_left(v2, 7) + rotate_left(v3, 12) + rotate_left(v4, 18);
        hash = merge_round(hash, v1);
        hash = merge_round(hash, v2);
        hash = merge_round(hash, v3);
        hash = merge_round(hash, v4);
    } else {
        hash = seed + PRIME64_5;
    }

    hash += cast(u64) count;

    // Whatever is left, eight bytes then four then one at a time.
    while count - at >= 8 {
        k := round(0, read_u64(bytes, at));
        hash ^= k;
        hash = rotate_left(hash, 27) * PRIME64_1 + PRIME64_4;
        at += 8;
    }

    if count - at >= 4 {
        hash ^= (cast(u64) read_u32(bytes, at)) * PRIME64_1;
        hash = rotate_left(hash, 23) * PRIME64_2 + PRIME64_3;
        at += 4;
    }

    while at < count {
        hash ^= (cast(u64) bytes[at]) * PRIME64_5;
        hash = rotate_left(hash, 11) * PRIME64_1;
        at += 1;
    }

    // Avalanche: makes every input bit affect every output bit.
    hash ^= hash >> 33;
    hash *= PRIME64_2;
    hash ^= hash >> 29;
    hash *= PRIME64_3;
    hash ^= hash >> 32;
    return hash;
}

xxhash64 :: (s: string, seed: u64 = 0) -> u64 {
    return xxhash64(s.data, s.count, seed);
}

#scope_module

round :: (accumulator: u64, input: u64) -> u64 {
    value := accumulator + input * PRIME64_2;
    value = rotate_left(value, 31);
    return value * PRIME64_1;
}

merge_round :: (accumulator: u64, value: u64) -> u64 {
    rounded := round(0, value);
    result := accumulator ^ rounded;
    return result * PRIME64_1 + PRIME64_4;
}

rotate_left :: (value: u64, amount: u64) -> u64 {
    shift := amount & 63;
    if shift == 0  return value;
    return (value << shift) | (value >> (64 - shift));
}

// Little-endian, which is what the specification fixes regardless of the
// machine.
read_u64 :: (bytes: *u8, at: s64) -> u64 {
    result: u64 = 0;
    for 0..7  result |= cast(u64) bytes[at + it] << cast(u64) (8 * it);
    return result;
}

read_u32 :: (bytes: *u8, at: s64) -> u32 {
    result: u32 = 0;
    for 0..3  result |= cast(u32) bytes[at + it] << cast(u32) (8 * it);
    return result;
}
