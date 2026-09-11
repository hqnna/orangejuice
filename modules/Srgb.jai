//
// sRGB and linear light.
//
// Colours on a screen are sRGB-encoded, and arithmetic on them — blending,
// averaging, lighting — is only correct in linear light. Averaging two sRGB
// values directly gives a result that is visibly too dark.
//
// Everything here works in [0, 1].
//

#import "Math";

// The exact transfer function the sRGB standard defines: a short linear
// segment near black, a power curve above it.
srgb_to_linear :: (value: float32) -> float32 {
    if value <= 0.04045  return value / 12.92;
    return cast(float32) pow(cast(float64) ((value + 0.055) / 1.055), 2.4);
}

linear_to_srgb :: (value: float32) -> float32 {
    if value <= 0.0031308  return value * 12.92;
    return cast(float32) (1.055 * pow(cast(float64) value, 1.0 / 2.4) - 0.055);
}

// The gamma-2.2 approximation. Cheaper and wrong near black, where the real
// curve is linear; fine for eyeballing, not for anything measured.
srgb_to_linear_approximate :: (value: float32) -> float32 {
    return cast(float32) pow(cast(float64) value, 2.2);
}

linear_to_srgb_approximate :: (value: float32) -> float32 {
    return cast(float32) pow(cast(float64) value, 1.0 / 2.2);
}

// Whole colours. Alpha is *not* converted: it is a coverage fraction, already
// linear, and putting it through the curve is a common and visible bug.
srgb_to_linear :: (colour: Vector4) -> Vector4 {
    return .{
        srgb_to_linear(colour.x),
        srgb_to_linear(colour.y),
        srgb_to_linear(colour.z),
        colour.w,
    };
}

linear_to_srgb :: (colour: Vector4) -> Vector4 {
    return .{
        linear_to_srgb(colour.x),
        linear_to_srgb(colour.y),
        linear_to_srgb(colour.z),
        colour.w,
    };
}

// A byte channel as sRGB, to linear light.
srgb_byte_to_linear :: (value: u8) -> float32 {
    return srgb_to_linear(cast(float32) value / 255.0);
}

linear_to_srgb_byte :: (value: float32) -> u8 {
    encoded := linear_to_srgb(clamp(value, cast(float32) 0, cast(float32) 1));
    return cast(u8) (encoded * 255.0 + 0.5);
}

// ------------------------------------------------------- the approximations ---
//
// The exact transfer function has a branch and a `pow`; these trade a little
// accuracy for none of that. `pow2` is the crudest — gamma 2.0 rather than
// 2.2 — `sqrt` its inverse, and `fast` a polynomial fit that is close enough
// for colour work.

srgb_to_linear_pow2 :: inline (x: float) -> float #must {
    return x * x;
}

linear_to_srgb_sqrt :: inline (x: float) -> float #must {
    return sqrtf(x);
}

srgb_to_linear_fast :: inline (x: float) -> float #must {
    return x * (x * (x * 0.30530601 + 0.68217111) + 0.012522878);
}

linear_to_srgb_fast :: inline (x: float) -> float #must {
    s1 := sqrtf(x);
    s2 := sqrtf(s1);
    s3 := sqrtf(s2);
    return 0.66200269 * s1 + 0.6841221 * s2 - 0.3235836 * s3 - 0.022541147 * x;
}

// The same, component by component. Alpha is left alone: it is not a colour.
srgb_to_linear_pow2 :: inline (c: Vector3) -> Vector3 #must {
    return .{srgb_to_linear_pow2(c.x), srgb_to_linear_pow2(c.y), srgb_to_linear_pow2(c.z)};
}

srgb_to_linear_pow2 :: inline (c: Vector4) -> Vector4 #must {
    return .{srgb_to_linear_pow2(c.x), srgb_to_linear_pow2(c.y), srgb_to_linear_pow2(c.z), c.w};
}

linear_to_srgb_sqrt :: inline (c: Vector3) -> Vector3 #must {
    return .{linear_to_srgb_sqrt(c.x), linear_to_srgb_sqrt(c.y), linear_to_srgb_sqrt(c.z)};
}

linear_to_srgb_sqrt :: inline (c: Vector4) -> Vector4 #must {
    return .{linear_to_srgb_sqrt(c.x), linear_to_srgb_sqrt(c.y), linear_to_srgb_sqrt(c.z), c.w};
}

srgb_to_linear_fast :: inline (c: Vector3) -> Vector3 #must {
    return .{srgb_to_linear_fast(c.x), srgb_to_linear_fast(c.y), srgb_to_linear_fast(c.z)};
}

srgb_to_linear_fast :: inline (c: Vector4) -> Vector4 #must {
    return .{srgb_to_linear_fast(c.x), srgb_to_linear_fast(c.y), srgb_to_linear_fast(c.z), c.w};
}

linear_to_srgb_fast :: inline (c: Vector3) -> Vector3 #must {
    return .{linear_to_srgb_fast(c.x), linear_to_srgb_fast(c.y), linear_to_srgb_fast(c.z)};
}

linear_to_srgb_fast :: inline (c: Vector4) -> Vector4 #must {
    return .{linear_to_srgb_fast(c.x), linear_to_srgb_fast(c.y), linear_to_srgb_fast(c.z), c.w};
}
