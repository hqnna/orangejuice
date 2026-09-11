//
// Matrices, row-major.
//
// `m.coef[row][column]`, and a transform multiplies as `M * v` with the vector
// on the right — the convention most graphics maths is written in. A back end
// wanting column-major uploads the transpose.
//
// Each matrix names its elements `_11`.._nn and places three views over the
// same storage: `v` as row vectors, `coef` as a two-dimensional array, and
// `floats` as one flat run. They are the same bytes read three ways, which is
// what lets a matrix be indexed, destructured or uploaded without a copy.
//

Matrix2 :: struct {
    _11, _12: float;
    _21, _22: float;

    #place _11; v:      [2]    Vector2 = ---;
    #place _11; coef:   [2][2] float   = ---;
    #place _11; floats: [4]    float   = ---;

    Row_Type    :: Vector2;
    Column_Type :: Vector2;

    IsMatrixFromMathModule :: true;
}

Matrix2_Identity :: Matrix2.{_11 = 1, _22 = 1};

Matrix3 :: struct {
    _11, _12, _13: float;
    _21, _22, _23: float;
    _31, _32, _33: float;

    #place _11; v:      [3]    Vector3 = ---;
    #place _11; coef:   [3][3] float   = ---;
    #place _11; floats: [9]    float   = ---;

    Row_Type    :: Vector3;
    Column_Type :: Vector3;

    IsMatrixFromMathModule :: true;
}

Matrix3_Identity :: Matrix3.{_11 = 1, _22 = 1, _33 = 1};

Matrix4 :: struct {
    _11, _12, _13, _14: float;
    _21, _22, _23, _24: float;
    _31, _32, _33, _34: float;
    _41, _42, _43, _44: float;

    #place _11; mat4x3: Matrix4x3 = ---;

    #place _11; v:      [4]    Vector4 = ---;
    #place _11; coef:   [4][4] float   = ---;
    #place _11; floats: [16]   float   = ---;

    Row_Type    :: Vector4;
    Column_Type :: Vector4;

    IsMatrixFromMathModule :: true;
}

Matrix4_Identity :: Matrix4.{_11 = 1, _22 = 1, _33 = 1, _44 = 1};

// A `Matrix4` whose bottom row is known to be [0, 0, 0, 1] — the usual case
// in affine graphics. Dropping the row saves the storage and the multiplies
// that would only ever produce it again.
Matrix4x3 :: struct {
    _11, _12, _13, _14: float;
    _21, _22, _23, _24: float;
    _31, _32, _33, _34: float;

    #place _11; v:      [3]    Vector4 = ---;
    #place _11; coef:   [3][4] float   = ---;
    #place _11; floats: [12]   float   = ---;

    Row_Type    :: Vector4;
    Column_Type :: Vector3;

    IsMatrixFromMathModule :: true;
}

Matrix4x3_Identity :: Matrix4x3.{_11 = 1, _22 = 1, _33 = 1};

// The shapes that let one routine work on the top-left block of a Matrix3, a
// Matrix4 or a Matrix4x3 alike. They have no `coef`, so a routine needing one
// must take the real type.
AnyMatrix3 :: struct {
    _11, _12, _13: float;
    _21, _22, _23: float;
    _31, _32, _33: float;
}

AnyMatrix4x3 :: struct {
    _11, _12, _13, _14: float;
    _21, _22, _23, _24: float;
    _31, _32, _33, _34: float;
}

AnyMatrix2 :: struct {
    _11, _12: float;
    _21, _22: float;
}

AnyMatrix :: struct {
    IsMatrixFromMathModule :: true;
}

// The older spellings of the identity constants.
MATRIX3_IDENTITY :: Matrix3_Identity;
MATRIX4_IDENTITY :: Matrix4_Identity;

// ------------------------------------------------------- rows and columns ---

row :: (m: Matrix4, i: int) -> Vector4 { return m.v[i]; }
row :: (m: Matrix3, i: int) -> Vector3 { return m.v[i]; }
row :: (m: Matrix2, i: int) -> Vector2 { return m.v[i]; }
row :: (m: Matrix4x3, i: int) -> Vector4 { return m.v[i]; }

column :: (m: Matrix4, i: int) -> Vector4 {
    return .{m.coef[0][i], m.coef[1][i], m.coef[2][i], m.coef[3][i]};
}

column :: (m: Matrix3, i: int) -> Vector3 {
    return .{m.coef[0][i], m.coef[1][i], m.coef[2][i]};
}

column :: (m: Matrix2, i: int) -> Vector2 {
    return .{m.coef[0][i], m.coef[1][i]};
}

// A Matrix4x3's columns are three tall: the row it does not store is the one
// that would have contributed the fourth element.
column :: (m: Matrix4x3, i: int) -> Vector3 {
    return .{m.coef[0][i], m.coef[1][i], m.coef[2][i]};
}

make_matrix_from_rows :: (a: Vector3, b: Vector3, c: Vector3) -> Matrix3 {
    result: Matrix3 = ---;
    result.v[0] = a;
    result.v[1] = b;
    result.v[2] = c;
    return result;
}

make_matrix_from_rows :: (a: Vector4, b: Vector4, c: Vector4, d: Vector4) -> Matrix4 {
    result: Matrix4 = ---;
    result.v[0] = a;
    result.v[1] = b;
    result.v[2] = c;
    result.v[3] = d;
    return result;
}

make_matrix_from_columns :: (a: Vector3, b: Vector3, c: Vector3) -> Matrix3 {
    return transpose(make_matrix_from_rows(a, b, c));
}

make_matrix_from_columns :: (a: Vector4, b: Vector4, c: Vector4, d: Vector4) -> Matrix4 {
    return transpose(make_matrix_from_rows(a, b, c, d));
}

make_matrix3 :: (a: Vector3, b: Vector3, c: Vector3) -> Matrix3 {
    return make_matrix_from_rows(a, b, c);
}

make_matrix4 :: (a: Vector4, b: Vector4, c: Vector4, d: Vector4) -> Matrix4 {
    return make_matrix_from_rows(a, b, c, d);
}

identity :: ($T: Type) -> T {
    #if T == Matrix2  return Matrix2_Identity;
    #if T == Matrix3  return Matrix3_Identity;
    #if T == Matrix4  return Matrix4_Identity;
    #if T == Matrix4x3  return Matrix4x3_Identity;
    empty: T;
    return empty;
}

identity_of :: (m: $T) -> T {
    return identity(T);
}

operator * :: (a: Matrix4, b: Matrix4) -> Matrix4 {
    result: Matrix4;
    for row: 0..3 {
        for column: 0..3 {
            sum: float32 = 0;
            for k: 0..3  sum += a.coef[row][k] * b.coef[k][column];
            result.coef[row][column] = sum;
        }
    }
    return result;
}

operator * :: (a: Matrix3, b: Matrix3) -> Matrix3 {
    result: Matrix3;
    for row: 0..2 {
        for column: 0..2 {
            sum: float32 = 0;
            for k: 0..2  sum += a.coef[row][k] * b.coef[k][column];
            result.coef[row][column] = sum;
        }
    }
    return result;
}

// A point: the translation column applies, so `w` is taken as 1.
operator * :: (m: Matrix4, v: Vector4) -> Vector4 {
    return .{
        m.coef[0][0] * v.x + m.coef[0][1] * v.y + m.coef[0][2] * v.z + m.coef[0][3] * v.w,
        m.coef[1][0] * v.x + m.coef[1][1] * v.y + m.coef[1][2] * v.z + m.coef[1][3] * v.w,
        m.coef[2][0] * v.x + m.coef[2][1] * v.y + m.coef[2][2] * v.z + m.coef[2][3] * v.w,
        m.coef[3][0] * v.x + m.coef[3][1] * v.y + m.coef[3][2] * v.z + m.coef[3][3] * v.w,
    };
}

operator * :: (m: Matrix3, v: Vector3) -> Vector3 {
    return .{
        m.coef[0][0] * v.x + m.coef[0][1] * v.y + m.coef[0][2] * v.z,
        m.coef[1][0] * v.x + m.coef[1][1] * v.y + m.coef[1][2] * v.z,
        m.coef[2][0] * v.x + m.coef[2][1] * v.y + m.coef[2][2] * v.z,
    };
}

transpose :: (m: Matrix4) -> Matrix4 {
    result: Matrix4;
    for row: 0..3  for column: 0..3  result.coef[row][column] = m.coef[column][row];
    return result;
}

transpose :: (m: Matrix3) -> Matrix3 {
    result: Matrix3;
    for row: 0..2  for column: 0..2  result.coef[row][column] = m.coef[column][row];
    return result;
}

// ------------------------------------------------------------ transforms ---

make_translation_matrix4 :: (v: Vector3) -> Matrix4 {
    result: Matrix4;
    result.coef[0][3] = v.x;
    result.coef[1][3] = v.y;
    result.coef[2][3] = v.z;
    return result;
}

make_scale_matrix4 :: (v: Vector3) -> Matrix4 {
    result: Matrix4;
    result.coef[0][0] = v.x;
    result.coef[1][1] = v.y;
    result.coef[2][2] = v.z;
    return result;
}

// From a unit quaternion.
make_rotation_matrix4 :: (q: Quaternion) -> Matrix4 {
    result: Matrix4;
    x := q.x; y := q.y; z := q.z; w := q.w;

    result.coef[0][0] = 1 - 2 * (y * y + z * z);
    result.coef[0][1] =     2 * (x * y - z * w);
    result.coef[0][2] =     2 * (x * z + y * w);

    result.coef[1][0] =     2 * (x * y + z * w);
    result.coef[1][1] = 1 - 2 * (x * x + z * z);
    result.coef[1][2] =     2 * (y * z - x * w);

    result.coef[2][0] =     2 * (x * z - y * w);
    result.coef[2][1] =     2 * (y * z + x * w);
    result.coef[2][2] = 1 - 2 * (x * x + y * y);
    return result;
}

// Right-handed, looking down -Z, which is the OpenGL convention.
make_look_at_matrix4 :: (eye: Vector3, target: Vector3, up: Vector3) -> Matrix4 {
    forward := normalize(target - eye);
    right := normalize(cross(forward, up));
    true_up := cross(right, forward);

    result: Matrix4;
    result.coef[0][0] = right.x;
    result.coef[0][1] = right.y;
    result.coef[0][2] = right.z;
    result.coef[0][3] = -dot(right, eye);

    result.coef[1][0] = true_up.x;
    result.coef[1][1] = true_up.y;
    result.coef[1][2] = true_up.z;
    result.coef[1][3] = -dot(true_up, eye);

    result.coef[2][0] = -forward.x;
    result.coef[2][1] = -forward.y;
    result.coef[2][2] = -forward.z;
    result.coef[2][3] = dot(forward, eye);
    return result;
}

// `fov` is the vertical field of view in radians. Maps depth to [-1, 1].
make_projection_matrix4 :: (fov: float32, aspect: float32, near: float32, far: float32) -> Matrix4 {
    result: Matrix4;
    f := 1.0 / cast(float32) tan(cast(float64) fov * 0.5);

    result.coef[0][0] = f / aspect;
    result.coef[1][1] = f;
    result.coef[2][2] = (far + near) / (near - far);
    result.coef[2][3] = (2 * far * near) / (near - far);
    result.coef[3][2] = -1;
    result.coef[3][3] = 0;
    return result;
}

make_orthographic_matrix4 :: (left: float32, right: float32, bottom: float32, top: float32, near: float32, far: float32) -> Matrix4 {
    result: Matrix4;
    result.coef[0][0] = 2 / (right - left);
    result.coef[1][1] = 2 / (top - bottom);
    result.coef[2][2] = -2 / (far - near);
    result.coef[0][3] = -(right + left) / (right - left);
    result.coef[1][3] = -(top + bottom) / (top - bottom);
    result.coef[2][3] = -(far + near) / (far - near);
    return result;
}

// ------------------------------------------------------------- inversion ---

// The inverse of a matrix built from rotation and translation only: the
// rotation transposes and the translation is rotated back. Wrong for a matrix
// with scale or perspective in it, which is what `inverse` is for.
inverse_rigid :: (m: Matrix4) -> Matrix4 {
    result: Matrix4;
    for row: 0..2  for column: 0..2  result.coef[row][column] = m.coef[column][row];

    tx := m.coef[0][3];
    ty := m.coef[1][3];
    tz := m.coef[2][3];
    result.coef[0][3] = -(result.coef[0][0] * tx + result.coef[0][1] * ty + result.coef[0][2] * tz);
    result.coef[1][3] = -(result.coef[1][0] * tx + result.coef[1][1] * ty + result.coef[1][2] * tz);
    result.coef[2][3] = -(result.coef[2][0] * tx + result.coef[2][1] * ty + result.coef[2][2] * tz);
    return result;
}

// Cofactor expansion. `ok` is false for a singular matrix, in which case the
// result is the identity rather than a matrix full of infinities.
inverse :: (m: Matrix4) -> (result: Matrix4, ok: bool) {
    a := m.coef;

    s0 := a[0][0] * a[1][1] - a[1][0] * a[0][1];
    s1 := a[0][0] * a[1][2] - a[1][0] * a[0][2];
    s2 := a[0][0] * a[1][3] - a[1][0] * a[0][3];
    s3 := a[0][1] * a[1][2] - a[1][1] * a[0][2];
    s4 := a[0][1] * a[1][3] - a[1][1] * a[0][3];
    s5 := a[0][2] * a[1][3] - a[1][2] * a[0][3];

    c5 := a[2][2] * a[3][3] - a[3][2] * a[2][3];
    c4 := a[2][1] * a[3][3] - a[3][1] * a[2][3];
    c3 := a[2][1] * a[3][2] - a[3][1] * a[2][2];
    c2 := a[2][0] * a[3][3] - a[3][0] * a[2][3];
    c1 := a[2][0] * a[3][2] - a[3][0] * a[2][2];
    c0 := a[2][0] * a[3][1] - a[3][0] * a[2][1];

    determinant := s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0;
    if determinant == 0  return MATRIX4_IDENTITY, false;

    d := 1.0 / determinant;
    result: Matrix4;
    r := *result.coef;

    r.*[0][0] = ( a[1][1] * c5 - a[1][2] * c4 + a[1][3] * c3) * d;
    r.*[0][1] = (-a[0][1] * c5 + a[0][2] * c4 - a[0][3] * c3) * d;
    r.*[0][2] = ( a[3][1] * s5 - a[3][2] * s4 + a[3][3] * s3) * d;
    r.*[0][3] = (-a[2][1] * s5 + a[2][2] * s4 - a[2][3] * s3) * d;

    r.*[1][0] = (-a[1][0] * c5 + a[1][2] * c2 - a[1][3] * c1) * d;
    r.*[1][1] = ( a[0][0] * c5 - a[0][2] * c2 + a[0][3] * c1) * d;
    r.*[1][2] = (-a[3][0] * s5 + a[3][2] * s2 - a[3][3] * s1) * d;
    r.*[1][3] = ( a[2][0] * s5 - a[2][2] * s2 + a[2][3] * s1) * d;

    r.*[2][0] = ( a[1][0] * c4 - a[1][1] * c2 + a[1][3] * c0) * d;
    r.*[2][1] = (-a[0][0] * c4 + a[0][1] * c2 - a[0][3] * c0) * d;
    r.*[2][2] = ( a[3][0] * s4 - a[3][1] * s2 + a[3][3] * s0) * d;
    r.*[2][3] = (-a[2][0] * s4 + a[2][1] * s2 - a[2][3] * s0) * d;

    r.*[3][0] = (-a[1][0] * c3 + a[1][1] * c1 - a[1][2] * c0) * d;
    r.*[3][1] = ( a[0][0] * c3 - a[0][1] * c1 + a[0][2] * c0) * d;
    r.*[3][2] = (-a[3][0] * s3 + a[3][1] * s1 - a[3][2] * s0) * d;
    r.*[3][3] = ( a[2][0] * s3 - a[2][1] * s1 + a[2][2] * s0) * d;
    return result, true;
}

// --------------------------------------------------------- more transforms ---

// A perspective projection from a vertical field of view, in the OpenGL
// convention: the eye looks down -z and the depth range is [-1, 1] unless
// `depth_range_01` asks for the [0, 1] one.
make_projection_matrix :: (fov_vertical: float, aspect_ratio_horizontal_over_vertical: float, z_near: float, z_far: float, x_offset := 0.0, y_offset := 0.0, depth_range_01 := false) -> Matrix4 {
    result := Matrix4_Identity;

    cotangent := 1.0 / cast(float) tan(fov_vertical * 0.5);
    denominator := 1.0 / (z_far - z_near);

    result._11 = cotangent / aspect_ratio_horizontal_over_vertical;
    result._22 = cotangent;
    result._33 = -(z_far + z_near) * denominator;
    result._43 = -1;
    result._34 = -2 * z_far * z_near * denominator;
    result._44 = 0;

    result._13 = x_offset;
    result._23 = y_offset;

    if depth_range_01  remap_depth_to_01(*result);
    return result;
}

// The same projection written as the six planes of the frustum rather than as
// a field of view, which is what an off-centre or asymmetric one needs.
make_frustum_matrix :: (l: float, r: float, b: float, t: float, n: float, f: float, depth_range_01 := false) -> Matrix4 {
    twice_near := 2.0 * n;
    one_over_x := 1.0 / (r - l);
    one_over_y := 1.0 / (t - b);
    one_over_z := 1.0 / (f - n);

    result := Matrix4_Identity;
    result._11 = twice_near * one_over_x;
    result._22 = twice_near * one_over_y;
    result._13 = (r + l) * one_over_x;
    result._23 = (t + b) * one_over_y;
    result._33 = -(f + n) * one_over_z;
    result._43 = -1;
    result._34 = -f * twice_near * one_over_z;
    result._44 = 0;

    if depth_range_01  remap_depth_to_01(*result);
    return result;
}

orthographic_projection_matrix :: (left: float, right: float, bottom: float, top: float, near: float, far: float, depth_range_01 := false) -> Matrix4 {
    m: Matrix4;

    m._11 = 2.0 / (right - left);
    m._14 = -(right + left) / (right - left);

    m._22 = 2.0 / (top - bottom);
    m._24 = -(top + bottom) / (top - bottom);

    m._33 = -2.0 / (far - near);
    m._34 = -(far + near) / (far - near);

    m._44 = 1.0;

    if depth_range_01  remap_depth_to_01(*m);
    return m;
}

// The view matrix that puts the eye at `viewpoint` looking towards `look_at`.
// `x_is_forward` is the convention of this distribution — x down the view
// direction — where a graphics API usually wants -z, which is what passing
// false gives.
make_look_at_matrix :: (viewpoint: Vector3, look_at: Vector3, reference_up_vector: Vector3, $x_is_forward := true) -> Matrix4 {
    forward := look_at - viewpoint;
    normalize(*forward, fallback = .{z = 1});

    left := cross_product(reference_up_vector, forward);
    normalize(*left, fallback = .{z = 1});

    // Re-derived from the two that are already orthogonal, so the basis stays
    // square whatever the caller's `up` was.
    up := cross_product(forward, left);
    normalize(*up, fallback = .{z = 1});

    // A view matrix is the *inverse* of the camera's transform, and the
    // inverse of a rotation is its transpose: the basis vectors go in as rows.
    #if x_is_forward {
        rotation := make_matrix_from_rows(forward, left, up);
    } else {
        rotation := make_matrix_from_rows(left * -1, up, forward * -1);
    }

    return translate(rotation, -viewpoint);
}

// `m` followed by a translation of `t` in `m`'s own frame — the translation
// column takes the rotated offset, which is what makes this a right multiply.
translate :: (m: Matrix4, t: Vector3) -> Matrix4 #must {
    result := m;
    inline translate(*result, t);
    return result;
}

translate :: (m: *Matrix4, t: Vector3) {
    for row: 0..3 {
        m.coef[row][3] += m.coef[row][0] * t.x + m.coef[row][1] * t.y + m.coef[row][2] * t.z;
    }
}

scale :: (m: $T/interface AnyMatrix3, s: float) -> T #must {
    scale(*m, s);
    return m;
}

scale :: (m: *$T/interface AnyMatrix3, s: float) {
    m._11 *= s;  m._12 *= s;  m._13 *= s;
    m._21 *= s;  m._22 *= s;  m._23 *= s;
    m._31 *= s;  m._32 *= s;  m._33 *= s;
}

// A non-uniform scale: each *column* is scaled, so the basis vector that
// column stands for grows rather than the row.
scale :: (m: $T/interface AnyMatrix3, v: Vector3) -> T #must {
    scale(*m, v);
    return m;
}

scale :: (m: *$T/interface AnyMatrix3, v: Vector3) {
    m._11 *= v.x;  m._12 *= v.y;  m._13 *= v.z;
    m._21 *= v.x;  m._22 *= v.y;  m._23 *= v.z;
    m._31 *= v.x;  m._32 *= v.y;  m._33 *= v.z;
}

// The rotation a quaternion names, as whichever matrix type was asked for.
rotation_matrix :: ($T: Type, q: Quaternion) -> T {
    m: T;
    #if T == Matrix4  m._44 = 1;
    set_rotation(*m, q);
    return m;
}

// A point goes through the translation column as well as the rotation, which
// is what tells it apart from a direction.
transform_point :: (m: $T/interface AnyMatrix4x3, v: Vector3) -> Vector3 {
    result: Vector3 = ---;
    result.x = m._11 * v.x + m._12 * v.y + m._13 * v.z + m._14;
    result.y = m._21 * v.x + m._22 * v.y + m._23 * v.z + m._24;
    result.z = m._31 * v.x + m._32 * v.y + m._33 * v.z + m._34;
    return result;
}

transform_vector :: (m: $T/interface AnyMatrix3, v: Vector3) -> Vector3 {
    result: Vector3 = ---;
    result.x = m._11 * v.x + m._12 * v.y + m._13 * v.z;
    result.y = m._21 * v.x + m._22 * v.y + m._23 * v.z;
    result.z = m._31 * v.x + m._32 * v.y + m._33 * v.z;
    return result;
}

// ----------------------------------------------------- determinant, inverse ---

// Lengyel's formulation: the matrix read as four column vectors and the four
// numbers of its bottom row, which shares most of its work with the adjugate
// below.
determinant :: (m: Matrix4) -> float #must {
    a, b, c, d, x, y, z, w := lengyel_parts(m);

    s := cross(a, b);
    t := cross(c, d);
    u := a * y - b * x;
    v := c * w - d * z;

    return dot(s, v) + dot(t, u);
}

// The transpose of the cofactor matrix — the inverse without the divide, so it
// is finite even for a singular matrix.
adjugate_lengyel :: (m: Matrix4) -> Matrix4 #must {
    a, b, c, d, x, y, z, w := lengyel_parts(m);

    s := cross(a, b);
    t := cross(c, d);
    u := a * y - b * x;
    v := c * w - d * z;

    return lengyel_assemble(a, b, c, d, s, t, u, v, x, y, z, w);
}

adjugate :: adjugate_lengyel;

// The inverse, and whether the matrix was singular enough that the result is
// not to be trusted.
inverse_lengyel :: (m: Matrix4, epsilon := 0.0001) -> Matrix4 #must, bool {
    a, b, c, d, x, y, z, w := lengyel_parts(m);

    s := cross(a, b);
    t := cross(c, d);
    u := a * y - b * x;
    v := c * w - d * z;

    det := dot(s, v) + dot(t, u);
    // The divide is folded into the four partial products rather than applied
    // to the sixteen results.
    inverse_determinant := 1.0 / det;
    s *= inverse_determinant;
    t *= inverse_determinant;
    u *= inverse_determinant;
    v *= inverse_determinant;

    return lengyel_assemble(a, b, c, d, s, t, u, v, x, y, z, w), abs(det) < epsilon;
}

// The same inverse by row reduction, which needs no formula and is what a
// matrix whose structure is unknown is safest through. A column with no
// non-zero pivot leaves whatever was reduced so far.
inverse_gaussian_elimination :: (m: Matrix4) -> Matrix4 #must {
    a := m;
    b := Matrix4_Identity;

    for i: 0..3 {
        // Partial pivoting: the largest candidate in the column, which is what
        // keeps the division from amplifying rounding error.
        pivot_row := i;
        best := -1.0;
        for k: i..3 {
            if abs(a.coef[k][i]) > best {
                best = abs(a.coef[k][i]);
                pivot_row = k;
            }
        }
        if best <= 0  return b;

        if pivot_row != i {
            for k: i..3  Basic.Swap(*a.coef[i][k], *a.coef[pivot_row][k]);
            for k: 0..3  Basic.Swap(*b.coef[i][k], *b.coef[pivot_row][k]);
        }

        pivot := a.coef[i][i];
        // The elements left of the pivot are already zero, so only what is to
        // its right needs scaling.
        for k: i + 1..3  a.coef[i][k] /= pivot;
        for k: 0..3      b.coef[i][k] /= pivot;

        for j: i + 1..3 {
            factor := a.coef[j][i];
            for k: i + 1..3  a.coef[j][k] -= a.coef[i][k] * factor;
            for k: 0..3      b.coef[j][k] -= b.coef[i][k] * factor;
        }
    }

    // Back substitution. `a` is upper triangular with a unit diagonal by now,
    // so only `b` has to be touched.
    for #v2 < i: 1..3 {
        for j: 0..i - 1 {
            factor := a.coef[j][i];
            for k: 0..3  b.coef[j][k] -= b.coef[i][k] * factor;
        }
    }

    return b;
}

// The inverse of a matrix that is a rotation and a translation and nothing
// else: the rotation transposes and the translation is rotated back.
isometry_inverse :: (m: Matrix4) -> Matrix4 #must {
    result := Matrix4_Identity;
    for i: 0..2  for j: 0..2  result.coef[i][j] = m.coef[j][i];

    t := Vector3.{-m.coef[0][3], -m.coef[1][3], -m.coef[2][3]};
    // The bottom row is known to be [0, 0, 0, 1], so the fourth row of the
    // translate below would do nothing and is left out.
    for i: 0..2 {
        result.coef[i][3] = result.coef[i][0] * t.x + result.coef[i][1] * t.y + result.coef[i][2] * t.z;
    }
    return result;
}

// The transposed adjugate of a 3x3, which is just the three cross products of
// its rows — what a normal is transformed by.
adjoint_transpose :: (m: Matrix3) -> Matrix3 {
    r: Matrix3 = ---;
    r.v[0] = cross(m.v[1], m.v[2]);
    r.v[1] = cross(m.v[2], m.v[0]);
    r.v[2] = cross(m.v[0], m.v[1]);
    return r;
}

// ----------------------------------------------------- norms and reflectors ---

// The largest absolute row sum, or column sum when `tpose` — the operator
// 1-norm and the infinity-norm, which are each other's transpose.
mat_norm :: (m: Matrix3, tpose: bool) -> float {
    largest := 0.0;
    for i: 0..2 {
        sum: float = ---;
        if tpose  sum = abs(m.coef[0][i]) + abs(m.coef[1][i]) + abs(m.coef[2][i]);
        else      sum = abs(m.coef[i][0]) + abs(m.coef[i][1]) + abs(m.coef[i][2]);
        if largest < sum  largest = sum;
    }
    return largest;
}

norm_inf :: #bake_arguments mat_norm(tpose = false);
norm_one :: #bake_arguments mat_norm(tpose = true);

// Applies the Householder reflection `u` names to the columns of `m`.
reflect_cols :: (m: *Matrix3, u: Vector3) {
    for i: 0..2 {
        s := u.x * m.coef[0][i] + u.y * m.coef[1][i] + u.z * m.coef[2][i];
        for j: 0..2  m.coef[j][i] -= u.component[j] * s;
    }
}

reflect_rows :: (m: *Matrix3, u: Vector3) {
    for i: 0..2 {
        s := dot_product(u, m.v[i]);
        for j: 0..2  m.coef[i][j] -= u.component[j] * s;
    }
}

// Splits a matrix into a rotation and a symmetric stretch, `M = Q * S`, by
// averaging the matrix with its inverse transpose until the two agree — the
// Higham iteration. `det` comes back too, since a caller usually wants to know
// whether the rotation flipped.
polar_decomp :: (m: Matrix3) -> (Q: Matrix3, S: Matrix3, det: float) {
    TOLERANCE :: 1.0e-6;

    mk := transpose(m);

    det: float;
    error := FLOAT32_MAX;
    m_one := norm_one(mk);
    m_inf := norm_inf(mk);

    while error > m_one * TOLERANCE {
        adjugate_transposed := adjoint_transpose(mk);

        det = dot(mk.v[0], adjugate_transposed.v[0]);
        if det == 0.0 {
            mk = polar_decomp_rank2(*mk, adjugate_transposed);
            break;
        }

        adj_one := norm_one(adjugate_transposed);
        adj_inf := norm_inf(adjugate_transposed);

        // The weight that makes the two halves of the average the same size,
        // which is what gives the iteration its quadratic convergence.
        gamma := sqrt(sqrt((adj_one * adj_inf) / (m_one * m_inf)) / abs(det));
        g1 := cast(float) (gamma * 0.5);
        g2 := cast(float) (0.5 / (gamma * det));

        previous := mk;
        mk = g1 * mk + g2 * adjugate_transposed;
        previous -= mk;

        error = norm_one(previous);
        m_one = norm_one(mk);
        m_inf = norm_inf(mk);
    }

    q := transpose(mk);
    s := mk * m;

    // The stretch is symmetric in exact arithmetic; averaging across the
    // diagonal removes what rounding left.
    for i: 0..2  for j: 0..2 {
        s.coef[i][j] = 0.5 * (s.coef[i][j] + s.coef[j][i]);
        s.coef[j][i] = s.coef[i][j];
    }

    return q, s, det;
}

#scope_module

// The rows of the matrix as three columns and the numbers of its bottom row,
// which is the shape Lengyel's determinant, adjugate and inverse all work in.
lengyel_parts :: (m: Matrix4) -> (a: Vector3, b: Vector3, c: Vector3, d: Vector3, x: float, y: float, z: float, w: float) {
    a := Vector3.{m._11, m._21, m._31};
    b := Vector3.{m._12, m._22, m._32};
    c := Vector3.{m._13, m._23, m._33};
    d := Vector3.{m._14, m._24, m._34};
    return a, b, c, d, m._41, m._42, m._43, m._44;
}

lengyel_assemble :: (a: Vector3, b: Vector3, c: Vector3, d: Vector3, s: Vector3, t: Vector3, u: Vector3, v: Vector3, x: float, y: float, z: float, w: float) -> Matrix4 {
    r0 := cross(b, v) + t * y;
    r1 := cross(v, a) - t * x;
    r2 := cross(d, u) + s * w;
    r3 := cross(u, c) - s * z;

    return .{
        r0.x, r0.y, r0.z, -dot(b, t),
        r1.x, r1.y, r1.z,  dot(a, t),
        r2.x, r2.y, r2.z, -dot(d, s),
        r3.x, r3.y, r3.z,  dot(c, s),
    };
}

// A depth range of [-1, 1] becomes [0, 1] by halving the z row and adding half
// the w row, which is the transform z' = z * 0.5 + 0.5 written on the matrix.
remap_depth_to_01 :: (m: *Matrix4) {
    m._33 = m._33 * 0.5 + m._43 * 0.5;
    m._34 = m._34 * 0.5 + m._44 * 0.5;
}

// The rank-2 fallback of `polar_decomp`: a singular matrix has no inverse to
// average with, so the rotation is built out of two Householder reflections
// and the 2x2 rotation left between them.
polar_decomp_rank2 :: (m: *Matrix3, adjugate_transposed: Matrix3) -> Matrix3 {
    q: Matrix3;

    // Any non-zero column of the transposed adjugate spans the kernel's
    // complement; the first will do.
    v1 := Vector3.{adjugate_transposed.coef[0][0], adjugate_transposed.coef[1][0], adjugate_transposed.coef[2][0]};
    v1 = make_reflector(v1);
    reflect_cols(m, v1);

    v2 := cross(m.v[0], m.v[1]);
    v2 = make_reflector(v2);
    reflect_rows(m, v2);

    w := m.coef[0][0];
    x := m.coef[0][1];
    y := m.coef[1][0];
    z := m.coef[1][1];

    if w * z > x * y {
        c := z + w;
        s := y - x;
        d := sqrt(c * c + s * s);
        c /= d;
        s /= d;
        q.coef[0][0] = c;
        q.coef[1][1] = c;
        q.coef[0][1] = -s;
        q.coef[1][0] = s;
    } else {
        c := z - w;
        s := y + x;
        d := sqrt(c * c + s * s);
        c /= d;
        s /= d;
        q.coef[0][0] = -c;
        q.coef[1][1] = c;
        q.coef[0][1] = s;
        q.coef[1][0] = s;
    }

    q.coef[2][2] = 1.0;

    reflect_cols(*q, v1);
    reflect_rows(*q, v2);
    return q;
}

#scope_export

// ------------------------------------------------ multiply, in every shape ---
//
// A matrix-vector product cannot be written once for every matrix, since
// `Matrix4x3 * Vector4` produces a `Vector3` where the others produce a vector
// of their own width, so each shape is spelled out.

multiply :: (m: Matrix4, v: Vector4) -> Vector4 {
    result: Vector4 = ---;
    result.x = m._11 * v.x + m._12 * v.y + m._13 * v.z + m._14 * v.w;
    result.y = m._21 * v.x + m._22 * v.y + m._23 * v.z + m._24 * v.w;
    result.z = m._31 * v.x + m._32 * v.y + m._33 * v.z + m._34 * v.w;
    result.w = m._41 * v.x + m._42 * v.y + m._43 * v.z + m._44 * v.w;
    return result;
}

multiply :: (m: Matrix4x3, v: Vector4) -> Vector3 {
    result: Vector3 = ---;
    result.x = m._11 * v.x + m._12 * v.y + m._13 * v.z + m._14 * v.w;
    result.y = m._21 * v.x + m._22 * v.y + m._23 * v.z + m._24 * v.w;
    result.z = m._31 * v.x + m._32 * v.y + m._33 * v.z + m._34 * v.w;
    return result;
}

multiply :: (m: Matrix2, v: Vector2) -> Vector2 {
    result: Vector2 = ---;
    result.x = m._11 * v.x + m._12 * v.y;
    result.y = m._21 * v.x + m._22 * v.y;
    return result;
}

// The top-left 3x3 block of whatever it was given, which is what makes one
// routine serve Matrix3, Matrix4 and Matrix4x3 alike.
multiply :: (m: $T/interface AnyMatrix3, v: Vector3) -> Vector3 {
    result: Vector3 = ---;
    result.x = m._11 * v.x + m._12 * v.y + m._13 * v.z;
    result.y = m._21 * v.x + m._22 * v.y + m._23 * v.z;
    result.z = m._31 * v.x + m._32 * v.y + m._33 * v.z;
    return result;
}

multiply :: (a: Matrix2, b: Matrix2) -> Matrix2 {
    result: Matrix2;
    for row: 0..1  for column: 0..1 {
        sum: float;
        for k: 0..1  sum += a.coef[row][k] * b.coef[k][column];
        result.coef[row][column] = sum;
    }
    return result;
}

multiply :: (a: Matrix3, b: Matrix3) -> Matrix3 {
    result: Matrix3;
    for row: 0..2  for column: 0..2 {
        sum: float;
        for k: 0..2  sum += a.coef[row][k] * b.coef[k][column];
        result.coef[row][column] = sum;
    }
    return result;
}

multiply :: (a: Matrix4, b: Matrix4) -> Matrix4 {
    result: Matrix4;
    for row: 0..3  for column: 0..3 {
        sum: float;
        for k: 0..3  sum += a.coef[row][k] * b.coef[k][column];
        result.coef[row][column] = sum;
    }
    return result;
}

// A `Matrix4x3` is a `Matrix4` whose bottom row is [0, 0, 0, 1], so the
// products below are the full ones with that row substituted in rather than
// multiplied out.
multiply :: (a: Matrix4x3, b: Matrix4x3) -> Matrix4x3 {
    result: Matrix4x3;
    for row: 0..2  for column: 0..3 {
        sum: float;
        for k: 0..2  sum += a.coef[row][k] * b.coef[k][column];
        if column == 3  sum += a.coef[row][3];
        result.coef[row][column] = sum;
    }
    return result;
}

multiply :: (a: Matrix4, b: Matrix4x3) -> Matrix4 {
    result: Matrix4;
    for row: 0..3  for column: 0..3 {
        sum: float;
        for k: 0..2  sum += a.coef[row][k] * b.coef[k][column];
        if column == 3  sum += a.coef[row][3];
        result.coef[row][column] = sum;
    }
    return result;
}

multiply :: (a: Matrix4x3, b: Matrix4) -> Matrix4 {
    result := Matrix4_Identity;
    for row: 0..2  for column: 0..3 {
        sum: float;
        for k: 0..3  sum += a.coef[row][k] * b.coef[k][column];
        result.coef[row][column] = sum;
    }
    return result;
}

// The 3x3 stands for the 4x4 that leaves the translation column alone.
multiply :: (a: Matrix4, b: Matrix3) -> Matrix4 {
    result: Matrix4;
    for row: 0..3  for column: 0..3 {
        sum: float;
        for k: 0..2  sum += a.coef[row][k] * b.coef[k][column];
        if column == 3  sum += a.coef[row][3];
        result.coef[row][column] = sum;
    }
    return result;
}

operator * :: (a: Matrix2, b: Matrix2) -> Matrix2 { return inline multiply(a, b); }
operator * :: (a: Matrix4x3, b: Matrix4x3) -> Matrix4x3 { return inline multiply(a, b); }
operator * :: (a: Matrix4, b: Matrix4x3) -> Matrix4 { return inline multiply(a, b); }
operator * :: (a: Matrix4x3, b: Matrix4) -> Matrix4 { return inline multiply(a, b); }
operator * :: (a: Matrix4, b: Matrix3) -> Matrix4 { return inline multiply(a, b); }
operator * :: (m: Matrix2, v: Vector2) -> Vector2 { return inline multiply(m, v); }
operator * :: (m: Matrix4x3, v: Vector4) -> Vector3 { return inline multiply(m, v); }

// Elementwise, over the flat view, so one routine covers every matrix width.
operator * :: (a: $T/interface AnyMatrix, t: float) -> T #symmetric {
    r: T = ---;
    for 0..a.floats.count - 1  r.floats[it] = a.floats[it] * t;
    return r;
}

operator + :: (a: $T/interface AnyMatrix, b: T) -> T {
    r: T = ---;
    for 0..a.floats.count - 1  r.floats[it] = a.floats[it] + b.floats[it];
    return r;
}

operator - :: (a: $T/interface AnyMatrix, b: T) -> T {
    r: T = ---;
    for 0..a.floats.count - 1  r.floats[it] = a.floats[it] - b.floats[it];
    return r;
}

operator == :: (a: $T/interface AnyMatrix, b: T) -> bool {
    for 0..a.floats.count - 1  if a.floats[it] != b.floats[it]  return false;
    return true;
}
