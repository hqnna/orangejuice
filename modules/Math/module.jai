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

#scope_module

libm :: #system_library "m";

#scope_export

// ------------------------------------------------------------- constants ---

PI   :: 3.1415926535897932384626433832795;
TAU  :: 6.283185307179586476925286766559;
E    :: 2.7182818284590452353602874713527;

PI32  : float32 : 3.14159265;
TAU32 : float32 : 6.28318531;

FLOAT32_MIN :: 1.17549435e-38;
FLOAT32_MAX :: 3.40282347e+38;
FLOAT64_MIN :: 2.2250738585072014e-308;
FLOAT64_MAX :: 1.7976931348623157e+308;

S8_MIN  :: -128;
S8_MAX  ::  127;
S16_MIN :: -32768;
S16_MAX ::  32767;
S32_MIN :: -2147483648;
S32_MAX ::  2147483647;
S64_MIN :: -9223372036854775808;
S64_MAX ::  9223372036854775807;

U8_MAX  :: 255;
U16_MAX :: 65535;
U32_MAX :: 4294967295;
U64_MAX :: 18446744073709551615;

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

Vector2 :: struct { x: float32; y: float32; }
Vector3 :: struct { x: float32; y: float32; z: float32; }
Vector4 :: struct { x: float32; y: float32; z: float32; w: float32; }

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

length :: (v: Vector2) -> float32 { return sqrtf(length_squared(v)); }
length :: (v: Vector3) -> float32 { return sqrtf(length_squared(v)); }
length :: (v: Vector4) -> float32 { return sqrtf(length_squared(v)); }

distance :: (a: Vector2, b: Vector2) -> float32 { return length(b - a); }
distance :: (a: Vector3, b: Vector3) -> float32 { return length(b - a); }

// A zero-length vector normalizes to zero rather than to NaN, so a caller
// need not guard the degenerate case.
normalize :: (v: Vector2) -> Vector2 {
    len := length(v);
    if len == 0  return .{0, 0};
    return v / len;
}

normalize :: (v: Vector3) -> Vector3 {
    len := length(v);
    if len == 0  return .{0, 0, 0};
    return v / len;
}

lerp :: (a: Vector2, b: Vector2, t: float32) -> Vector2 { return a + (b - a) * t; }
lerp :: (a: Vector3, b: Vector3, t: float32) -> Vector3 { return a + (b - a) * t; }

// ----------------------------------------------------------- quaternions ---

// `w` is the scalar part; `xyz` the vector part. Unit quaternions represent
// rotations, and the ones here assume that.
Quaternion :: struct { x: float32; y: float32; z: float32; w: float32 = 1; }

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

normalize :: (q: Quaternion) -> Quaternion {
    len := sqrtf(q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w);
    if len == 0  return .{0, 0, 0, 1};
    return .{q.x / len, q.y / len, q.z / len, q.w / len};
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
