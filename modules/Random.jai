//
// Random numbers from a generator the program shares.
//
// The state is a global rather than something threaded through every call, so
// `random_get()` reads like the convenience it is. A program that wants
// reproducibility calls `random_seed` at the start; a program that wants two
// independent streams uses `PCG` directly.
//
// xoshiro256**: four 64-bit words, a period of 2^256-1, and better spread than
// the linear congruential generators it replaces. Not cryptographic.
//

#import "PCG";

#scope_module

Random_State :: struct {
    s: [4] u64;
}

// Seeded to something arbitrary but fixed, so a program that never calls
// `random_seed` still gets the same run twice — which is what makes a failure
// reproducible.
global_state: Random_State = .{.[
    0x853c_49e6_748f_ea9b,
    0xda3e_39cb_94b9_5bdb,
    0x9e37_79b9_7f4a_7c15,
    0xbf58_476d_1ce4_e5b9,
]};

#scope_export

// Spreads one number over the four words with SplitMix64, so that seeding with
// 0 or 1 does not leave most of the state empty.
random_seed :: (seed: u64) {
    z := seed;
    for 0..3 {
        z += 0x9e37_79b9_7f4a_7c15;
        x := z;
        x = (x ^ (x >> 30)) * 0xbf58_476d_1ce4_e5b9;
        x = (x ^ (x >> 27)) * 0x94d0_49bb_1331_11eb;
        global_state.s[it] = x ^ (x >> 31);
    }
}

random_get :: () -> u64 {
    s := *global_state.s;

    result := rotate_left(s.*[1] * 5, 7) * 9;
    t := s.*[1] << 17;

    s.*[2] ^= s.*[0];
    s.*[3] ^= s.*[1];
    s.*[1] ^= s.*[2];
    s.*[0] ^= s.*[3];
    s.*[2] ^= t;
    s.*[3] = rotate_left(s.*[3], 45);

    return result;
}

// In [0, limit). Rejection sampling, so the distribution stays flat however
// `limit` divides 2^64.
random_get_within_range :: (limit: u64) -> u64 {
    if limit == 0  return 0;
    threshold := (~limit + 1) % limit;   // 2^64 % limit
    while true {
        value := random_get();
        if value >= threshold  return value % limit;
    }
    return 0;
}

// In [low, high]. An inverted range is treated as the range it names.
random_get_within_range :: (low: s64, high: s64) -> s64 {
    lo := low;
    hi := high;
    if hi < lo { t := lo; lo = hi; hi = t; }
    span := cast(u64) (hi - lo) + 1;
    return lo + cast(s64) random_get_within_range(span);
}

// In [0, 1). 53 bits is what a float64 holds exactly.
random_get_zero_to_one :: () -> float64 {
    return cast(float64) (random_get() >> 11) * (1.0 / 9007199254740992.0);
}

random_get_zero_to_one_open :: () -> float64 {
    // Never exactly 0, for callers dividing by it.
    return (cast(float64) (random_get() >> 11) + 0.5) * (1.0 / 9007199254740992.0);
}

// Fisher-Yates: every permutation equally likely, in place.
shuffle :: (array: [] $T) {
    if array.count < 2  return;
    index := array.count - 1;
    while index > 0 {
        pick := cast(s64) random_get_within_range(cast(u64) (index + 1));
        temporary := array[index];
        array[index] = array[pick];
        array[pick] = temporary;
        index -= 1;
    }
}

random_choice :: (array: [] $T) -> (value: T, ok: bool) {
    if !array.count {
        empty: T;
        return empty, false;
    }
    return array[random_get_within_range(cast(u64) array.count)], true;
}

#scope_module

rotate_left :: (value: u64, amount: u64) -> u64 {
    shift := amount & 63;
    if shift == 0  return value;
    return (value << shift) | (value >> (64 - shift));
}
