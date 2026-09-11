//
// Planes in three dimensions.
//
// `ax + by + cz + d = 0`. The first three coefficients are the normal, so a
// plane and a `Vector4` are the same four floats — which is why the normal and
// the vector form are `#place`d over the same storage rather than computed.
//
// A *normalized* plane is one whose normal has unit length; only then is
// `plane_dot` the signed distance from the plane to a point.
//

Plane3 :: struct {
    a, b, c, d: float;

#place a;
    #as vector4: Vector4;

#place a;
    normal: Vector3;
}

make_plane :: (a: float, b: float, c: float, d: float) -> Plane3 {
    result: Plane3 = ---;
    result.a = a;
    result.b = b;
    result.c = c;
    result.d = d;
    return result;
}

get_normal :: (p: Plane3) -> Vector3 {
    return p.normal;
}

// The signed distance from the plane to `v` when the plane is normalized:
// positive on the side the normal points at, negative on the other.
plane_dot :: (plane: Plane3, v: Vector3) -> float #symmetric {
    return plane.a * v.x + plane.b * v.y + plane.c * v.z + plane.d;
}

plane_from_point_and_normal :: (point: Vector3, normal: Vector3) -> Plane3 {
    result: Plane3 = ---;
    result.normal = normal;
    // `d` is what makes the plane pass through `point`.
    result.d = -dot(normal, point);
    return result;
}

// The plane through three points, wound so that the normal follows the
// right-hand rule around `p0 -> p1 -> p2`.
normalized_plane_from_points :: (p0: Vector3, p1: Vector3, p2: Vector3) -> Plane3 {
    normal := unit_vector(cross(p1 - p0, p2 - p0));
    return plane_from_point_and_normal(p0, normal);
}
