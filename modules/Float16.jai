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

Float16 :: #type,distinct u16;

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
