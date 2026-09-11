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
// or the unit renegotiated. It is `S128` under another name, so all the
// arithmetic comes from there.
//
// The epoch is not Unix's: `APOLLO_TIME_FOR_JAN_1_1970` is the conversion.
//

Apollo_Time :: #type,isa S128;

// A value no real time can take, for a slot that has not been filled in.
APOLLO_TIME_INVALID :: Apollo_Time.{0, 0x8000_0000_0000_0000};

APOLLO_TIME_FOR_JAN_1_1970 :: Apollo_Time.{low = 0xdbd87f0edb7c8000, high = 0x300};

FEMTOSECONDS :: 1_000_000_000_000_000;

Time_Zone :: enum u32 {
    UTC   :: 0;
    LOCAL :: 1;  // Whatever the system's local time zone is.
}

// The members say where they start counting because the answer differs between
// systems, and a name that does not say invites the off-by-one.
Calendar_Time :: struct {
    year:                      s32;
    month_starting_at_0:       s8;  // 0-11, so it indexes a month-name array.
    day_of_month_starting_at_0: s8; // 0-30.
    day_of_week_starting_at_0: s8;  // Sunday first.
    hour:                      s8;
    minute:                    s8;
    second:                    s8;
    millisecond:               s16;

    time_zone: Time_Zone;
}

// ------------------------------------------------------------ conversions ---

ConvertToApollo :: (m: s64, FACTOR: s64) #expand {
    `return to_s128(m) * to_s128(FACTOR);
}

seconds_to_apollo :: (m: s64) -> Apollo_Time {
    ConvertToApollo(m, FEMTOSECONDS);
}

milliseconds_to_apollo :: (m: s64) -> Apollo_Time {
    ConvertToApollo(m, 1_000_000_000_000);
}

microseconds_to_apollo :: (m: s64) -> Apollo_Time {
    ConvertToApollo(m, 1_000_000_000);
}

nanoseconds_to_apollo :: (m: s64) -> Apollo_Time {
    ConvertToApollo(m, 1_000_000);
}

// How many whole `divisor`-sized units the time is. `success` is false when
// the answer does not fit in an `s64` — for nanoseconds that means a span
// longer than about 292 years.
//
// Without rounding this truncates toward zero, the way integer division does;
// with it, a remainder past half the divisor moves the result away from zero.
ConvertFromApollo :: inline (t: Apollo_Time, divisor: u64, $DO_ROUNDING: bool) -> (result: s64, success: bool) {
    quotient, remainder, overflow := signed_divide_with_remainder(t, S128.{divisor, 0});

    // A quotient that fits in an `s64` has a top half of all zeroes or all
    // ones; anything else has spilled into the high bits.
    failed := overflow || !(quotient.high == 0 || quotient.high == -1);

    result := cast,no_check(s64) quotient.low;

    #if DO_ROUNDING {
        // The remainder is below the divisor, so its own high half is zero.
        half := divisor / 2;
        if t.high >= 0 {
            if remainder.low > half || (divisor % 2 == 0 && remainder.low == half)  result += 1;
        } else {
            if remainder.low > half  result -= 1;
        }
    }

    // The divisor is unsigned, so a good result has the same sign as `t`; a
    // disagreement means the value overflowed on the way.
    failed = failed || (result != 0 && ((t.high < 0) != (result < 0)));
    return result, !failed;
}

to_seconds :: (t: Apollo_Time, $DO_ROUNDING := false) -> (result: s64, success: bool) {
    return ConvertFromApollo(t, FEMTOSECONDS, DO_ROUNDING);
}

to_milliseconds :: (t: Apollo_Time, $DO_ROUNDING := false) -> (result: s64, success: bool) {
    return ConvertFromApollo(t, 1_000_000_000_000, DO_ROUNDING);
}

to_microseconds :: (t: Apollo_Time, $DO_ROUNDING := false) -> (result: s64, success: bool) {
    return ConvertFromApollo(t, 1_000_000_000, DO_ROUNDING);
}

to_nanoseconds :: (t: Apollo_Time, $DO_ROUNDING := false) -> (result: s64, success: bool) {
    return ConvertFromApollo(t, 1_000_000, DO_ROUNDING);
}

// Seconds and the fraction of one, as a `float64`. Only about 15 significant
// digits survive, so this is for durations rather than for timestamps.
to_float64_seconds :: (input: Apollo_Time) -> (result: float64, success: bool) {
    a := input;
    if input.high < 0  a = -a;

    seconds, ok := to_seconds(a);
    if !ok  return 0, false;

    // What is left after the whole seconds is below one second, so it fits in
    // the low half on its own.
    femto := a - seconds_to_apollo(seconds);
    result := cast(float64) seconds + cast(float64) femto.low / cast(float64) FEMTOSECONDS;

    if input.high < 0  result = -result;
    return result, true;
}

// A fractional number of seconds, taken apart into its mantissa and exponent
// so that the femtosecond count is exact rather than rounded through another
// float.
to_apollo :: (seconds: float64) -> Apollo_Time {
    MANTISSA_BITS :: 52;
    EXPONENT_MASK :: 0x7ff0_0000_0000_0000;

    bits := (cast(*u64) *seconds).*;
    exponent := cast(s16) ((bits & EXPONENT_MASK) >> MANTISSA_BITS);
    exponent -= 1023;
    // The mantissa is read as a whole number rather than a fraction, so the
    // exponent shifts by its width too.
    exponent -= MANTISSA_BITS;

    MANTISSA_HIGH_BIT :: 1 << MANTISSA_BITS;
    mantissa := (bits & (MANTISSA_HIGH_BIT - 1)) | MANTISSA_HIGH_BIT;

    zero: Apollo_Time;
    if !mantissa  return zero;

    a := seconds_to_apollo(cast,no_check(s64) mantissa);

    shift := exponent;
    if shift < 0 {
        shift = -shift;
        if shift > 255  return zero;
        a = a >> cast(u8) shift;
    } else if shift {
        if shift > 255  return zero;
        a = a << cast(u8) shift;
    }

    if seconds < 0  a = -a;
    return a;
}

// -1, 0 or 1, for handing to a sort that wants a three-way comparison.
compare_apollo_times :: (a: Apollo_Time, b: Apollo_Time) -> s32 {
    if a < b   return -1;
    if a == b  return 0;
    return 1;
}

// -------------------------------------------------------------- the clocks ---

// The wall clock, which is what a timestamp written down and read back later
// must come from. It can step backwards when the machine's time is corrected.
current_time_consensus :: () -> Apollo_Time {
    ts: timespec;
    clock_gettime(CLOCK_REALTIME, *ts);
    return timespec_to_apollo(ts);
}

// A reading from an arbitrary origin that only ever goes forward, which is the
// one to measure an elapsed span with.
current_time_monotonic :: () -> Apollo_Time {
    ts: timespec;
    clock_gettime(CLOCK_MONOTONIC_RAW, *ts);
    return timespec_to_apollo(ts);
}

// How long the program has been running, in seconds.
seconds_since_init :: () -> float64 {
    now := current_time_monotonic();
    elapsed, ok := to_float64_seconds(now - program_start_time);
    if !ok  return 0;
    return elapsed;
}

timespec_to_apollo :: (ts: timespec) -> Apollo_Time {
    return seconds_to_apollo(ts.tv_sec) + nanoseconds_to_apollo(ts.tv_nsec) + APOLLO_TIME_FOR_JAN_1_1970;
}

// The reverse. A `timespec` cannot go negative, so a time before 1970 comes
// back with `success` false.
to_timespec :: (at: Apollo_Time) -> (ts: timespec, success: bool) {
    empty: timespec;
    a := at - APOLLO_TIME_FOR_JAN_1_1970;

    divided := a / FEMTOSECONDS;
    seconds: s64;
    if divided.high == 0 {
        // 63 bits rather than 64: the top one is the sign.
        if divided.low & 0x8000_0000_0000_0000  return empty, false;
        seconds = cast(s64) divided.low;
    } else if divided.high == -1 {
        seconds = cast,no_check(s64) divided.low;
    } else {
        return empty, false;
    }

    femto := a - seconds_to_apollo(seconds);
    nanoseconds := (cast(s64) femto.low + 500_000) / 1_000_000;

    ts: timespec;
    ts.tv_sec = seconds;
    ts.tv_nsec = nanoseconds;
    if seconds < 0  return ts, false;
    return ts, true;
}

// ------------------------------------------------------------- the calendar ---

to_calendar :: (at: Apollo_Time, time_zone := Time_Zone.UTC) -> Calendar_Time {
    empty: Calendar_Time;
    ts, ok := to_timespec(at);
    if !ok {
        log("Error: Input Apollo_Time was out of range when calling to_calendar(). We are not putting failure into the API though, because long-term in an OS-independent scenario we should not have this kind of failure case, maybe.\n");
        return empty;
    }

    NANO_PER_MILLI :: 1_000_000;
    milliseconds := cast,trunc(s16) ((ts.tv_nsec + NANO_PER_MILLI / 2) / NANO_PER_MILLI);
    if milliseconds >= 1000 {
        // Rounding can reach a full second, and carrying that here — before
        // the seconds are broken down — saves carrying it through minutes,
        // hours and days afterwards.
        milliseconds = 0;
        ts.tv_sec += 1;
    }

    parts: tm;
    seconds := ts.tv_sec;
    if time_zone == .LOCAL  localtime_r(*seconds, *parts);
    else                    gmtime_r(*seconds, *parts);

    result: Calendar_Time;
    result.year = parts.tm_year + 1900;
    result.month_starting_at_0 = cast,trunc(s8) parts.tm_mon;
    result.day_of_month_starting_at_0 = cast,trunc(s8) max(0, parts.tm_mday - 1);
    result.day_of_week_starting_at_0 = cast,trunc(s8) parts.tm_wday;
    result.hour = cast,trunc(s8) parts.tm_hour;
    result.minute = cast,trunc(s8) parts.tm_min;
    result.second = cast,trunc(s8) parts.tm_sec;
    result.millisecond = milliseconds;
    result.time_zone = time_zone;
    return result;
}

calendar_to_apollo :: (ct: Calendar_Time) -> Apollo_Time {
    empty: Apollo_Time;

    parts: tm;
    parts.tm_year = ct.year - 1900;
    parts.tm_mon = ct.month_starting_at_0;
    parts.tm_mday = ct.day_of_month_starting_at_0 + 1;
    parts.tm_hour = ct.hour;
    parts.tm_min = ct.minute;
    parts.tm_sec = ct.second;
    // Let the C library work out whether daylight saving was in effect.
    parts.tm_isdst = -1;

    seconds: time_t;
    if ct.time_zone == .LOCAL  seconds = timelocal(*parts);
    else                       seconds = timegm(*parts);
    if seconds == -1  return empty;

    return seconds_to_apollo(seconds) + milliseconds_to_apollo(ct.millisecond) + APOLLO_TIME_FOR_JAN_1_1970;
}

// "20 July 1969, 20:17:39", in temporary storage. English only for now.
calendar_to_string :: (ct: Calendar_Time) -> string {
    MONTH_NAMES :: string.["January", "February", "March", "April", "May", "June",
                           "July", "August", "September", "October", "November", "December"];

    month := clamp(cast(s64) ct.month_starting_at_0, 0, 11);
    two :: (x: Any) -> FormatInt { return formatInt(x, minimum_digits = 2); }

    return tprint("% % %, %:%:%", ct.day_of_month_starting_at_0 + 1, MONTH_NAMES[month], ct.year,
                  two(ct.hour), two(ct.minute), two(ct.second));
}

// "1969-07-20T20:17:39.000Z", in temporary storage.
calendar_to_iso_string :: (ct: Calendar_Time) -> string {
    two   :: (x: Any) -> FormatInt { return formatInt(x, minimum_digits = 2); }
    three :: (x: Any) -> FormatInt { return formatInt(x, minimum_digits = 3); }

    zone := ifx ct.time_zone == .UTC then "Z" else "";

    builder: String_Builder;
    builder.allocator = temp;
    print_to_builder(*builder, "%-%-%T%:%:%.%", ct.year, two(ct.month_starting_at_0 + 1),
                     two(ct.day_of_month_starting_at_0 + 1), two(ct.hour), two(ct.minute),
                     two(ct.second), three(ct.millisecond));
    append(*builder, zone);
    return builder_to_string(*builder,, allocator = temp);
}

#scope_module

// What `seconds_since_init` measures from. A global rather than something the
// runtime fills in, so that a program which never asks does not pay for it.
program_start_time := #run current_time_monotonic();

#scope_export
