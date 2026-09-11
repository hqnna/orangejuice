//
// xxHash: a fast non-cryptographic hash.
//
// Two of them, 32-bit and 64-bit, each in a one-shot form (`native_XXH32`,
// `native_XXH64`) and a streaming one (`_reset`, `_update`, `_digest`) for
// input that does not arrive all at once. The streaming state holds a small
// buffer, so a caller may hand over any size of piece.
//
// The shape is four accumulators, each taking a word per round and being
// rotated and multiplied by a prime; at the end they are folded together and
// the leftover bytes mixed in. That is where the speed comes from — the four
// chains are independent, so the processor runs them at once.
//
// Not cryptography. It is for hash tables, checksums and content addressing.
//

#import "Basic";

// 0.7.0.
XXH_VERSION_MAJOR   :: 0;
XXH_VERSION_MINOR   :: 7;
XXH_VERSION_RELEASE :: 0;
XXH_VERSION_NUMBER  :: XXH_VERSION_MAJOR * 100 * 100 + XXH_VERSION_MINOR * 100 + XXH_VERSION_RELEASE;

XXH_versionNumber :: () -> u32 { return XXH_VERSION_NUMBER; }

// ------------------------------------------------------------------ 32 bit ---

XXH32_state_t :: struct {
    total_len_32: u32;
    // Whether sixteen bytes have ever gone through, which decides whether the
    // accumulators mean anything.
    large_len: u32;
    v1: u32;
    v2: u32;
    v3: u32;
    v4: u32;
    // What is left over from the last update, waiting for a full block.
    mem32: [4] u32;
    memsize: u32;
    reserved: u32;
}

native_XXH32 :: (input: *void, len: u32, seed: u32) -> u32 #must {
    state: XXH32_state_t;
    XXH32_reset(*state, seed);
    XXH32_update(*state, input, len);
    return XXH32_digest(*state);
}

XXH32_reset :: (state: *XXH32_state_t, seed: u32) {
    memset(state, 0, size_of(XXH32_state_t));

    state.v1 = seed + PRIME32_1 + PRIME32_2;
    state.v2 = seed + PRIME32_2;
    state.v3 = seed + 0;
    state.v4 = seed - PRIME32_1;
}

XXH32_update :: (state: *XXH32_state_t, input: *void, len: u32) {
    p := cast(*u8) input;
    end := p + len;

    state.total_len_32 += len;
    state.large_len |= cast(u32) (state.total_len_32 >= 16);

    // Not a whole block even with what was held over: keep it and wait.
    if state.memsize + len < 16 {
        memcpy(cast(*u8) state.mem32.data + state.memsize, input, len);
        state.memsize += len;
        return;
    }

    if state.memsize {
        // Top the held bytes up to a block and consume it.
        memcpy(cast(*u8) state.mem32.data + state.memsize, input, 16 - state.memsize);
        at := state.mem32.data;
        state.v1 = XXH32_round(state.v1, at.*);  at += 1;
        state.v2 = XXH32_round(state.v2, at.*);  at += 1;
        state.v3 = XXH32_round(state.v3, at.*);  at += 1;
        state.v4 = XXH32_round(state.v4, at.*);

        p += 16 - state.memsize;
        state.memsize = 0;
    }

    if p <= end - 16 {
        limit := end - 16;
        // Kept in locals through the loop: four independent chains.
        v1 := state.v1;
        v2 := state.v2;
        v3 := state.v3;
        v4 := state.v4;

        while true {
            v1 = XXH32_round(v1, (cast(*u32) p).*);  p += 4;
            v2 = XXH32_round(v2, (cast(*u32) p).*);  p += 4;
            v3 = XXH32_round(v3, (cast(*u32) p).*);  p += 4;
            v4 = XXH32_round(v4, (cast(*u32) p).*);  p += 4;
            if p > limit  break;
        }

        state.v1 = v1;
        state.v2 = v2;
        state.v3 = v3;
        state.v4 = v4;
    }

    if p < end {
        memcpy(state.mem32.data, p, end - p);
        state.memsize = cast(u32) (end - p);
    }
}

XXH32_digest :: (state: *XXH32_state_t) -> u32 {
    h32: u32;

    if state.large_len {
        h32 = (state.v1 <<< 1) + (state.v2 <<< 7) + (state.v3 <<< 12) + (state.v4 <<< 18);
    } else {
        // Under sixteen bytes nothing ever went through a round, so the seed
        // itself starts the mixing.
        h32 = state.v3 + PRIME32_5;
    }

    h32 += state.total_len_32;

    return XXH32_finalize(h32, state.mem32.data, state.memsize);
}

// ------------------------------------------------------------------ 64 bit ---

XXH64_state_t :: struct {
    total_len: u64;
    v1: u64;
    v2: u64;
    v3: u64;
    v4: u64;
    mem64: [4] u64;
    memsize: u32;
    reserved: [2] u32;
}

native_XXH64 :: (input: *void, len: u32, seed: u32) -> u64 #must {
    state: XXH64_state_t;
    XXH64_reset(*state, seed);
    XXH64_update(*state, input, len);
    return XXH64_digest(*state);
}

XXH64_reset :: (state: *XXH64_state_t, seed: u64) {
    memset(state, 0, size_of(XXH64_state_t));

    state.v1 = seed + PRIME64_1 + PRIME64_2;
    state.v2 = seed + PRIME64_2;
    state.v3 = seed + 0;
    state.v4 = seed - PRIME64_1;
}

XXH64_update :: (state: *XXH64_state_t, input: *void, len: u32) {
    p := cast(*u8) input;
    end := p + len;

    state.total_len += len;

    if state.memsize + len < 32 {
        memcpy(cast(*u8) state.mem64.data + state.memsize, input, len);
        state.memsize += len;
        return;
    }

    if state.memsize {
        memcpy(cast(*u8) state.mem64.data + state.memsize, input, 32 - state.memsize);
        at := state.mem64.data;
        state.v1 = XXH64_round(state.v1, at.*);  at += 1;
        state.v2 = XXH64_round(state.v2, at.*);  at += 1;
        state.v3 = XXH64_round(state.v3, at.*);  at += 1;
        state.v4 = XXH64_round(state.v4, at.*);

        p += 32 - state.memsize;
        state.memsize = 0;
    }

    if p <= end - 32 {
        limit := end - 32;
        v1 := state.v1;
        v2 := state.v2;
        v3 := state.v3;
        v4 := state.v4;

        while true {
            v1 = XXH64_round(v1, (cast(*u64) p).*);  p += 8;
            v2 = XXH64_round(v2, (cast(*u64) p).*);  p += 8;
            v3 = XXH64_round(v3, (cast(*u64) p).*);  p += 8;
            v4 = XXH64_round(v4, (cast(*u64) p).*);  p += 8;
            if p > limit  break;
        }

        state.v1 = v1;
        state.v2 = v2;
        state.v3 = v3;
        state.v4 = v4;
    }

    if p < end {
        memcpy(state.mem64.data, p, end - p);
        state.memsize = cast(u32) (end - p);
    }
}

XXH64_digest :: (state: *XXH64_state_t) -> u64 {
    h64: u64;

    if state.total_len >= 32 {
        h64 = (state.v1 <<< 1) + (state.v2 <<< 7) + (state.v3 <<< 12) + (state.v4 <<< 18);
        h64 = XXH64_mergeRound(h64, state.v1);
        h64 = XXH64_mergeRound(h64, state.v2);
        h64 = XXH64_mergeRound(h64, state.v3);
        h64 = XXH64_mergeRound(h64, state.v4);
    } else {
        h64 = state.v3 + PRIME64_5;
    }

    h64 += state.total_len;

    return XXH64_finalize(h64, state.mem64.data, state.total_len);
}

// ------------------------------------------------------- the older spelling ---

// A whole buffer in one call, with the seed last, which is what this module
// used to offer and what our own code was written against.
xxhash64 :: (bytes: *u8, count: s64, seed: u64 = 0) -> u64 {
    state: XXH64_state_t;
    XXH64_reset(*state, seed);
    XXH64_update(*state, bytes, cast(u32) count);
    return XXH64_digest(*state);
}

xxhash64 :: (s: string, seed: u64 = 0) -> u64 {
    return xxhash64(s.data, s.count, seed);
}

#scope_file

PRIME32_1 : u32 : 2654435761;
PRIME32_2 : u32 : 2246822519;
PRIME32_3 : u32 : 3266489917;
PRIME32_4 : u32 : 668265263;
PRIME32_5 : u32 : 374761393;

PRIME64_1 : u64 : 11400714785074694791;
PRIME64_2 : u64 : 14029467366897019727;
PRIME64_3 : u64 : 1609587929392839161;
PRIME64_4 : u64 : 9650029242287828579;
PRIME64_5 : u64 : 2870177450012600261;

XXH32_round :: inline (acc: u32, input: u32) -> u32 {
    result := acc + input * PRIME32_2;
    result = result <<< 13;
    return result * PRIME32_1;
}

// Mixes every bit into every other one, so that two inputs differing in one
// place do not produce results differing in one place.
XXH32_avalanche :: (h: u32) -> u32 {
    h32 := h;
    h32 ^= h32 >> 15;
    h32 *= PRIME32_2;
    h32 ^= h32 >> 13;
    h32 *= PRIME32_3;
    h32 ^= h32 >> 16;
    return h32;
}

// The bytes past the last whole block: four at a time while there are four,
// then one at a time.
XXH32_finalize :: (h: u32, ptr: *void, length: int) -> u32 {
    h32 := h;
    p := cast(*u8) ptr;
    left := length & 15;

    while left >= 4 {
        h32 += (cast(*u32) p).* * PRIME32_3;
        h32 = (h32 <<< 17) * PRIME32_4;
        p += 4;
        left -= 4;
    }

    while left > 0 {
        h32 += cast(u32) p.* * PRIME32_5;
        h32 = (h32 <<< 11) * PRIME32_1;
        p += 1;
        left -= 1;
    }

    return XXH32_avalanche(h32);
}

XXH64_round :: inline (acc: u64, input: u64) -> u64 {
    result := acc + input * PRIME64_2;
    result = result <<< 31;
    return result * PRIME64_1;
}

// Folds one accumulator into the running hash. The extra multiply and add
// after the xor is what keeps two accumulators from cancelling.
XXH64_mergeRound :: inline (acc: u64, val: u64) -> u64 {
    mixed := XXH64_round(0, val);
    result := acc ^ mixed;
    return result * PRIME64_1 + PRIME64_4;
}

XXH64_avalanche :: (h: u64) -> u64 {
    h64 := h;
    h64 ^= h64 >> 33;
    h64 *= PRIME64_2;
    h64 ^= h64 >> 29;
    h64 *= PRIME64_3;
    h64 ^= h64 >> 32;
    return h64;
}

XXH64_finalize :: (h: u64, ptr: *void, length: u64) -> u64 {
    h64 := h;
    p := cast(*u8) ptr;
    left := length & 31;

    while left >= 8 {
        k1 := XXH64_round(0, (cast(*u64) p).*);
        h64 ^= k1;
        h64 = (h64 <<< 27) * PRIME64_1 + PRIME64_4;
        p += 8;
        left -= 8;
    }

    if left >= 4 {
        h64 ^= cast(u64) (cast(*u32) p).* * PRIME64_1;
        h64 = (h64 <<< 23) * PRIME64_2 + PRIME64_3;
        p += 4;
        left -= 4;
    }

    while left > 0 {
        h64 ^= cast(u64) p.* * PRIME64_5;
        h64 = (h64 <<< 11) * PRIME64_1;
        p += 1;
        left -= 1;
    }

    return XXH64_avalanche(h64);
}
