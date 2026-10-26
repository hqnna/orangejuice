//
// The batch every immediate_* procedure adds to.
//
// Vertices go into a fixed buffer three at a time — a quad is two triangles
// — and the whole buffer is drawn with one `glDrawArrays` when it would
// overflow, and whenever something that changes how it is drawn (the
// shader, the texture, the scissor, the render target) is about to change.
//
// A quad's corners go counter-clockwise from the first: with the `x0, y0,
// x1, y1` form that is (x0, y0), (x1, y0), (x1, y1), (x0, y1), with texture
// coordinates (0, 0), (1, 0), (1, 1), (0, 1). Its per-corner colours are
// named for the corner: `c00` at (x0, y0), `c10` at (x1, y0), `c11` at (x1,
// y1) and `c01` at (x0, y1).
//

Vertex :: struct {
    position: Vector3;
    color: Vector4;
    uv: Vector2;
}

MAX_IMMEDIATE_VERTICES :: 6 * 2048;

immediate_begin :: () {
    immediate_flush();
}

immediate_flush :: () {
    if !immediate_count  return;
    count := immediate_count;
    immediate_count = 0;
    if !gl_ready  return;
    state := simp_state();
    shader := state.current_shader;
    if !shader  return;
    if !shader.locations_known  find_locations(shader);

    backend_bind_vertex_array();
    glBindBuffer(GL_ARRAY_BUFFER, immediate_vbo);
    glBufferData(GL_ARRAY_BUFFER, count * size_of(Vertex), immediate_vertices.data, GL_STREAM_DRAW);
    bind_attribute(shader.position_loc, 3, 0);
    bind_attribute(shader.color_loc, 4, size_of(Vector3));
    bind_attribute(shader.uv_0_loc, 2, size_of(Vector3) + size_of(Vector4));
    if state.current_texture  glBindTexture(GL_TEXTURE_2D, state.current_texture.gl_handle);
    glDrawArrays(GL_TRIANGLES, 0, xx count);
}

immediate_triangle :: (p0: Vector3, p1: Vector3, p2: Vector3, c0 := Vector4.{1, 1, 1, 1}, c1 := Vector4.{1, 1, 1, 1}, c2 := Vector4.{1, 1, 1, 1}, uv0 := Vector2.{0, 0}, uv1 := Vector2.{1, 0}, uv2 := Vector2.{1, 1}) {
    v := immediate_room(3);
    v[0] = .{p0, c0, uv0};
    v[1] = .{p1, c1, uv1};
    v[2] = .{p2, c2, uv2};
}

immediate_triangle :: (p0: Vector2, p1: Vector2, p2: Vector2, c0 := Vector4.{1, 1, 1, 1}, c1 := Vector4.{1, 1, 1, 1}, c2 := Vector4.{1, 1, 1, 1}, uv0 := Vector2.{0, 0}, uv1 := Vector2.{1, 0}, uv2 := Vector2.{1, 1}) {
    immediate_triangle(to_3(p0), to_3(p1), to_3(p2), c0, c1, c2, uv0, uv1, uv2);
}

// An axis-aligned rectangle in one colour.
immediate_quad :: (x0: float, y0: float, x1: float, y1: float, color := Vector4.{1, 1, 1, 1}) {
    immediate_quad(.{x0, y0, 0}, .{x1, y0, 0}, .{x1, y1, 0}, .{x0, y1, 0}, color, color, color, color);
}

// An axis-aligned rectangle with a colour per corner.
immediate_quad :: (x0: float, y0: float, x1: float, y1: float, c00: Vector4, c01: Vector4, c10: Vector4, c11: Vector4) {
    immediate_quad(.{x0, y0, 0}, .{x1, y0, 0}, .{x1, y1, 0}, .{x0, y1, 0}, c00, c10, c11, c01);
}

immediate_quad :: (p0: Vector2, p1: Vector2, p2: Vector2, p3: Vector2, color := Vector4.{1, 1, 1, 1}, uv0 := Vector2.{0, 0}, uv1 := Vector2.{1, 0}, uv2 := Vector2.{1, 1}, uv3 := Vector2.{0, 1}) {
    immediate_quad(to_3(p0), to_3(p1), to_3(p2), to_3(p3), color, color, color, color, uv0, uv1, uv2, uv3);
}

immediate_quad :: (p0: Vector3, p1: Vector3, p2: Vector3, p3: Vector3, color := Vector4.{1, 1, 1, 1}, uv0 := Vector2.{0, 0}, uv1 := Vector2.{1, 0}, uv2 := Vector2.{1, 1}, uv3 := Vector2.{0, 1}) {
    immediate_quad(p0, p1, p2, p3, color, color, color, color, uv0, uv1, uv2, uv3);
}

// The general quad: a colour and texture coordinate per corner.
immediate_quad :: (p0: Vector3, p1: Vector3, p2: Vector3, p3: Vector3, c0: Vector4, c1: Vector4, c2: Vector4, c3: Vector4, uv0 := Vector2.{0, 0}, uv1 := Vector2.{1, 0}, uv2 := Vector2.{1, 1}, uv3 := Vector2.{0, 1}) {
    v := immediate_room(6);
    v[0] = .{p0, c0, uv0};
    v[1] = .{p1, c1, uv1};
    v[2] = .{p2, c2, uv2};
    v[3] = .{p0, c0, uv0};
    v[4] = .{p2, c2, uv2};
    v[5] = .{p3, c3, uv3};
}

#scope_module

immediate_vertices: [MAX_IMMEDIATE_VERTICES] Vertex;
immediate_count: s64;
immediate_vbo: GLuint;

bind_attribute :: (location: GLint, components: GLint, offset: s64) {
    if location < 0  return;
    glEnableVertexAttribArray(xx location);
    glVertexAttribPointer(xx location, components, GL_FLOAT, GL_FALSE, size_of(Vertex), cast(*void) offset);
}

to_3 :: inline (p: Vector2) -> Vector3 {
    return .{p.x, p.y, 0};
}

// Room for `count` more vertices, drawing what is there first when they
// would not fit.
immediate_room :: (count: s64) -> *Vertex {
    if immediate_count + count > MAX_IMMEDIATE_VERTICES  immediate_flush();
    result := *immediate_vertices[immediate_count];
    immediate_count += count;
    return result;
}

// Draws what is batched and makes `texture` the one drawn with next.
immediate_set_texture :: (texture: *Texture) {
    state := simp_state();
    if texture && gl_ready  texture_make_resident(texture);
    if state.current_texture == texture  return;
    immediate_flush();
    state.current_texture = texture;
}
