//
// The shaders Simp draws with, and how a program writes one of its own.
//
// A shader is one GLSL source holding both stages. `get_shader_program`
// compiles it twice, as GLSL 3.30, with `VERTEX_SHADER` defined and `OUT_IN`
// meaning `out` the first time and `FRAGMENT_SHADER` defined and `OUT_IN`
// meaning `in` the second, so a varying is declared once as `OUT_IN`. The
// vertex stage reads `vert_position` (a `vec4`, of which Simp supplies x, y
// and z), `vert_color` and `vert_uv0`, and the `mat4 projection` uniform
// maps pixels to clip space. A sampler is bound to texture unit 0.
//

shader_color:            Shader;
shader_color_with_blend: Shader;
shader_sprite:           Shader;
shader_text:             Shader;

// Vertex colours, written straight through, or blended over what is there.
set_shader_for_color :: (enable_blend := false) {
    immediate_set_shader(ifx enable_blend then *shader_color_with_blend else *shader_color);
}

// `texture` times the vertex colour, blended by its alpha.
set_shader_for_images :: (texture: *Texture) {
    immediate_set_texture(texture);
    immediate_set_shader(*shader_sprite);
}

// Glyphs of a font atlas: the vertex colour, as opaque as the glyph covers.
set_shader_for_text :: (texture: *Texture) {
    immediate_set_texture(texture);
    immediate_set_shader(*shader_text);
}

// Draws what is batched and makes `shader` draw what comes next.
immediate_set_shader :: (shader: *Shader) {
    state := simp_state();
    if !gl_ready {
        state.current_shader = shader;
        return;
    }
    if state.current_shader == shader && shader.locations_known  return;
    immediate_flush();
    state.current_shader = shader;
    if !shader.locations_known  find_locations(shader);
    glUseProgram(shader.gl_handle);
    if shader.alpha_blend {
        glEnable(GL_BLEND);
        glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
    } else {
        glDisable(GL_BLEND);
    }
    if shader.texture_loc >= 0  glUniform1i(shader.texture_loc, 0);
    set_projection();
}

// Compiles and links `source` as described above, returning 0 and logging
// why when it does not compile.
get_shader_program :: (source: string) -> GLuint {
    if !gl_ready {
        log_error("Simp: get_shader_program needs a render target; call set_render_target first.\n");
        return 0;
    }
    vertex := compile_stage(GL_VERTEX_SHADER, VERTEX_PREFIX, source);
    fragment := compile_stage(GL_FRAGMENT_SHADER, FRAGMENT_PREFIX, source);
    if !vertex || !fragment  return 0;

    program := glCreateProgram();
    glAttachShader(program, vertex);
    glAttachShader(program, fragment);
    glBindAttribLocation(program, ATTRIBUTE_POSITION, "vert_position");
    glBindAttribLocation(program, ATTRIBUTE_COLOR, "vert_color");
    glBindAttribLocation(program, ATTRIBUTE_UV_0, "vert_uv0");
    glLinkProgram(program);
    glDeleteShader(vertex);
    glDeleteShader(fragment);

    linked: GLint;
    glGetProgramiv(program, GL_LINK_STATUS, *linked);
    if !linked {
        log_error("Simp: a shader did not link:\n%\n", info_log(program, true));
        glDeleteProgram(program);
        return 0;
    }
    return program;
}

#scope_module

ATTRIBUTE_POSITION :: 0;
ATTRIBUTE_COLOR    :: 1;
ATTRIBUTE_UV_0     :: 2;

VERTEX_PREFIX   :: "#version 330 core\n#define VERTEX_SHADER\n#define OUT_IN out\n#line 1\n";
FRAGMENT_PREFIX :: "#version 330 core\n#define FRAGMENT_SHADER\n#define OUT_IN in\n#line 1\n";

compile_stage :: (kind: GLenum, prefix: string, source: string) -> GLuint {
    shader := glCreateShader(kind);
    sources: [2] *u8;
    sources[0] = prefix.data;
    sources[1] = source.data;
    lengths: [2] GLint;
    lengths[0] = xx prefix.count;
    lengths[1] = xx source.count;
    glShaderSource(shader, 2, sources.data, lengths.data);
    glCompileShader(shader);
    compiled: GLint;
    glGetShaderiv(shader, GL_COMPILE_STATUS, *compiled);
    if !compiled {
        stage := ifx kind == GL_VERTEX_SHADER then "vertex" else "fragment";
        log_error("Simp: a % shader did not compile:\n%\n", stage, info_log(shader, false));
        glDeleteShader(shader);
        return 0;
    }
    return shader;
}

info_log :: (object: GLuint, is_program: bool) -> string {
    length: GLint;
    if is_program  glGetProgramiv(object, GL_INFO_LOG_LENGTH, *length);
    else           glGetShaderiv(object, GL_INFO_LOG_LENGTH, *length);
    if length <= 0  return "";
    text := talloc_string(length);
    written: GLsizei;
    if is_program  glGetProgramInfoLog(object, length, *written, text.data);
    else           glGetShaderInfoLog(object, length, *written, text.data);
    text.count = written;
    return text;
}

find_locations :: (shader: *Shader) {
    shader.position_loc   = glGetAttribLocation(shader.gl_handle, "vert_position");
    shader.color_loc      = glGetAttribLocation(shader.gl_handle, "vert_color");
    shader.uv_0_loc       = glGetAttribLocation(shader.gl_handle, "vert_uv0");
    shader.projection_loc = glGetUniformLocation(shader.gl_handle, "projection");
    shader.texture_loc    = glGetUniformLocation(shader.gl_handle, "diffuse_texture");
    if shader.texture_loc < 0  shader.texture_loc = glGetUniformLocation(shader.gl_handle, "text_sampler");
    shader.locations_known = true;
}

init_shaders :: () {
    shader_color.gl_handle = get_shader_program(COLOR_SOURCE);
    shader_color_with_blend.gl_handle = shader_color.gl_handle;
    shader_color_with_blend.alpha_blend = true;
    shader_sprite.gl_handle = get_shader_program(SPRITE_SOURCE);
    shader_sprite.alpha_blend = true;
    shader_text.gl_handle = get_shader_program(SPRITE_SOURCE);
    shader_text.alpha_blend = true;
}

COLOR_SOURCE :: #string GLSL
OUT_IN vec4 iterated_color;

#ifdef VERTEX_SHADER
in vec4 vert_position;
in vec4 vert_color;
uniform mat4 projection;

void main() {
    gl_Position = projection * vec4(vert_position.xyz, 1.0);
    iterated_color = vert_color;
}
#endif

#ifdef FRAGMENT_SHADER
out vec4 color;

void main() {
    color = iterated_color;
}
#endif
GLSL

// Glyph atlases are one channel and swizzled so that sampling one gives
// white with the coverage as alpha, which is why text needs no shader of its
// own.
SPRITE_SOURCE :: #string GLSL
OUT_IN vec4 iterated_color;
OUT_IN vec2 iterated_uv;

#ifdef VERTEX_SHADER
in vec4 vert_position;
in vec4 vert_color;
in vec2 vert_uv0;
uniform mat4 projection;

void main() {
    gl_Position = projection * vec4(vert_position.xyz, 1.0);
    iterated_color = vert_color;
    iterated_uv = vert_uv0;
}
#endif

#ifdef FRAGMENT_SHADER
out vec4 color;
uniform sampler2D diffuse_texture;

void main() {
    color = texture(diffuse_texture, iterated_uv) * iterated_color;
}
#endif
GLSL
