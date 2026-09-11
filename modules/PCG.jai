//
// PCG: a small, fast, statistically good random number generator.
//
// The state is one 64-bit number advanced by a linear congruential step. What
// comes out is not that state but a *permutation* of its high bits — an xorshift
// whose amount is itself chosen by bits of the state (XSH RR). That last touch
// is what turns an LCG, whose low bits are famously poor, into a generator that
// passes the usual test batteries at this size.
//
// Every routine comes in two forms: one taking the state to advance, and one
// using a generator the module keeps. The second is what a program that does
// not care about reproducibility wants; the first is what a program that does
// should use, since a global is shared with everything else in the process.
//

#import "Basic";

// One draw, and the state advanced past it.
random_get :: (state: *u64) -> u32 {
    previous := state.*;
    state.* = state.* * PCG_DEFAULT_MULTIPLIER_64 + PCG_DEFAULT_INCREMENT_64;

    // The output is computed from the *old* state so that the multiply above
    // and the shifts below can run at the same time.
    xorshifted := cast,trunc(u32) (((previous >> 18) ^ previous) >> 27);
    rot := cast(u32) (previous >> 59);
    return (xorshifted >> rot) | (xorshifted << (neg(rot) & 31));
}

random_get :: inline () -> u32 {
    return random_get(*pcg32_state);
}

random_seed :: (state: *u64, new_seed: u32) {
    state.* = cast(u64) new_seed * PCG_DEFAULT_MULTIPLIER_64 + PCG_DEFAULT_INCREMENT_64;
}

random_seed :: inline (new_seed: u32) {
    random_seed(*pcg32_state, new_seed);
}

// A draw below `range`, without the bias that a plain `%` would introduce: the
// values that would make the last, short bucket are thrown away and redrawn.
random_get_within_bound :: (state: *u64, range: u32) -> u32 {
    assert(range != 0);
    threshold := neg(range) % range;
    while true {
        r := random_get(state);
        if r >= threshold  return r % range;
    }
    return 0;
}

random_get_within_bound :: inline (bound: u32) -> u32 {
    return random_get_within_bound(*pcg32_state, bound);
}

// The same answer by Lemire's method: one multiply gives the bucket, and only
// the rare draw that lands in the ragged tail needs a rejection test. Faster
// than the modulo version, and takes the same number of draws in the common
// case.
random_get_within_bound2 :: (state: *u64, range: u32) -> u32 {
    assert(range != 0);

    x := random_get(state);
    m := cast(u64) x * cast(u64) range;
    l := cast,trunc(u32) m;

    if l < range {
        t: u32 = neg(range);
        if t >= range {
            t -= range;
            if t >= range  t %= range;
        }
        while l < t {
            x = random_get(state);
            m = cast(u64) x * cast(u64) range;
            l = cast,trunc(u32) m;
        }
    }
    return cast(u32) (m >> 32);
}

random_get_within_bound2 :: inline (bound: u32) -> u32 {
    return random_get_within_bound2(*pcg32_state, bound);
}

random_get_zero_to_one :: (state: *u64) -> float {
    MAX_U32: float64 : 0xffff_ffff;
    return cast(float) (cast(float64) random_get(state) / MAX_U32);
}

random_get_zero_to_one :: () -> float {
    return random_get_zero_to_one(*pcg32_state);
}

random_get_within_range :: (state: *u64, min: float, max: float) -> float {
    return random_get_zero_to_one(state) * (max - min) + min;
}

random_get_within_range :: (min: float, max: float) -> float {
    return random_get_within_range(*pcg32_state, min, max);
}

#scope_module

// The generator the argument-less forms use. It starts from the increment
// rather than from zero so that a program which never seeds still gets a
// usable sequence.
pcg32_state: u64 = PCG_DEFAULT_INCREMENT_64;

PCG_DEFAULT_MULTIPLIER_64 :: 6364136223846793005;
PCG_DEFAULT_INCREMENT_64  :: 1442695040888963407;

// Two's-complement negation of an unsigned value, which is what the rejection
// thresholds above are expressed in terms of.
neg :: inline (x: u32) -> u32 {
    return cast,no_check(u32) -cast,no_check(s32) x;
}
