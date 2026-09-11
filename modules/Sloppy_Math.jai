//
// Fast approximations, for when the exact answer is not worth its cycles.
//
// Everything here trades accuracy for speed and says how much. Nothing here
// belongs in code where the result is compared for equality, accumulated over
// many steps, or shown to anybody.
//

#import "Math";

DEFAULT_EPSILON :: 0.0001;

// Within about 0.001 over [-pi, pi]. Bhaskara's approximation, which is a
// rational function rather than a polynomial and stays well-behaved at the
// ends.
sloppy_sin :: (x: float32) -> float32 {
    // Fold into [-pi, pi] first; the approximation is only good there.
    angle := x;
    while angle >  PI32  angle -= TAU32;
    while angle < -PI32  angle += TAU32;

    negative := angle < 0;
    if negative  angle = -angle;

    numerator := 16.0 * angle * (PI32 - angle);
    denominator := 5.0 * PI32 * PI32 - 4.0 * angle * (PI32 - angle);
    result := numerator / denominator;
    return ifx negative then -result else result;
}

sloppy_cos :: (x: float32) -> float32 {
    return sloppy_sin(x + PI32 * 0.5);
}

// The classic reciprocal square root: one Newton step from a bit-pattern
// estimate. Within about 0.2%, and the constant is the one from Quake III.
sloppy_inverse_sqrt :: (x: float32) -> float32 {
    if x <= 0  return 0;
    half := x * 0.5;
    bits := (cast(*u32) *x).*;
    bits = 0x5f37_59df - (bits >> 1);
    guess := (cast(*float32) *bits).*;
    // One iteration; a second would roughly square the accuracy and cost as
    // much again.
    return guess * (1.5 - half * guess * guess);
}

sloppy_sqrt :: (x: float32) -> float32 {
    if x <= 0  return 0;
    return x * sloppy_inverse_sqrt(x);
}

// Within about 2%, by treating the exponent field as a logarithm.
sloppy_log2 :: (x: float32) -> float32 {
    if x <= 0  return 0;
    bits := (cast(*u32) *x).*;
    exponent := cast(s32) ((bits >> 23) & 0xff) - 127;
    // The mantissa, as a value in [1, 2).
    mantissa_bits := (bits & 0x007f_ffff) | 0x3f80_0000;
    mantissa := (cast(*float32) *mantissa_bits).*;
    // A linear fit of log2 over [1, 2).
    return cast(float32) exponent + (mantissa - 1.0);
}

// Within `epsilon` of each other. The default is loose on purpose: this module
// is for the cases where "close enough" is the right question.
values_are_close :: (f0: float, f1: float, epsilon := DEFAULT_EPSILON) -> bool {
    return abs(f0 - f1) <= epsilon;
}

is_approximately_zero :: (f: float, epsilon := DEFAULT_EPSILON) -> bool {
    return abs(f) <= epsilon;
}

// The extremes of a numeric type, by type rather than by name.
max_of_type :: inline ($T: Type) -> T {
    #if      T == bool    return true;
    else #if T == u8      return U8_MAX;
    else #if T == s8      return S8_MAX;
    else #if T == u16     return U16_MAX;
    else #if T == s16     return S16_MAX;
    else #if T == u32     return U32_MAX;
    else #if T == s32     return S32_MAX;
    else #if T == u64     return U64_MAX;
    else #if T == s64     return S64_MAX;
    else #if T == float   return FLOAT32_MAX;
    else #if T == float64 return FLOAT64_MAX;
    else #assert false "Unhandled type T in max_of_type.";
}

min_of_type :: inline ($T: Type) -> T {
    #if      T == bool    return false;
    else #if T == u8      return 0;
    else #if T == s8      return S8_MIN;
    else #if T == u16     return 0;
    else #if T == s16     return S16_MIN;
    else #if T == u32     return 0;
    else #if T == s32     return S32_MIN;
    else #if T == u64     return 0;
    else #if T == s64     return S64_MIN;
    else #if T == float   return -FLOAT32_MAX;
    else #if T == float64 return -FLOAT64_MAX;
    else #assert false "Unhandled type T in min_of_type.";
}
