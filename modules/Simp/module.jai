//
// Simp: a small immediate-mode 2D renderer — coloured and textured
// triangles and quads, images and text — drawn with OpenGL into the windows
// `Window_Creation` makes.
//
//     window := create_window(800, 600, "Simp");
//     Simp.set_render_target(window);
//     while running {
//         update_window_events();
//         for get_window_resizes()  Simp.update_window(it.window);
//         Simp.clear_render_target(.2, .3, .3, 1);
//         Simp.set_shader_for_color();
//         Simp.immediate_quad(10, 10, 110, 60, .{1, .5, .5, 1});
//         Simp.draw_text(font, 10, 100, "Hello", .{1, 1, 1, 1});
//         Simp.swap_buffers(window);
//     }
//
// Coordinates are pixels of the render target. With the default
// `.RIGHT_HANDED` coordinate system (0, 0) is the bottom-left corner and `y`
// goes up; `set_render_target(window, .LEFT_HANDED)` puts (0, 0) at the
// top-left with `y` going down, the way a text editor counts lines.
//
// Drawing is batched: `immediate_quad` and `immediate_triangle` add vertices
// to a buffer that is drawn when it fills, when the shader or texture
// changes, and on `immediate_flush` or `swap_buffers`. Which shader draws
// them is chosen with `set_shader_for_color` (vertex colours, blended when
// asked), `set_shader_for_images` (a `Texture` times the vertex colour) and
// `set_shader_for_text`; `immediate_set_shader` takes a `Shader` of the
// program's own, written as one source with `#ifdef VERTEX_SHADER` and
// `#ifdef FRAGMENT_SHADER` halves and compiled by `get_shader_program` — see
// `shaders.jai` for what the source may assume.
//
// A `Texture` is loaded from an image file or from memory through
// `stb_image`, or made from a `Bitmap`; an image loaded while the coordinate
// system is right-handed is flipped so that a quad with the default texture
// coordinates shows it the right way up. Fonts are `Dynamic_Font`s, opened
// through `freetype` by `get_font_at_size` and rasterized a glyph at a time
// into atlas textures as text that needs them is prepared.
//
// A `Texture` or font loaded before there is a render target keeps its
// pixels and reaches the GPU the first time it is drawn with.
//
// The renderer's state is `context.simp`, which `set_render_target` points
// at the module's own; a program may read `context.simp.current_shader` and
// `context.simp.coordinate_system`.
//
// The backend is OpenGL 3.3 through GLX on Linux. Each window gets its own
// GLX context, all sharing one set of textures and shaders.
//

#import "Basic";
#import "Math";
#import "String";
#import "GL";
WT :: #import "Window_Type";

#scope_module
Window_Type :: WT.Window_Type;
#scope_export

Coordinate_System :: enum u8 {
    RIGHT_HANDED :: 0;
    LEFT_HANDED  :: 1;
}

Shader :: struct {
    gl_handle: GLuint;
    alpha_blend: bool;

    // Where the program's attributes and uniforms are, found the first time
    // the shader is used.
    locations_known: bool;
    position_loc:   GLint = -1;
    color_loc:      GLint = -1;
    uv_0_loc:       GLint = -1;
    projection_loc: GLint = -1;
    texture_loc:    GLint = -1;
}

Simp_State :: struct {
    current_shader: *Shader;
    coordinate_system: Coordinate_System;

    current_window: Window_Type;
    render_target_width: s32;
    render_target_height: s32;

    current_texture: *Texture;
    scissor_enabled: bool;
    scissor: [4] s32;

    projection: [16] float;
}

#add_context simp: *Simp_State;

// Makes `window` what everything draws into, until the next call. The first
// call also starts OpenGL.
set_render_target :: (window: Window_Type, coordinate_system := Coordinate_System.RIGHT_HANDED) {
    state := simp_state();
    if state.current_window != window  immediate_flush();
    state.coordinate_system = coordinate_system;
    if !backend_make_current(window)  return;
    state.current_window = window;
    width, height := backend_window_size(window);
    set_render_target_size(width, height);
}

// Reads the window's size again, after `Input` reported a resize. Only the
// current render target needs it, but any window may be passed.
update_window :: (window: Window_Type) {
    state := simp_state();
    if window != state.current_window  return;
    width, height := backend_window_size(window);
    if width == state.render_target_width && height == state.render_target_height  return;
    immediate_flush();
    set_render_target_size(width, height);
}

// The size of the window's drawable area, in pixels.
get_render_dimensions :: (window: Window_Type) -> width: s32, height: s32 {
    width, height := backend_window_size(window);
    return width, height;
}

clear_render_target :: (r: float, g: float, b: float, a: float) {
    if !gl_ready  return;
    immediate_flush();
    state := simp_state();
    if state.scissor_enabled  glDisable(GL_SCISSOR_TEST);
    glClearColor(r, g, b, a);
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);
    if state.scissor_enabled  glEnable(GL_SCISSOR_TEST);
}

// Draws what is batched and shows the frame. `vsync` waits for the display's
// refresh when the platform lets it.
swap_buffers :: (window: Window_Type, vsync := true) {
    if !gl_ready  return;
    state := simp_state();
    if window == state.current_window  immediate_flush();
    backend_swap_buffers(window, vsync);
}

// Hands the current projection to the current shader again — what a program
// that switched shaders with OpenGL directly calls.
set_projection :: () {
    state := simp_state();
    shader := state.current_shader;
    if !gl_ready || !shader  return;
    if shader.projection_loc >= 0  glUniformMatrix4fv(shader.projection_loc, 1, GL_FALSE, state.projection.data);
}

// Only draws inside the rectangle from (x0, y0) to (x1, y1), in the
// coordinate system of the render target.
set_scissor :: (x0: s32, y0: s32, x1: s32, y1: s32) {
    if !gl_ready  return;
    immediate_flush();
    state := simp_state();
    state.scissor_enabled = true;
    state.scissor = .[x0, y0, x1, y1];
    apply_scissor();
}

clear_scissor :: () {
    if !gl_ready  return;
    immediate_flush();
    state := simp_state();
    state.scissor_enabled = false;
    glDisable(GL_SCISSOR_TEST);
}

#scope_module

module_state: Simp_State;
gl_ready: bool;

simp_state :: () -> *Simp_State {
    if !context.simp  context.simp = *module_state;
    return context.simp;
}

set_render_target_size :: (width: s32, height: s32) {
    state := simp_state();
    state.render_target_width = width;
    state.render_target_height = height;
    glViewport(0, 0, xx width, xx height);

    w := cast(float) max(width, 1);
    h := cast(float) max(height, 1);
    // Column-major, the way glUniformMatrix4fv reads it without transposing.
    p: [16] float;
    p[0] = 2 / w;
    p[5] = 2 / h;
    p[10] = -1;
    p[12] = -1;
    p[13] = -1;
    p[15] = 1;
    if state.coordinate_system == .LEFT_HANDED {
        p[5] = -2 / h;
        p[13] = 1;
    }
    state.projection = p;
    set_projection();
    if state.scissor_enabled  apply_scissor();
}

apply_scissor :: () {
    state := simp_state();
    x0 := min(state.scissor[0], state.scissor[2]);
    x1 := max(state.scissor[0], state.scissor[2]);
    y0 := min(state.scissor[1], state.scissor[3]);
    y1 := max(state.scissor[1], state.scissor[3]);
    if state.coordinate_system == .LEFT_HANDED {
        top := y0;
        y0 = state.render_target_height - y1;
        y1 = state.render_target_height - top;
    }
    glEnable(GL_SCISSOR_TEST);
    glScissor(x0, y0, xx (x1 - x0), xx (y1 - y0));
}

#scope_export

#load "shaders.jai";
#load "immediate.jai";
#load "texture.jai";
#load "font.jai";

#if OS == .LINUX {
    #load "backend_glx.jai";
}
