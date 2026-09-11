//
// A 128-bit multiplicative congruential generator.
//
// The state is one 128-bit odd number, multiplied by a fixed 128-bit factor at
// every step; what comes out is the top half. An MCG's low bits are poor — the
// bottom bit never changes at all — which is exactly why the *high* word is
// what is returned and why the float conversions take bits from the top.
//
// It is the "today's 64-bit minimal standard" generator: small, fast, and good
// enough for anything that is not cryptography. It is not cryptography.
//
// The state lives in the context, so two threads each get their own and a
// program that wants a reproducible stream can hold one of its own instead.
//

#import "Basic";
#import "Bit_Operations";

Random_State :: struct {
    low:  u64;
    high: u64;
}

// One step, and the high half of the new state.
random_get :: (state: *Random_State) -> u64 {
    FACTOR_HIGH : u64 : 0x0fc9_4e3b_f4e9_ab32;
    FACTOR_LOW  : u64 : 0x8664_58cd_56f5_e605;

    // `state = state * FACTOR` over 128 bits. The product of the two high
    // halves would land entirely above the top and is dropped.
    product_high, product_low := multiply_64bits_with_overflow(state.low, FACTOR_LOW);

    // The old `low` is needed here, so the high word is computed first.
    state.high = product_high + (state.high * FACTOR_LOW) + (state.low * FACTOR_HIGH);
    state.low = product_low;
    return state.high;
}

random_get :: inline () -> u64 {
    return random_get(*context.random_state);
}

// The state must be odd to have full period, so the seed goes in the high word
// and the low one is set to 1.
random_seed :: inline (state: *Random_State, new_seed: u64) {
    state.low = 1;
    state.high = new_seed;
}

random_seed :: inline (new_seed: u64) {
    random_seed(*context.random_state, new_seed);
}

random_seed :: inline (state: *Random_State, new_seed: Random_State) {
    state.* = new_seed;
}

random_seed :: inline (new_seed: Random_State) {
    random_seed(*context.random_state, new_seed);
}

// ------------------------------------------------------------ as a float ---
//
// These take 24 bits rather than all 64: a `float` cannot represent every
// integer past 2^24, so dividing by a bigger range would introduce a bias the
// generator does not have.

random_get_zero_to_one :: inline (state: *Random_State) -> float {
    randint := random_get(state) & MASK;
    return randint / cast(float) MASK;
}

random_get_zero_to_one :: inline () -> float {
    return random_get_zero_to_one(*context.random_state);
}

// Half-open: dividing by the range rather than by the largest value in it
// means 1.0 is never produced.
random_get_zero_to_one_open :: inline (state: *Random_State) -> float {
    randint := random_get(state) & MASK;
    return randint / cast(float) RANDRANGE;
}

random_get_zero_to_one_open :: inline () -> float {
    return random_get_zero_to_one_open(*context.random_state);
}

// The full-precision versions. This one builds the double's bits directly, so
// that every representable value in [0, 1] can come out — including the very
// small ones a plain divide can never reach.
random_get_zero_to_one_new :: (state: *Random_State) -> float64 {
    Bits :: union {
        d: float64;
        x: u64;
    }

    // The number of low zero bits, which is how the exponent is chosen.
    ctz :: inline (x: u64) -> s32 {
        result := bit_scan_forward(x);
        if result == 0  return 64;
        return cast(s32) (result - 1);
    }

    bits: Bits;
    bits.x = random_get(state);
    e := ctz(bits.x) - 11;
    // A mantissa with room to spare means the exponent needs another draw to
    // decide how far down it goes.
    if e >= 0  e = ctz(random_get(state));

    bits.x = (((bits.x >> 11) + 1) >> 1) - ((cast,no_check(u64) e - 1011) << 52);
    return bits.d;
}

random_get_zero_to_one_new :: inline () -> float64 {
    return random_get_zero_to_one_new(*context.random_state);
}

random_get_zero_to_one_open_new :: (state: *Random_State) -> float64 {
    u := random_get(state) >> 11;
    return u * 0h3ca0_0000_0000_0000;  // 2^-53.
}

random_get_zero_to_one_open_new :: inline () -> float64 {
    return random_get_zero_to_one_open_new(*context.random_state);
}

// Anywhere in [min, max]. The two may be given either way round.
random_get_within_range :: inline (state: *Random_State, min: float, max: float) -> float {
    randint := random_get(state) & MASK;
    result := min + (randint / cast(float) MASK) * (max - min);

    // The multiply can land a hair outside the range it was scaled into.
    if result > max  result = max;
    if result < min  result = min;
    return result;
}

random_get_within_range :: inline (min: float, max: float) -> float {
    return random_get_within_range(*context.random_state, min, max);
}

// ------------------------------------------------------------- over arrays ---

// Fisher-Yates: each element is swapped with one drawn from the part of the
// array that has not been placed yet, which makes every permutation equally
// likely.
shuffle :: (array: [] $T) {
    if array.count < 2  return;
    at := array.count - 1;
    while at > 0 {
        pick := cast(s64) (random_get() % cast(u64) (at + 1));
        array[at], array[pick] = array[pick], array[at];
        at -= 1;
    }
}

random_choice :: (array: [] $T) -> (value: T, ok: bool) {
    empty: T;
    if !array.count  return empty, false;
    return array[cast(s64) (random_get() % cast(u64) array.count)], true;
}

#scope_file

// A float cannot represent every integer past 2^24, so the float conversions
// draw from that range rather than from all 64 bits.
RANDRANGE : u32 : 1 << 24;
MASK :: RANDRANGE - 1;
#assert (RANDRANGE & MASK) == 0;

#scope_export

// The state every argument-less call uses. Starting it at an arbitrary odd
// number means a program that never seeds still gets a usable stream.
#add_context random_state := Random_State.{1, 0};
