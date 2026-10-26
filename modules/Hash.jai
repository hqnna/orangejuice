//
// Hash functions for short strings and small values.
//
// `get_hash` is the one `Hash_Table` calls; the named functions under it are
// there for a caller that wants a particular one. All the 32-bit ones take an
// `h` to chain from, so hashing B with A's result mixes the two — a design the
// reference itself flags as the wrong way round, since it forces every hash
// function used here to grow that parameter. Expect it to be replaced by a
// mixing function that combines any two results.
//

#import "Basic";

// Hash from the sdbm project: `hash(i) = hash(i-1) * 65599 + data[i]`, written
// as shifts. Like djb2 it produces near-sequential hashes for inputs with
// sequential trailing bytes, which asset names often are; fnv1a is the better
// general-purpose string hash.
sdbm_hash :: (data: *void, size: int, h := HASH_INIT) -> u32 {
    hash := h;
    for 0..size - 1 {
        hash = (hash << 16) + (hash << 6) - hash + (cast(*u8) data)[it];
    }
    return hash;
}

// The same, with negative zero folded onto zero so that the two hash alike.
// Cleaning up the input is usually the better answer.
sdbm_float_hash :: inline (f: *float, count: int, h := HASH_INIT) -> u32 {
    hash := h;
    for 0..count - 1 {
        bits := (cast(*u32) (f + it)).*;
        if bits == 0x8000_0000  bits = 0;
        hash = inline sdbm_hash(*bits, 4, hash);
    }
    return hash;
}

djb2_hash :: (s: string) -> u32 {
    hash := HASH_INIT;
    for 0..s.count - 1  hash = ((hash << 5) + hash) + s[it];  // hash * 33 + c
    return hash;
}

// FNV-1a. Good distribution and chains well when hashing repeatedly. Fold it
// into the range you want by xor rather than by shifting.
FNV_64_PRIME       : u64 : 0x100000001b3;
FNV_64_OFFSET_BIAS : u64 : 0xcbf29ce484222325;

fnv1a_hash :: inline (val: u64, h := FNV_64_OFFSET_BIAS) -> u64 {
    hash := h;
    hash ^= val;
    return hash * FNV_64_PRIME;
}

fnv1a_hash :: (data: *void, size: int, h := FNV_64_OFFSET_BIAS) -> u64 {
    hash := h;
    for 0..size - 1  hash = fnv1a_hash((cast(*u8) data)[it], hash);
    return hash;
}

// Knuth's multiplicative hash. The *high* bits are the good ones, so shift it
// down to the range you want rather than masking it.
knuth_hash :: inline (x: u64) -> u64 {
    KNUTH_GOLDEN_RATIO_64 :: 11400714819323198485;
    return KNUTH_GOLDEN_RATIO_64 * x;
}

// ------------------------------------------------------------ the general ---

// Anything else: a number, a bool, an enum or a pointer goes through Knuth,
// which is both better and faster for the small integers a table's keys
// usually are; a float through sdbm, because Knuth's multiply would map its
// exponent bits badly; and a struct, a fixed array or a distinct type over
// any of these through sdbm over its bytes (hitboxer hashes a
// `Source_Code_Location`). The choice is made with `type_info(T)`, which
// folds, rather than with a `#modify`: the JIT has to build a whole
// `#Context` to run one, and a table in that context hashes with this.
get_hash :: (x: $T, h := HASH_INIT) -> u32 {
    #if type_info(T).type == .INTEGER || type_info(T).type == .BOOL || type_info(T).type == .ENUM || type_info(T).type == .POINTER {
        return cast,trunc(u32) (knuth_hash(cast,no_check(u64) x ^ h) >> 32);
    } else {
        value := x;
        return inline sdbm_hash(*value, size_of(T), h);
    }
}

get_hash :: (s: string, h := HASH_INIT) -> u32 {
    return cast,trunc(u32) inline fnv1a_hash(s.data, s.count, h);
}

get_hash :: (array: [] $T, h := HASH_INIT) -> u32 {
    return inline sdbm_hash(array.data, array.count * size_of(T), h);
}

#scope_file

HASH_INIT : u32 : 5381;
