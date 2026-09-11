//
// float16: the IEEE 754 half-precision format.
//
// One sign bit, five exponent bits, ten of mantissa. About three decimal
// digits of precision and a range up to 65504 — enough for colours, normals
// and weights, and half the memory of a float32.
//
// Stored as a `u16` rather than a distinct float type, since the machine has
// no half-precision arithmetic to offer: convert, compute in float32, convert
// back.
//

#import "Math";
Float16 :: #type,isa u16;

// Handles subnormals, infinities and NaN rather than only the easy range.
float32_to_float16 :: (value: float32) -> Float16 {
    bits := (cast(*u32) *value).*;

    sign := (bits >> 16) & 0x8000;
    exponent := cast(s32) ((bits >> 23) & 0xff) - 127;
    mantissa := bits & 0x007f_ffff;

    // NaN and infinity keep their exponent; NaN must stay NaN rather than
    // become infinity, so a nonzero mantissa is preserved.
    if exponent == 128 {
        if mantissa  return cast(Float16) (sign | 0x7e00);
        return cast(Float16) (sign | 0x7c00);
    }

    // Too large for half precision: saturate to infinity.
    if exponent > 15  return cast(Float16) (sign | 0x7c00);

    // Small enough to need a subnormal, or to vanish.
    if exponent < -14 {
        if exponent < -24  return cast(Float16) sign;
        // Put the implicit leading 1 back and shift it down into place.
        // The half subnormal is value * 2^24, which for a float32 significand
        // S at exponent e is S >> (-e - 1).
        shifted := (mantissa | 0x0080_0000) >> cast(u32) (-exponent - 14);
        return cast(Float16) (sign | (shifted >> 13));
    }

    half_exponent := cast(u32) (exponent + 15) << 10;
    return cast(Float16) (sign | half_exponent | (mantissa >> 13));
}

float16_to_float32 :: (value: Float16) -> float32 {
    bits := cast(u32) value;

    sign := (bits & 0x8000) << 16;
    exponent := (bits >> 10) & 0x1f;
    mantissa := bits & 0x3ff;

    result: u32;
    if exponent == 0 {
        if mantissa == 0 {
            result = sign;
        } else {
            // Subnormal: normalise it by shifting until the leading bit is
            // implicit again, taking the exponent down as it goes.
            shift := 0;
            m := mantissa;
            while (m & 0x400) == 0 {
                m <<= 1;
                shift += 1;
            }
            m &= 0x3ff;
            // The leading bit sat at 10-shift, so the value is 2^(10-shift-24);
            // biased that is 127 - 14 - shift.
            e := cast(u32) (127 - 14 - shift);
            result = sign | (e << 23) | (m << 13);
        }
    } else if exponent == 31 {
        // Infinity, or NaN with its mantissa carried over.
        result = sign | 0x7f80_0000 | (mantissa << 13);
    } else {
        e := exponent + (127 - 15);
        result = sign | (e << 23) | (mantissa << 13);
    }
    return (cast(*float32) *result).*;
}

FLOAT16_MAX      :: 65504.0;
FLOAT16_MIN      :: 0.00006103515625;    // smallest normal
FLOAT16_EPSILON  :: 0.0009765625;        // 2^-10

// ------------------------------------------------------ the fast versions ---
//
// Fabian Giesen's conversions. They do the exponent rebias with an ordinary
// float multiply rather than by branching over the subnormal cases, which is
// what makes them short enough to vectorise. The results are the same as the
// exact conversions above, round-to-nearest included.

float_to_half_fast3 :: (_f: float32) -> Float16 {
    f32infty :: cast(u32) 255 << 23;
    f16infty :: cast(u32) 31 << 23;

    magic : float32 : 0h0780_0000;

    sign_mask  : u32 : 0x8000_0000;
    round_mask : u32 : 0xffff_f000;

    Bits :: union {
        f: float32;
        u: u32;
    }
    input: Bits;
    input.f = _f;

    sign := input.u & sign_mask;
    input.u ^= sign;

    o: Float16;
    if input.u >= f32infty {
        // A NaN becomes a quiet NaN; an infinity stays one.
        o = cast,no_check(Float16) (ifx input.u > f32infty then cast(u16) 0x7e00 else cast(u16) 0x7c00);
    } else {
        // The multiply renormalises whatever is left, subnormals included, and
        // the two round_mask steps are what round to nearest-even.
        input.u &= round_mask;
        input.f *= magic;
        input.u -= round_mask;
        if input.u > f16infty  input.u = f16infty;
        o = cast,no_check(Float16) (input.u >> 13);
    }

    o |= cast,no_check(Float16) (sign >> 16);
    return o;
}

half_to_float_fast4 :: (h: Float16) -> float32 {
    magic : float32 : 0h3880_0000;
    shifted_exp :: cast(u32) 0x7c00 << 13;

    Bits :: union {
        u: u32;
        f: float32;
    }
    o: Bits;
    bits := cast(u16) h;

    o.u = (cast(u32) (bits & 0x7fff)) << 13;
    exp := shifted_exp & o.u;
    o.u += (127 - 15) << 23;

    if exp == shifted_exp {
        // Infinity or NaN: the exponent needs the rest of the way.
        o.u += (128 - 16) << 23;
    } else if exp == 0 {
        // Zero or subnormal: bump the exponent and subtract the bias back out,
        // which renormalises without a loop.
        o.u += 1 << 23;
        o.f -= magic;
    }

    o.u |= (cast(u32) (bits & 0x8000)) << 16;
    return o.f;
}

// ------------------------------------------------------------ the packings ---
//
// A vector or a matrix of halves, for data that is uploaded rather than
// computed with.

Vector3_Float16 :: struct {
    x, y, z: Float16;
}

Matrix3_Float16 :: struct {
    _11, _12, _13: Float16;
    _21, _22, _23: Float16;
    _31, _32, _33: Float16;
}

vector3_float16 :: (v: Vector3) -> Vector3_Float16 {
    result: Vector3_Float16 = ---;
    result.x = float_to_half_fast3(v.x);
    result.y = float_to_half_fast3(v.y);
    result.z = float_to_half_fast3(v.z);
    return result;
}

vector3_float32 :: (v: Vector3_Float16) -> Vector3 {
    result: Vector3 = ---;
    result.x = half_to_float_fast4(v.x);
    result.y = half_to_float_fast4(v.y);
    result.z = half_to_float_fast4(v.z);
    return result;
}

matrix3_float16 :: (s: Matrix3) -> Matrix3_Float16 {
    result: Matrix3_Float16 = ---;
    halves := cast(*Float16) *result;
    for 0..8  halves[it] = float_to_half_fast3(s.floats[it]);
    return result;
}

matrix3_float32 :: (s: Matrix3_Float16) -> Matrix3 {
    result: Matrix3 = ---;
    source := s;
    halves := cast(*Float16) *source;
    for 0..8  result.floats[it] = half_to_float_fast4(halves[it]);
    return result;
}
