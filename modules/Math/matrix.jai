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
