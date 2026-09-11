//
// 128-bit integers, built out of two 64-bit halves.
//
// The machine has no 128-bit type, so these are structs with the operators
// written out. The layout is little-endian in the halves — `low` first — which
// is what lets a pair of them be compared or added a word at a time.
//
// `Apollo_Time` is an `S128` under another name, which is why this lives in
// Basic rather than off in a module of its own: every program that measures
// time depends on it.
//

S128 :: struct {
    low:  u64;
    high: s64;
}

U128 :: struct {
    low:  u64;
    high: u64;
}

// What a division returns when it overflowed or the divisor was zero. There is
// no value a 128-bit division cannot legitimately produce, so this doubles as
// a sentinel only alongside the `overflow_or_divide_by_zero` flag.
U128_ERROR :: U128.{0xffff_ffff_ffff_ffff, 0xffff_ffff_ffff_ffff};
S128_ERROR :: S128.{0xffff_ffff_ffff_ffff, 0xffff_ffff_ffff_ffff};


// ----------------------------------------------------------- conversions ---

// Widens any integer. A signed one keeps its sign, so -1 becomes all ones; an
// unsigned one gets a zero top half.
to_s128 :: (a: $T) -> S128 {
    result: S128;
    #if type_info(T).type == .INTEGER && (cast(*Type_Info_Integer) type_info(T)).signed {
        wide := cast(s64) a;
        result.low = cast,no_check(u64) wide;
        result.high = ifx wide < 0 then -1 else 0;
    } else {
        result.low = cast,no_check(u64) a;
        result.high = 0;
    }
    return result;
}

to_u128 :: (a: $T) -> U128 {
    result: U128;
    #if type_info(T).type == .INTEGER && (cast(*Type_Info_Integer) type_info(T)).signed {
        wide := cast(s64) a;
        result.low = cast,no_check(u64) wide;
        result.high = ifx wide < 0 then U64_MAX else 0;
    } else {
        result.low = cast,no_check(u64) a;
        result.high = 0;
    }
    return result;
}

// 64 x 64 -> 128, done in 32-bit halves so nothing overflows on the way.
multiply_64bits_with_overflow :: (a: u64, b: u64) -> (high: u64, low: u64) {
    a_low  := a & 0xffff_ffff;
    a_high := a >> 32;
    b_low  := b & 0xffff_ffff;
    b_high := b >> 32;

    low_low   := a_low * b_low;
    low_high  := a_low * b_high;
    high_low  := a_high * b_low;
    high_high := a_high * b_high;

    // The two middle products straddle the 32-bit boundary: their low halves
    // join the bottom word and their high halves the top.
    middle := (low_low >> 32) + (low_high & 0xffff_ffff) + (high_low & 0xffff_ffff);

    low := (low_low & 0xffff_ffff) | (middle << 32);
    high := high_high + (low_high >> 32) + (high_low >> 32) + (middle >> 32);
    return high, low;
}

// ------------------------------------------------------------- comparing ---

operator == :: inline (a: S128, b: S128) -> bool {
    return a.low == b.low && a.high == b.high;
}

operator == :: inline (a: U128, b: U128) -> bool {
    return a.low == b.low && a.high == b.high;
}

operator ! :: inline (a: S128) -> bool {
    return a.low == 0 && a.high == 0;
}

operator ! :: inline (a: U128) -> bool {
    return a.low == 0 && a.high == 0;
}

// The top half decides unless the two agree, and then the bottom half does —
// unsigned in both cases, since only the *top* half carries a sign.
operator < :: (a: S128, b: S128) -> bool {
    if a.high != b.high  return a.high < b.high;
    return a.low < b.low;
}

operator < :: (a: U128, b: U128) -> bool {
    if a.high != b.high  return a.high < b.high;
    return a.low < b.low;
}

operator > :: (a: S128, b: S128) -> bool {
    if a.high != b.high  return a.high > b.high;
    return a.low > b.low;
}

operator > :: (a: U128, b: U128) -> bool {
    if a.high != b.high  return a.high > b.high;
    return a.low > b.low;
}

operator <= :: (a: S128, b: S128) -> bool { return !(a > b); }
operator >= :: (a: S128, b: S128) -> bool { return !(a < b); }
operator <= :: (a: U128, b: U128) -> bool { return !(a > b); }
operator >= :: (a: U128, b: U128) -> bool { return !(a < b); }

// ----------------------------------------------------------- adding ---

// One body would do for both signednesses; the carry out of the low half is
// exactly the case where the sum wrapped.
operator + :: inline (a: S128, b: S128) -> S128 {
    result: S128;
    result.low = a.low + b.low;
    result.high = a.high + b.high;
    if result.low < a.low  result.high += 1;
    return result;
}

operator + :: (a: U128, b: U128) -> U128 {
    result: U128;
    result.low = a.low + b.low;
    result.high = a.high + b.high;
    if result.low < a.low  result.high += 1;
    return result;
}

operator + :: (a: U128, b: u64) -> U128 {
    return a + U128.{b, 0};
}

operator - :: inline (a: S128, b: S128) -> S128 {
    result: S128;
    result.low = a.low - b.low;
    result.high = a.high - b.high;
    if a.low < b.low  result.high -= 1;
    return result;
}

operator - :: inline (a: U128, b: U128) -> U128 {
    result: U128;
    result.low = a.low - b.low;
    result.high = a.high - b.high;
    if a.low < b.low  result.high -= 1;
    return result;
}

operator - :: inline (a: S128) -> S128 {
    zero: S128;
    return zero - a;
}

// ------------------------------------------------------------- shifting ---

operator << :: (a: S128, $$x: u8) -> S128 {
    n := x & 127;
    result: S128;
    if n == 0  return a;
    if n >= 64 {
        result.low = 0;
        result.high = cast,no_check(s64) (a.low << (n - 64));
        return result;
    }
    result.low = a.low << n;
    // The bits leaving the low half arrive at the bottom of the high one.
    result.high = cast,no_check(s64) ((cast,no_check(u64) a.high << n) | (a.low >> (64 - n)));
    return result;
}

operator << :: inline (a: U128, $$x: u8) -> U128 {
    wide := (cast(*S128) *a).* << x;
    return (cast(*U128) *wide).*;
}

// Arithmetic: the sign bit is what fills in at the top.
operator >> :: (a: S128, $$x: u8) -> S128 {
    n := x & 127;
    result: S128;
    if n == 0  return a;

    fill: s64 = ifx a.high < 0 then -1 else 0;
    if n >= 64 {
        result.high = fill;
        result.low = cast,no_check(u64) (a.high >> (n - 64));
        return result;
    }
    result.high = a.high >> n;
    result.low = (a.low >> n) | (cast,no_check(u64) a.high << (64 - n));
    return result;
}

// Logical: zeroes fill in at the top.
operator >> :: (a: U128, $$x: u8) -> U128 {
    n := x & 127;
    result: U128;
    if n == 0  return a;
    if n >= 64 {
        result.high = 0;
        result.low = a.high >> (n - 64);
        return result;
    }
    result.high = a.high >> n;
    result.low = (a.low >> n) | (a.high << (64 - n));
    return result;
}

// A signed value shifted the unsigned way, for a caller that wants the bits
// rather than the number.
logical_shift_right :: (a: S128, $$x: u8) -> S128 {
    wide := (cast(*U128) *a).* >> x;
    return (cast(*S128) *wide).*;
}

// ----------------------------------------------------------- multiplying ---

operator * :: (_a: U128, _b: U128) -> U128 {
    high, low := multiply_64bits_with_overflow(_a.low, _b.low);
    result: U128;
    result.low = low;
    // The cross terms only reach the top half; whatever they carry past it is
    // gone, which is what wrapping at 128 bits means.
    result.high = high + _a.low * _b.high + _a.high * _b.low;
    return result;
}

operator * :: (_a: S128, _b: S128) -> S128 {
    a := (cast(*U128) *_a).*;
    b := (cast(*U128) *_b).*;
    wide := a * b;
    return (cast(*S128) *wide).*;
}

operator * :: (a: S128, factor: s64) -> S128 #symmetric {
    return a * to_s128(factor);
}

operator * :: (a: U128, b: u64) -> U128 {
    return a * U128.{b, 0};
}

operator * :: (a: U128, b: s64) -> U128 {
    return a * to_u128(b);
}

// ------------------------------------------------------------- dividing ---

operator / :: (dividend: U128, divisor: U128) -> U128 {
    quotient := unsigned_divide_with_remainder(dividend, divisor);
    return quotient;
}

operator / :: (dividend: S128, divisor: S128) -> S128 {
    quotient := signed_divide_with_remainder(dividend, divisor);
    return quotient;
}

operator / :: inline (a: S128, factor: s64) -> S128 {
    return a / to_s128(factor);
}

// Truncates toward zero, the way integer division does everywhere else, so
// the remainder takes the dividend's sign.
signed_divide_with_remainder :: inline (dividend: S128, divisor: S128) -> (quotient: S128, remainder: S128, overflow_or_divide_by_zero: bool) {
    negative_quotient := (dividend.high < 0) != (divisor.high < 0);
    negative_remainder := dividend.high < 0;

    a := ifx dividend.high < 0 then -dividend else dividend;
    b := ifx divisor.high < 0 then -divisor else divisor;

    q, r, bad := unsigned_divide_with_remainder((cast(*U128) *a).*, (cast(*U128) *b).*);
    if bad  return S128_ERROR, S128_ERROR, true;

    quotient := (cast(*S128) *q).*;
    remainder := (cast(*S128) *r).*;
    if negative_quotient   quotient = -quotient;
    if negative_remainder  remainder = -remainder;
    return quotient, remainder, false;
}

// Shift-and-subtract long division: one pass over 128 bits. Not the fastest
// way, but the one that is obviously right, and nothing here is on an inner
// loop.
unsigned_divide_with_remainder :: (u: U128, v: U128) -> (quotient: U128, remainder: U128, overflow_or_divide_by_zero: bool) {
    zero: U128;
    if v == zero  return U128_ERROR, U128_ERROR, true;

    quotient: U128;
    remainder: U128;

    bit := 127;
    while bit >= 0 {
        // The remainder is always below the divisor before the shift, so the
        // shift cannot lose a bit off the top.
        remainder = remainder << 1;
        remainder.low |= bit_at(u, bit);
        if remainder >= v {
            remainder = remainder - v;
            set_bit(*quotient, bit);
        }
        bit -= 1;
    }
    return quotient, remainder, false;
}

#scope_module

U64_MAX :: 0xFFFF_FFFF_FFFF_FFFF;

count_leading_zero_bits :: inline (x: u64) -> u8 {
    if x == 0  return 64;
    count: u8 = 0;
    value := x;
    while (value & 0x8000_0000_0000_0000) == 0 {
        value <<= 1;
        count += 1;
    }
    return count;
}

bit_at :: (value: U128, bit: s64) -> u64 {
    if bit >= 64  return (value.high >> cast(u64) (bit - 64)) & 1;
    return (value.low >> cast(u64) bit) & 1;
}

set_bit :: (value: *U128, bit: s64) {
    if bit >= 64  value.high |= cast(u64) 1 << cast(u64) (bit - 64);
    else          value.low  |= cast(u64) 1 << cast(u64) bit;
}

#scope_export
