//
// Math: vectors, matrices, quaternions, and the transcendentals.
//
// `sin`, `exp`, `pow` and the rest are bound from the platform's libm rather
// than reimplemented. The reference ships a port of the cephes library for
// them; binding is both safer numerically — libm is tested against the
// hard-rounding cases nobody thinks of — and clean-room by construction.
//
// Everything else here is ordinary arithmetic, written out.
//

#load "matrix.jai";
#load "plane.jai";
#load "print.jai";

#scope_module

libm :: #library,system "libm";

// For Swap and the printers.
Basic :: #import "Basic";

#scope_export

// ------------------------------------------------------------- constants ---

PI   :: 3.1415926535897932384626433832795;
TAU  :: 6.283185307179586476925286766559;
E    :: 2.7182818284590452353602874713527;

PI64  :: 3.1415926535897932384626433832795;
TAU64 :: 6.283185307179586476925286766559;

SQRT2 :: 1.41421356237309504880168872420970;
// The square root of a half, which shows up wherever a unit vector is split
// evenly between two axes.
SQRTH :: 0.70710678118654752440084436210485;

PI32  : float32 : 3.14159265;
TAU32 : float32 : 6.28318531;

FLOAT32_MIN :: 1.17549435e-38;
FLOAT32_MAX :: 3.40282347e+38;
FLOAT64_MIN :: 2.2250738585072014e-308;
FLOAT64_MAX :: 1.7976931348623157e+308;

FLOAT16_MAX :: 65504.0;

// The bit patterns rather than expressions, so that nothing has to divide by
// zero at compile time to name them.
FLOAT32_INFINITY :: 0h7F800000;
FLOAT32_NAN      :: 0h7FC00000;
FLOAT64_INFINITY :: 0h7FF0000000000000;
FLOAT64_NAN      :: 0h7FF8000000000000;

// Each one carries the type it is the limit of, so that arithmetic mixing it
// with an untyped constant comes out in that type rather than in `int`. The
// signed minimums are written in hex: their magnitude is one past the
// corresponding maximum, so the decimal form would overflow the very type it
// is the limit of before the minus sign could be applied.
S8_MIN  :s8:  0x80;
S8_MAX  :s8:  0x7f;
S16_MIN :s16: 0x8000;
S16_MAX :s16: 0x7fff;
S32_MIN :s32: 0x8000_0000;
S32_MAX :s32: 0x7fff_ffff;
S64_MIN :s64: 0x8000_0000_0000_0000;
S64_MAX :s64: 0x7fff_ffff_ffff_ffff;

U8_MAX  :u8:  0xff;
U16_MAX :u16: 0xffff;
U32_MAX :u32: 0xffff_ffff;
U64_MAX :u64: 0xffff_ffff_ffff_ffff;

// ---------------------------------------------------- the transcendentals ---

sin   :: (x: float64) -> float64 #foreign libm;
cos   :: (x: float64) -> float64 #foreign libm;
tan   :: (x: float64) -> float64 #foreign libm;
asin  :: (x: float64) -> float64 #foreign libm;
acos  :: (x: float64) -> float64 #foreign libm;
atan  :: (x: float64) -> float64 #foreign libm;
atan2 :: (y: float64, x: float64) -> float64 #foreign libm;

sinh  :: (x: float64) -> float64 #foreign libm;
cosh  :: (x: float64) -> float64 #foreign libm;
tanh  :: (x: float64) -> float64 #foreign libm;

exp   :: (x: float64) -> float64 #foreign libm;
exp2  :: (x: float64) -> float64 #foreign libm;
log   :: (x: float64) -> float64 #foreign libm;
log2  :: (x: float64) -> float64 #foreign libm;
log10 :: (x: float64) -> float64 #foreign libm;
pow   :: (base: float64, exponent: float64) -> float64 #foreign libm;
sqrt  :: (x: float64) -> float64 #foreign libm;
cbrt  :: (x: float64) -> float64 #foreign libm;

floor :: (x: float64) -> float64 #foreign libm;
ceil  :: (x: float64) -> float64 #foreign libm;
round :: (x: float64) -> float64 #foreign libm;
trunc :: (x: float64) -> float64 #foreign libm;
fmod  :: (x: float64, y: float64) -> float64 #foreign libm;

// The float32 forms, so a caller working in single precision does not pay for
// a widen and a narrow around every call.
sinf   :: (x: float32) -> float32 #foreign libm "sinf";
cosf   :: (x: float32) -> float32 #foreign libm "cosf";
tanf   :: (x: float32) -> float32 #foreign libm "tanf";
sqrtf  :: (x: float32) -> float32 #foreign libm "sqrtf";
powf   :: (base: float32, exponent: float32) -> float32 #foreign libm "powf";
floorf :: (x: float32) -> float32 #foreign libm "floorf";
ceilf  :: (x: float32) -> float32 #foreign libm "ceilf";

// ------------------------------------------------------------ the basics ---

abs :: (x: float64) -> float64 { return ifx x < 0 then -x else x; }
abs :: (x: float32) -> float32 { return ifx x < 0 then -x else x; }
abs :: (x: s64) -> s64 { return ifx x < 0 then -x else x; }
abs :: (x: s32) -> s32 { return ifx x < 0 then -x else x; }

sign :: (x: float64) -> float64 {
    if x > 0  return  1;
    if x < 0  return -1;
    return 0;
}

sign :: (x: s64) -> s64 {
    if x > 0  return  1;
    if x < 0  return -1;
    return 0;
}

// Clamped to [low, high]. An inverted range is treated as the range it names.
clamp :: (value: $T, low: T, high: T) -> T {
    lo := low;
    hi := high;
    if hi < lo { t := lo; lo = hi; hi = t; }
    if value < lo  return lo;
    if value > hi  return hi;
    return value;
}

// Linear interpolation. `t` outside [0, 1] extrapolates.
lerp :: (a: $T, b: T, t: float64) -> T {
    return cast(T) (a + cast(T) ((b - a) * cast(T) t));
}

lerp :: (a: float64, b: float64, t: float64) -> float64 {
    return a + (b - a) * t;
}

lerp :: (a: float32, b: float32, t: float32) -> float32 {
    return a + (b - a) * t;
}

// Where `value` sits between `a` and `b`, the inverse of `lerp`.
inverse_lerp :: (a: float64, b: float64, value: float64) -> float64 {
    if a == b  return 0;
    return (value - a) / (b - a);
}

// Hermite interpolation with zero derivative at both ends, which is what
// makes a transition ease in and out rather than start and stop abruptly.
smoothstep :: (edge0: float64, edge1: float64, x: float64) -> float64 {
    t := clamp(inverse_lerp(edge0, edge1, x), 0.0, 1.0);
    return t * t * (3 - 2 * t);
}

to_radians :: (degrees: float64) -> float64 { return degrees * (PI / 180.0); }
to_degrees :: (radians: float64) -> float64 { return radians * (180.0 / PI); }

// Whether two floats are near enough, which is the only sensible way to
// compare them after arithmetic.
approximately_equal :: (a: float64, b: float64, epsilon := 1.0e-9) -> bool {
    difference := a - b;
    if difference < 0  difference = -difference;
    return difference <= epsilon;
}

// --------------------------------------------------------------- vectors ---

// ------------------------------------------------------------- the vectors ---
//
// The `#place` aliases are what let one value be read several ways without a
// cast: `v.xy` is the first two components of a `Vector3`, `v.component[i]`
// indexes them, and all of it is the same four or twelve bytes.

Vector2 :: struct {
    x, y: float;

#place x;
    component: [2] float = ---;
}

Vector3 :: struct {
    x, y, z: float;

#place x;
    xy: Vector2 = ---;
#place y;
    yz: Vector2 = ---;

#place x;
    component: [3] float = ---;
}

Vector4 :: struct {
    x, y, z, w: float;

#place x;
    xy: Vector2 = ---;
#place y;
    yz: Vector2 = ---;
#place z;
    zw: Vector2 = ---;
#place x;
    xyz: Vector3 = ---;
#place y;
    yzw: Vector3 = ---;

#place x;
    component: [4] float = ---;
}

Quaternion :: struct {
    x, y, z: float;
    w      := 1.0;

#place x;
    component: [4] float = ---;

#place x;
    xyz: Vector3 = ---;
}

// A complex number is a `Vector2` under another name, so the vector
// arithmetic applies to it unchanged.
Complex :: #type,isa Vector2;

AnyVector2 :: struct {
    x, y: float;
}

// The pieces of a bigger vector, by name.
xy :: inline (v: Vector3) -> Vector2 { return v.xy; }
xy :: inline (v: Vector4) -> Vector2 { return v.xy; }
xyz :: inline (v: Vector4) -> Vector3 { return v.xyz; }
xyzw :: inline (v: Vector4) -> Vector4 { return v; }

// The constructors, for a caller that would rather name the components than
// rely on a literal's order.
make_vector2 :: inline (x: float, y: float) -> Vector2 { return .{x, y}; }
make_vector3 :: inline (x: float, y: float, z: float) -> Vector3 { return .{x, y, z}; }
make_vector3 :: inline (v: Vector2, z: float) -> Vector3 { return .{v.x, v.y, z}; }
make_vector4 :: inline (x: float, y: float, z: float, w: float) -> Vector4 { return .{x, y, z, w}; }
make_vector4 :: inline (v: Vector3, w: float) -> Vector4 { return .{v.x, v.y, v.z, w}; }

// A colour is a vector too; these name the usual orders.
rgb  :: make_vector3;
rgba :: make_vector4;


operator + :: (a: Vector2, b: Vector2) -> Vector2 { return .{a.x + b.x, a.y + b.y}; }
operator - :: (a: Vector2, b: Vector2) -> Vector2 { return .{a.x - b.x, a.y - b.y}; }
operator * :: (a: Vector2, s: float32) -> Vector2 #symmetric { return .{a.x * s, a.y * s}; }
operator / :: (a: Vector2, s: float32) -> Vector2 { return .{a.x / s, a.y / s}; }
operator == :: (a: Vector2, b: Vector2) -> bool { return a.x == b.x && a.y == b.y; }

operator + :: (a: Vector3, b: Vector3) -> Vector3 { return .{a.x + b.x, a.y + b.y, a.z + b.z}; }
operator - :: (a: Vector3, b: Vector3) -> Vector3 { return .{a.x - b.x, a.y - b.y, a.z - b.z}; }
operator * :: (a: Vector3, s: float32) -> Vector3 #symmetric { return .{a.x * s, a.y * s, a.z * s}; }
operator / :: (a: Vector3, s: float32) -> Vector3 { return .{a.x / s, a.y / s, a.z / s}; }
operator == :: (a: Vector3, b: Vector3) -> bool { return a.x == b.x && a.y == b.y && a.z == b.z; }

operator + :: (a: Vector4, b: Vector4) -> Vector4 { return .{a.x + b.x, a.y + b.y, a.z + b.z, a.w + b.w}; }
operator - :: (a: Vector4, b: Vector4) -> Vector4 { return .{a.x - b.x, a.y - b.y, a.z - b.z, a.w - b.w}; }
operator * :: (a: Vector4, s: float32) -> Vector4 #symmetric { return .{a.x * s, a.y * s, a.z * s, a.w * s}; }
operator == :: (a: Vector4, b: Vector4) -> bool { return a.x == b.x && a.y == b.y && a.z == b.z && a.w == b.w; }

dot :: (a: Vector2, b: Vector2) -> float32 { return a.x * b.x + a.y * b.y; }
dot :: (a: Vector3, b: Vector3) -> float32 { return a.x * b.x + a.y * b.y + a.z * b.z; }
dot :: (a: Vector4, b: Vector4) -> float32 { return a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w; }
dot :: (a: Quaternion, b: Quaternion) -> float32 { return a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w; }

cross :: (a: Vector3, b: Vector3) -> Vector3 {
    return .{
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    };
}

// The signed area of the parallelogram the two span: positive when `b` is
// counter-clockwise from `a`.
cross :: (a: Vector2, b: Vector2) -> float32 {
    return a.x * b.y - a.y * b.x;
}

length_squared :: (v: Vector2) -> float32 { return dot(v, v); }
length_squared :: (v: Vector3) -> float32 { return dot(v, v); }
length_squared :: (v: Vector4) -> float32 { return dot(v, v); }
length_squared :: (q: Quaternion) -> float32 { return dot(q, q); }

length :: (v: Vector2) -> float32 { return sqrtf(length_squared(v)); }
length :: (q: Quaternion) -> float32 { return sqrtf(length_squared(q)); }
length :: (v: Vector3) -> float32 { return sqrtf(length_squared(v)); }
length :: (v: Vector4) -> float32 { return sqrtf(length_squared(v)); }

distance :: (a: Vector2, b: Vector2) -> float32 { return length(b - a); }
distance :: (a: Vector3, b: Vector3) -> float32 { return length(b - a); }

// A vector no longer than `epsilon` is replaced by `fallback` rather than
// divided into a NaN, so a caller need not guard the degenerate case. The
// length *before* normalizing comes back, which is usually what the caller
// wanted the vector's length for anyway.
normalize :: (v: *$T/MyVectors, epsilon := 0.0, fallback := T.{}) -> float {
    len := length(v.*);
    if len <= epsilon {
        v.* = fallback;
        return len;
    }

    factor := 1 / len;
    v.x *= factor;
    v.y *= factor;
    #if (T == Vector3) || (T == Vector4) || (T == Quaternion)  v.z *= factor;
    #if (T == Vector4) || (T == Quaternion)                    v.w *= factor;

    return len;
}

normalize :: (v: $T/MyVectors, epsilon := 0.0, fallback := T.{}) -> T {
    result := v;
    inline normalize(*result, epsilon, fallback);
    return result;
}

lerp :: (a: Vector2, b: Vector2, t: float32) -> Vector2 { return a + (b - a) * t; }
lerp :: (a: Vector3, b: Vector3, t: float32) -> Vector3 { return a + (b - a) * t; }

// ----------------------------------------------------------- quaternions ---

// `w` is the scalar part; `xyz` the vector part. Unit quaternions represent
// rotations, and the ones here assume that.

operator * :: (a: Quaternion, b: Quaternion) -> Quaternion {
    return .{
        a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
    };
}

// For a unit quaternion the conjugate is the inverse rotation.
conjugate :: (q: Quaternion) -> Quaternion {
    return .{-q.x, -q.y, -q.z, q.w};
}

// The rotation of `angle` radians about `axis`, which must be a unit vector.
quaternion_from_axis_angle :: (axis: Vector3, angle: float32) -> Quaternion {
    half := angle * 0.5;
    s := sinf(half);
    return .{axis.x * s, axis.y * s, axis.z * s, cosf(half)};
}

// Applies the rotation to a vector: q * v * conjugate(q), written out.
rotate :: (v: Vector3, q: Quaternion) -> Vector3 {
    u := Vector3.{q.x, q.y, q.z};
    s := q.w;
    return u * (2 * dot(u, v))
         + v * (s * s - dot(u, u))
         + cross(u, v) * (2 * s);
}

// ------------------------------------------------------- the float corners ---

// The IEEE predicates, written against the bits rather than against arithmetic
// that an optimizer is entitled to rearrange.
isnan :: (x: float64) -> bool {
    bits := (cast(*u64) *x).*;
    return (bits & 0x7ff0_0000_0000_0000) == 0x7ff0_0000_0000_0000 && (bits & 0x000f_ffff_ffff_ffff) != 0;
}

isinf :: (x: float64) -> bool {
    bits := (cast(*u64) *x).*;
    return (bits & 0x7fff_ffff_ffff_ffff) == 0x7ff0_0000_0000_0000;
}

is_finite :: (x: float64) -> bool {
    bits := (cast(*u64) *x).*;
    return (bits & 0x7ff0_0000_0000_0000) != 0x7ff0_0000_0000_0000;
}

is_nan_is_inf :: (x: float64) -> (isnan: bool, isinf: bool) {
    return isnan(x), isinf(x);
}

// `x * 2^pw2`, exactly, by adding to the exponent rather than multiplying.
ldexp :: (x: float64, pw2: s32) -> float64 #foreign libm;

// `x` split into a fraction in [0.5, 1) and the power of two that scales it.
frexp :: (x: float64) -> (result: float64, pow2: s32) {
    exponent: s32;
    fraction := frexp_c(x, *exponent);
    return fraction, exponent;
}

// Like `fmod`, but the answer keeps `y`'s sign rather than `x`'s — which is
// what an angle wrapped into [0, tau) wants.
fmod_cycling :: inline (x: float64, y: float64) -> float64 {
    result := fmod(x, y);
    if result != 0 && (result < 0) != (y < 0)  result += y;
    return result;
}

// The fractional part, keeping the sign.
frac :: inline (x: $T) -> T {
    return x - cast(T) trunc(cast(float64) x);
}

square :: inline (x: $T) -> T {
    return x * x;
}

saturate :: (f: float) -> float {
    return clamp(f, 0, 1);
}

saturate :: (f: float64) -> float64 {
    return clamp(f, 0, 1);
}

// Clamps in place, for a caller holding a pointer rather than a value.
Clamp :: inline (pointer: *$T, low: T, high: T) {
    if pointer.* < low   { pointer.* = low;  return; }
    if pointer.* > high  { pointer.* = high; return; }
}

cot :: (x: float64) -> float64 {
    return 1.0 / tan(x);
}

// The half-precision bit pattern widened to a `float`. Subnormals and the
// infinities are handled, so every one of the 65536 patterns round-trips.
half_to_float :: (h: u16) -> float {
    sign := (cast(u32) h & 0x8000) << 16;
    exponent := (cast(u32) h >> 10) & 0x1f;
    mantissa := cast(u32) h & 0x3ff;

    bits: u32;
    if exponent == 0 {
        if mantissa == 0 {
            bits = sign;
        } else {
            // A half's subnormal is a float's normal: shift the mantissa up
            // until its top bit falls off, counting as it goes.
            shift: u32 = 0;
            while (mantissa & 0x400) == 0 {
                mantissa <<= 1;
                shift += 1;
            }
            mantissa &= 0x3ff;
            bits = sign | ((127 - 15 - shift + 1) << 23) | (mantissa << 13);
        }
    } else if exponent == 0x1f {
        bits = sign | 0x7f80_0000 | (mantissa << 13);
    } else {
        bits = sign | ((exponent + 127 - 15) << 23) | (mantissa << 13);
    }
    return (cast(*float) *bits).*;
}

// ------------------------------------------------------------ the vectors ---

dot_product :: dot;
cross_product :: cross;

distance_squared :: inline (a: Vector2, b: Vector2) -> float { return length_squared(b - a); }
distance_squared :: inline (a: Vector3, b: Vector3) -> float { return length_squared(b - a); }
distance_squared :: inline (a: Vector4, b: Vector4) -> float { return length_squared(b - a); }

// The direction of `v`. A zero vector has no direction, and comes back as
// itself rather than as infinities.
unit_vector :: (v: $T) -> T {
    return normalize(v);
}

// A vector perpendicular to `v`, for building a basis around a known
// direction. The axis furthest from `v` is the one crossed with, so that the
// result never collapses.
make_reflector :: (v: Vector3) -> (u: Vector3) {
    axis: Vector3;
    if abs(v.x) <= abs(v.y) && abs(v.x) <= abs(v.z)       axis = .{1, 0, 0};
    else if abs(v.y) <= abs(v.z)                          axis = .{0, 1, 0};
    else                                                  axis = .{0, 0, 1};
    return unit_vector(cross(v, axis));
}

// Three mutually perpendicular unit vectors, the first of them `direction`.
make_an_orthonormal_basis :: (direction: Vector3) -> (a: Vector3, b: Vector3, c: Vector3) {
    a := unit_vector(direction);
    b := make_reflector(a);
    return a, b, cross(a, b);
}

// ------------------------------------------------------- the quaternions ---

// Interpolates and renormalizes. Cheaper than `slerp` and good enough when the
// two rotations are close; the angular speed is not constant.
nlerp :: inline (p0: Quaternion, p1: Quaternion, fraction: float) -> Quaternion {
    // The shorter way round: a quaternion and its negation are the same
    // rotation, so flipping one keeps the path from going the long way.
    end := p1;
    if dot(p0, p1) < 0  end = .{-p1.x, -p1.y, -p1.z, -p1.w};

    result: Quaternion = ---;
    result.x = p0.x + (end.x - p0.x) * fraction;
    result.y = p0.y + (end.y - p0.y) * fraction;
    result.z = p0.z + (end.z - p0.z) * fraction;
    result.w = p0.w + (end.w - p0.w) * fraction;
    return normalize(result);
}

// Constant angular speed, at the cost of two trigonometric calls. Falls back
// to `nlerp` when the two are nearly equal, where the division would lose all
// its precision.
slerp :: (start: Quaternion, end: Quaternion, t: float) -> Quaternion {
    target := end;
    cosine := dot(start, end);
    if cosine < 0 {
        target = .{-end.x, -end.y, -end.z, -end.w};
        cosine = -cosine;
    }

    if cosine > 0.9995  return nlerp(start, target, t);

    angle := acos(cast(float64) cosine);
    sine := sin(angle);
    a := cast(float) (sin((1.0 - cast(float64) t) * angle) / sine);
    b := cast(float) (sin(cast(float64) t * angle) / sine);

    result: Quaternion = ---;
    result.x = start.x * a + target.x * b;
    result.y = start.y * a + target.y * b;
    result.z = start.z * a + target.z * b;
    result.w = start.w * a + target.w * b;
    return result;
}

#scope_module

frexp_c :: (x: float64, exponent: *s32) -> float64 #foreign libm "frexp";

#scope_export

// ------------------------------------------------------ setting rotations ---

// Fills `q` with the rotation of `theta` radians about `v`. The axis is
// normalized first, so a caller need not have done it.
set_from_axis_and_angle :: (q: *Quaternion, v: Vector3, theta: float) {
    axis := unit_vector(v);
    half := cast(float64) theta * 0.5;
    s := cast(float) sin(half);

    q.x = axis.x * s;
    q.y = axis.y * s;
    q.z = axis.z * s;
    q.w = cast(float) cos(half);
}

set_from_axis_and_angle :: inline (q: *Quaternion, x: float, y: float, z: float, theta: float) {
    set_from_axis_and_angle(q, Vector3.{x, y, z}, theta);
}

// The reverse. A quaternion with no rotation in it has no axis either, and
// comes back as the x axis with an angle of zero.
get_axis_and_angle :: (q: Quaternion) -> (x: float64, y: float64, z: float64, theta: float64) {
    scale := sqrt(cast(float64) (q.x * q.x + q.y * q.y + q.z * q.z));
    if scale == 0  return 1, 0, 0, 0;

    theta := 2.0 * acos(cast(float64) clamp(q.w, -1, 1));
    return q.x / scale, q.y / scale, q.z / scale, theta;
}

// Normalizes in place, handing back the length it had. A quaternion of zero
// length has no direction, so it becomes the identity rather than infinities.
normalize_or_identity :: (q: *Quaternion) -> float {
    magnitude := sqrt(cast(float64) (q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w));
    if magnitude == 0 {
        q.* = .{0, 0, 0, 1};
        return 0;
    }
    inverse_magnitude := cast(float) (1.0 / magnitude);
    q.x *= inverse_magnitude;
    q.y *= inverse_magnitude;
    q.z *= inverse_magnitude;
    q.w *= inverse_magnitude;
    return cast(float) magnitude;
}

multiply :: (v: Vector2, f: float) -> Vector2 { return v * f; }
multiply :: (v: Vector3, f: float) -> Vector3 { return v * f; }
multiply :: (v: Vector4, f: float) -> Vector4 { return v * f; }

// Turns `v` by `theta` radians about the origin.
rotate :: (v: Vector2, theta: float) -> Vector2 {
    c := cast(float) cos(cast(float64) theta);
    s := cast(float) sin(cast(float64) theta);
    return .{v.x * c - v.y * s, v.x * s + v.y * c};
}

rotate :: (v: *Vector2, theta: float) {
    v.* = rotate(v.*, theta);
}

// The vector types `normalize` accepts, as a type restriction rather than as
// an overload each.
MyVectors :: Type.[Vector2, Vector3, Vector4, Quaternion];

// ----------------------------------------------------------- componentwise ---

min :: inline (v1: Vector2, v2: Vector2) -> Vector2 {
    return .{Basic.min(v1.x, v2.x), Basic.min(v1.y, v2.y)};
}

min :: inline (v1: Vector3, v2: Vector3) -> Vector3 {
    return .{Basic.min(v1.x, v2.x), Basic.min(v1.y, v2.y), Basic.min(v1.z, v2.z)};
}

max :: inline (v1: Vector2, v2: Vector2) -> Vector2 {
    return .{Basic.max(v1.x, v2.x), Basic.max(v1.y, v2.y)};
}

max :: inline (v1: Vector3, v2: Vector3) -> Vector3 {
    return .{Basic.max(v1.x, v2.x), Basic.max(v1.y, v2.y), Basic.max(v1.z, v2.z)};
}

// ------------------------------------------------- a rotation as a matrix ---

// Writes the rotation `q` names into the top-left 3x3 block, leaving whatever
// else the matrix holds — its translation column, say — alone.
set_rotation :: (using m: *$T/interface AnyMatrix3, q: Quaternion) {
    // Every product below carries one factor of two, so it is taken out of the
    // three components once rather than out of the nine results.
    xs := q.x * 2;
    ys := q.y * 2;
    zs := q.z * 2;

    wx := q.w * xs;
    wy := q.w * ys;
    wz := q.w * zs;

    _xx := q.x * xs;
    xy := q.x * ys;
    xz := q.x * zs;

    yy := q.y * ys;
    yz := q.y * zs;
    zz := q.z * zs;

    _11 = 1.0 - (yy + zz);
    _12 = xy - wz;
    _13 = xz + wy;

    _21 = xy + wz;
    _22 = 1.0 - (_xx + zz);
    _23 = yz - wx;

    _31 = xz - wy;
    _32 = yz + wx;
    _33 = 1.0 - (_xx + yy);
}

// The quaternion a rotation matrix stands for, by Shoemake's method: whichever
// of the four components is largest is recovered from the trace first, since
// dividing by a small one would lose most of the precision.
get_rotation :: (using m: $T/interface AnyMatrix3) -> Quaternion {
    q: Quaternion = ---;
    trace := _11 + _22 + _33;

    if trace > 0 {
        // |w| > 1/2, so w is the safe one to start from.
        s := sqrt(trace + 1);  // 2w

        q.w = cast(float) (s * 0.5);
        s = 0.5 / s;           // 1/(4w)

        q.x = cast(float) ((_32 - _23) * s);
        q.y = cast(float) ((_13 - _31) * s);
        q.z = cast(float) ((_21 - _12) * s);
    } else {
        // Otherwise the largest diagonal element names the component to
        // recover, and the other two follow from it.
        diagonal := float.[_11, _22, _33];
        i := 0;
        if diagonal[1] > diagonal[0]  i = 1;
        if diagonal[2] > diagonal[i]  i = 2;

        j := (1 << i) & 3;  // i + 1 modulo 3.
        k := (1 << j) & 3;

        coef := float.[_11, _12, _13, _21, _22, _23, _31, _32, _33];
        at :: inline (c: [9] float, row: int, column: int) -> float { return c[row * 3 + column]; }

        s := sqrt(at(coef, i, i) - at(coef, j, j) - at(coef, k, k) + 1);

        q.component[i] = cast(float) (s * 0.5);
        s = 0.5 / s;
        q.component[j] = cast(float) ((at(coef, i, j) + at(coef, j, i)) * s);
        q.component[k] = cast(float) ((at(coef, k, i) + at(coef, i, k)) * s);
        q.w            = cast(float) ((at(coef, k, j) - at(coef, j, k)) * s);
    }

    normalize_or_identity(*q);
    return q;
}

// ------------------------------------------------------- the cephes corners ---
//
// The constants and helpers the argument reduction of the trigonometric
// routines is written in. They are part of the module's surface because a
// program doing its own reduction wants the same numbers.

// Pi/4 split across three float64s, so that `x - y*DP1 - y*DP2 - y*DP3` keeps
// more bits than one subtraction of a rounded Pi/4 could.
DP1 :: 7.85398125648498535156E-1;
DP2 :: 3.77489470793079817668E-8;
DP3 :: 2.69515142907905952645E-15;

// Past this the argument has fewer bits left than the reduction needs, so
// there is no sensible answer to give.
LOSSTH : float64 : 1.073741824e9;

// How far outside [-1, 1] `asin`/`acos` tolerate an argument before saying so,
// rather than quietly returning a NaN for what is usually a rounded 1.
ASIN_AND_ACOS_ERROR_REPORTING_LIMIT :: 1.000001;

// The multiple of 1/16 nearest below `x` — the first step of the range
// reduction `pow` and `exp` do.
reduc :: (x: float64) -> float64 {
    t := ldexp(x, 4);
    t = floor(t);
    return ldexp(t, -4);
}

// The sine or the cosine, chosen at compile time, so that a caller writing a
// loop over both pays for one of them.
sin_or_cos :: (x: float64, $IS_SIN: bool) -> float64 {
    #if IS_SIN  return sin(x);
    else        return cos(x);
}
