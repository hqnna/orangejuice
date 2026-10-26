//
// Simp's GLX backend: a GLX context per window, all sharing one set of
// objects with the first, and OpenGL loaded through `GL` once that first one
// is current.
//
// The context is made for the visual the window was created with — which is
// why `Window_Creation` picks a GLX visual — so any window GLX can draw into
// works, whoever created it. A vertex array object is not shared between
// contexts, so each window has its own.
//
// `vsync` is `GLX_EXT_swap_control` or `GLX_MESA_swap_control`, whichever
// the display has, set when it changes.
//

#scope_module

X :: #import "X11";

Window_Record :: struct {
    window: Window_Type;
    gl_context: GLXContext;
    vertex_array: GLuint;
    swap_interval: s32 = -1;
}

window_records: [..] *Window_Record;
current_record: *Window_Record;
shared_context: GLXContext;

Swap_Interval_EXT  :: #type (display: *X.Display, drawable: GLXDrawable, interval: s32) #c_call;
Swap_Interval_MESA :: #type (interval: u32) -> s32 #c_call;
swap_interval_ext: Swap_Interval_EXT;
swap_interval_mesa: Swap_Interval_MESA;

backend_make_current :: (window: Window_Type) -> bool {
    display := X.x_global_display;
    if !display || !window {
        log_error("Simp: set_render_target needs a window from Window_Creation.\n");
        return false;
    }

    record: *Window_Record;
    for window_records  if it.window == window  record = it;
    if !record {
        attributes: X.XWindowAttributes;
        if !X.XGetWindowAttributes(display, window, *attributes) {
            log_error("Simp: % is not a window on this display.\n", window);
            return false;
        }
        wanted: X.XVisualInfo;
        wanted.visualid = X.XVisualIDFromVisual(attributes.visual);
        count: s32;
        info := X.XGetVisualInfo(display, X.VisualIDMask, *wanted, *count);
        if !info {
            log_error("Simp: the window's visual is unknown to the display.\n");
            return false;
        }
        gl_context := glXCreateContext(display, info, shared_context, X.True);
        X.XFree(info);
        if !gl_context {
            log_error("Simp: the window has no visual OpenGL can render to (no GLX on this display?).\n");
            return false;
        }
        if !shared_context  shared_context = gl_context;
        record = New(Window_Record);
        record.window = window;
        record.gl_context = gl_context;
        array_add(*window_records, record);
    }

    if !glXMakeCurrent(display, window, record.gl_context) {
        log_error("Simp: could not make the window's OpenGL context current.\n");
        return false;
    }
    current_record = record;

    if !gl_ready  start_gl();
    if !record.vertex_array {
        glGenVertexArrays(1, *record.vertex_array);
        glBindVertexArray(record.vertex_array);
    }
    state := simp_state();
    if state.current_shader {
        shader := state.current_shader;
        state.current_shader = null;
        immediate_set_shader(shader);
    }
    return true;
}

backend_bind_vertex_array :: () {
    if current_record  glBindVertexArray(current_record.vertex_array);
}

backend_window_size :: (window: Window_Type) -> s32, s32 {
    display := X.x_global_display;
    if !display || !window  return 0, 0;
    attributes: X.XWindowAttributes;
    if !X.XGetWindowAttributes(display, window, *attributes)  return 0, 0;
    return attributes.width, attributes.height;
}

backend_swap_buffers :: (window: Window_Type, vsync: bool) {
    display := X.x_global_display;
    if !display  return;
    interval: s32 = xx ifx vsync then 1 else 0;
    for window_records  if it.window == window && it.swap_interval != interval {
        it.swap_interval = interval;
        if swap_interval_ext  swap_interval_ext(display, window, interval);
        else if swap_interval_mesa && window == simp_state().current_window  swap_interval_mesa(xx interval);
    }
    glXSwapBuffers(display, window);
}

has_word :: (list: string, word: string) -> bool {
    rest := list;
    while rest {
        found, item, remainder := split_from_left(rest, #char " ");
        if item == word  return true;
        if !found  break;
        rest = remainder;
    }
    return false;
}

start_gl :: () {
    gl_load(*gl);
    gl_ready = true;
    major, minor := gl_version();
    if major < 3 || (major == 3 && minor < 3) {
        log_error("Simp: OpenGL 3.3 is needed, and this context is %.%.\n", major, minor);
    }
    display := X.x_global_display;
    extensions := to_string(glXQueryExtensionsString(display, X.XDefaultScreen(display)));
    if has_word(extensions, "GLX_EXT_swap_control") {
        swap_interval_ext = xx glXGetProcAddressARB("glXSwapIntervalEXT");
    } else if has_word(extensions, "GLX_MESA_swap_control") {
        swap_interval_mesa = xx glXGetProcAddressARB("glXSwapIntervalMESA");
    }

    glDisable(GL_DEPTH_TEST);
    glDisable(GL_CULL_FACE);
    glGenBuffers(1, *immediate_vbo);
    init_shaders();
}
