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
