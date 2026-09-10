//
// PCG: a permuted congruential generator (O'Neill, 2014).
//
// A 64-bit linear congruential step for the state, then an xorshift and a
// data-dependent rotate to produce 32 bits of output. The permutation is what
// fixes the LCG's well-known weakness — that its low bits have short periods —
// without needing a large state.
//
// Deterministic and fast, not cryptographic: never generate a key with this.
//

PCG_State :: struct {
    state:     u64;
    increment: u64 = 1;   // must be odd
}

// Two generators seeded with different sequences produce independent streams
// even from the same seed.
pcg_seed :: (rng: *PCG_State, seed: u64, sequence: u64 = 1) {
    rng.state = 0;
    rng.increment = (sequence << 1) | 1;
    pcg_random_u32(rng);
    rng.state += seed;
    pcg_random_u32(rng);
}

MULTIPLIER :: 6364136223846793005;

pcg_random_u32 :: (rng: *PCG_State) -> u32 {
    old := rng.state;
    rng.state = old * MULTIPLIER + rng.increment;

    // Xorshift the high bits down, then rotate by an amount the top five bits
    // choose — the "XSH RR" output function.
    xorshifted := cast(u32) (((old >> 18) ^ old) >> 27);
    rotation := cast(u32) (old >> 59);
    return rotate_right(xorshifted, rotation);
}

pcg_random_u64 :: (rng: *PCG_State) -> u64 {
    high := cast(u64) pcg_random_u32(rng);
    low  := cast(u64) pcg_random_u32(rng);
    return (high << 32) | low;
}

// In [0, limit). Rejection sampling rather than a modulo, so every value is
// equally likely however `limit` divides the range.
pcg_random_below :: (rng: *PCG_State, limit: u32) -> u32 {
    if limit == 0  return 0;
    threshold := (~limit + 1) % limit;   // 2^32 % limit
    while true {
        value := pcg_random_u32(rng);
        if value >= threshold  return value % limit;
    }
    return 0;
}

// In [0, 1). 24 bits of mantissa is what a float32 holds exactly.
pcg_random_float :: (rng: *PCG_State) -> float32 {
    return cast(float32) (pcg_random_u32(rng) >> 8) * (1.0 / 16777216.0);
}

#scope_module

rotate_right :: (value: u32, amount: u32) -> u32 {
    shift := amount & 31;
    if shift == 0  return value;
    return (value >> shift) | (value << (32 - shift));
}
