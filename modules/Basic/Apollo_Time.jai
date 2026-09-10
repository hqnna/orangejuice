//
// Apollo_Time: a signed 128-bit count of femtoseconds.
//
// One representation for every time a program deals with — a wall clock, a
// monotonic reading, a file's timestamp, a duration — so that subtracting two
// of them is always meaningful and no caller has to remember which unit a
// particular API chose.
//
// Femtoseconds in 128 bits covers roughly ±5.4 million years at a resolution
// finer than any clock reports, which is why the type never has to be widened
// or the unit renegotiated. The epoch is not Unix's:
// `APOLLO_TIME_FOR_JAN_1_1970` is the conversion.
//

Apollo_Time :: struct {
    low:  u64;
    high: s64;
}

FEMTOSECONDS :: 1_000_000_000_000_000;

APOLLO_TIME_FOR_JAN_1_1970 :: Apollo_Time.{low = 0xdbd87f0edb7c8000, high = 0x300};

// ------------------------------------------------------------- arithmetic ---

operator + :: (a: Apollo_Time, b: Apollo_Time) -> Apollo_Time {
    result: Apollo_Time;
    result.low = a.low + b.low;
    result.high = a.high + b.high;
    // The carry out of the low half is exactly the case where the sum wrapped.
    if result.low < a.low  result.high += 1;
    return result;
}

operator - :: (a: Apollo_Time, b: Apollo_Time) -> Apollo_Time {
    result: Apollo_Time;
    result.low = a.low - b.low;
    result.high = a.high - b.high;
    if a.low < b.low  result.high -= 1;
    return result;
}

operator - :: (a: Apollo_Time) -> Apollo_Time {
    zero: Apollo_Time;
    return zero - a;
}

operator == :: (a: Apollo_Time, b: Apollo_Time) -> bool {
    return a.low == b.low && a.high == b.high;
}

operator < :: (a: Apollo_Time, b: Apollo_Time) -> bool {
    if a.high != b.high  return a.high < b.high;
    return a.low < b.low;
}

operator > :: (a: Apollo_Time, b: Apollo_Time) -> bool {
    if a.high != b.high  return a.high > b.high;
    return a.low > b.low;
}

// ------------------------------------------------------------ conversions ---

seconds_to_apollo :: (seconds: s64) -> Apollo_Time {
    return scale_to_apollo(seconds, FEMTOSECONDS);
}

milliseconds_to_apollo :: (milliseconds: s64) -> Apollo_Time {
    return scale_to_apollo(milliseconds, 1_000_000_000_000);
}

microseconds_to_apollo :: (microseconds: s64) -> Apollo_Time {
    return scale_to_apollo(microseconds, 1_000_000_000);
}

nanoseconds_to_apollo :: (nanoseconds: s64) -> Apollo_Time {
    return scale_to_apollo(nanoseconds, 1_000_000);
}

// A fractional number of seconds. Only about 15 significant digits survive a
// `float64`, so this is for durations rather than for timestamps.
to_apollo :: (seconds: float64) -> Apollo_Time {
    whole := cast(s64) seconds;
    remainder := seconds - cast(float64) whole;
    result := seconds_to_apollo(whole);
    return result + Apollo_Time.{low = cast(u64) (remainder * cast(float64) FEMTOSECONDS), high = 0};
}

// `success` is false when the answer does not fit in an `s64` — which for
// nanoseconds means a span longer than about 292 years.
to_nanoseconds :: (t: Apollo_Time) -> (result: s64, success: bool) {
    return scale_from_apollo(t, 1_000_000);
}

to_microseconds :: (t: Apollo_Time) -> (result: s64, success: bool) {
    return scale_from_apollo(t, 1_000_000_000);
}

to_milliseconds :: (t: Apollo_Time) -> (result: s64, success: bool) {
    return scale_from_apollo(t, 1_000_000_000_000);
}

to_seconds :: (t: Apollo_Time) -> float64 {
    nanoseconds, ok := to_nanoseconds(t);
    if ok  return cast(float64) nanoseconds * 1.0e-9;
    // Too large for nanoseconds; seconds still fit comfortably.
    seconds, seconds_ok := scale_from_apollo(t, FEMTOSECONDS);
    return cast(float64) seconds;
}

// -------------------------------------------------------------- the clocks ---

// The wall clock, which is what a timestamp written down and read back later
// must come from. It can step backwards when the machine's time is corrected.
current_time_consensus :: () -> Apollo_Time {
    seconds, nanoseconds := clock_reading(CLOCK_REALTIME);
    return APOLLO_TIME_FOR_JAN_1_1970 + seconds_to_apollo(seconds) + nanoseconds_to_apollo(nanoseconds);
}

// A reading from an arbitrary origin that only ever goes forward, which is the
// one to measure an elapsed span with.
current_time_monotonic :: () -> Apollo_Time {
    seconds, nanoseconds := clock_reading(CLOCK_MONOTONIC);
    return seconds_to_apollo(seconds) + nanoseconds_to_apollo(nanoseconds);
}

#scope_module

CLOCK_REALTIME  :: 0;
CLOCK_MONOTONIC :: 1;

Apollo_Timespec :: struct {
    tv_sec:  s64;
    tv_nsec: s64;
}

clock_gettime :: (clock: s32, spec: *Apollo_Timespec) -> s32 #foreign libc_apollo;

libc_apollo :: #system_library "c";

clock_reading :: (clock: s32) -> (seconds: s64, nanoseconds: s64) {
    now: Apollo_Timespec;
    if clock_gettime(clock, *now) != 0  return 0, 0;
    return now.tv_sec, now.tv_nsec;
}

// `count` units of `femtoseconds_per_unit` each, as a 128-bit product.
scale_to_apollo :: (count: s64, femtoseconds_per_unit: s64) -> Apollo_Time {
    negative := count < 0;
    magnitude: u64 = ifx negative then (~cast(u64) count) + 1 else cast(u64) count;

    result := multiply_64(magnitude, cast(u64) femtoseconds_per_unit);
    if negative  return -result;
    return result;
}

// The reverse: how many whole units the time is, truncated toward zero.
scale_from_apollo :: (t: Apollo_Time, femtoseconds_per_unit: s64) -> (result: s64, success: bool) {
    negative := t.high < 0;
    magnitude := ifx negative then -t else t;

    quotient := divide_128(magnitude, cast(u64) femtoseconds_per_unit);
    // Anything with a high half set, or a low half past the signed maximum,
    // cannot be said in an s64.
    if quotient.high != 0  return 0, false;
    if quotient.low > 0x7fff_ffff_ffff_ffff  return 0, false;

    value := cast(s64) quotient.low;
    if negative  value = -value;
    return value, true;
}

// 64 × 64 → 128, done in 32-bit halves so nothing overflows on the way.
multiply_64 :: (a: u64, b: u64) -> Apollo_Time {
    a_low  := a & 0xffff_ffff;
    a_high := a >> 32;
    b_low  := b & 0xffff_ffff;
    b_high := b >> 32;

    low_low   := a_low * b_low;
    low_high  := a_low * b_high;
    high_low  := a_high * b_low;
    high_high := a_high * b_high;

    // The two middle products each straddle the 32-bit boundary; their low
    // halves join the bottom word and their high halves the top.
    middle := (low_low >> 32) + (low_high & 0xffff_ffff) + (high_low & 0xffff_ffff);

    result: Apollo_Time;
    result.low = (low_low & 0xffff_ffff) | (middle << 32);
    result.high = cast(s64) (high_high + (low_high >> 32) + (high_low >> 32) + (middle >> 32));
    return result;
}

// Shift-and-subtract long division of a non-negative 128-bit value. One pass
// over 128 bits: not the fastest way, but the one that is obviously right, and
// a time conversion is not on anyone's inner loop.
divide_128 :: (value: Apollo_Time, divisor: u64) -> Apollo_Time {
    quotient: Apollo_Time;
    if divisor == 0  return quotient;

    remainder: u64 = 0;
    bit := 127;
    while bit >= 0 {
        // The remainder cannot reach the top bit, because it is always less
        // than the divisor before the shift.
        remainder = (remainder << 1) | bit_at(value, bit);
        if remainder >= divisor {
            remainder -= divisor;
            set_bit(*quotient, bit);
        }
        bit -= 1;
    }
    return quotient;
}

bit_at :: (value: Apollo_Time, bit: s64) -> u64 {
    if bit >= 64  return (cast(u64) value.high >> cast(u64) (bit - 64)) & 1;
    return (value.low >> cast(u64) bit) & 1;
}

set_bit :: (value: *Apollo_Time, bit: s64) {
    if bit >= 64  value.high |= cast(s64) (cast(u64) 1 << cast(u64) (bit - 64));
    else          value.low  |= cast(u64) 1 << cast(u64) bit;
}
