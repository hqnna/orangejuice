//
// Window_Creation's X11 side.
//
// The visual is chosen with `glXChooseVisual`, declared here rather than
// taken from `GL` so that a program that only wants a window does not compile
// all of OpenGL. Fullscreen is the EWMH `_NET_WM_STATE_FULLSCREEN` state,
// which only a window manager acts on.
//

#scope_module

X :: #import "X11";

#scope_export

create_window :: (width: int, height: int, window_name: string, window_x := -1, window_y := -1, parent := INVALID_WINDOW, background_color_rgb := float.[0.15, 0.15, 0.2], wanted_msaa: s32 = 0, window_class := "") -> Window_Type {
    if !X.init_global_display() {
        log_error("Window_Creation: could not open the X display; is DISPLAY set?\n");
        return INVALID_WINDOW;
    }
    display := X.x_global_display;
    screen := X.XDefaultScreen(display);
    root := ifx parent != INVALID_WINDOW then parent else X.XRootWindow(display, screen);

    visual := X.XDefaultVisual(display, screen);
    depth := X.XDefaultDepth(display, screen);
    info := choose_gl_visual(display, screen, wanted_msaa);
    if !info && wanted_msaa > 0  info = choose_gl_visual(display, screen, 0);
    if info {
        visual = info.visual;
        depth = info.depth;
        X.XFree(info);
    }

    attributes: X.XSetWindowAttributes;
    attributes.colormap = X.XCreateColormap(display, root, visual, X.AllocNone);
    attributes.background_pixel = pixel_from_rgb(background_color_rgb);
    attributes.border_pixel = 0;
    attributes.event_mask = EVENT_MASK;
    mask := X.CWColormap | X.CWBackPixel | X.CWBorderPixel | X.CWEventMask;

    x := ifx window_x == -1 then 0 else window_x;
    y := ifx window_y == -1 then 0 else window_y;
    window := X.XCreateWindow(display, root, xx x, xx y, xx max(width, 1), xx max(height, 1), 0, depth, X.InputOutput, visual, xx mask, *attributes);
    if !window {
        log_error("Window_Creation: XCreateWindow failed.\n");
        return INVALID_WINDOW;
    }

    name := temp_c_string(window_name);
    X.XStoreName(display, window, name);
    utf8 := X.XInternAtom(display, "UTF8_STRING", X.False);
    net_wm_name := X.XInternAtom(display, "_NET_WM_NAME", X.False);
    X.XChangeProperty(display, window, net_wm_name, utf8, 8, X.PropModeReplace, window_name.data, xx window_name.count);

    class_name := temp_c_string(ifx window_class then window_class else window_name);
    class_hint: X.XClassHint;
    class_hint.res_name = class_name;
    class_hint.res_class = class_name;
    X.XSetClassHint(display, window, *class_hint);

    if window_x != -1 || window_y != -1 {
        hints: X.XSizeHints;
        hints.flags = X.USPosition | X.PPosition;
        hints.x = xx x;
        hints.y = xx y;
        X.XSetWMNormalHints(display, window, *hints);
    }

    wm_delete_window := X.XInternAtom(display, "WM_DELETE_WINDOW", X.False);
    X.XSetWMProtocols(display, window, *wm_delete_window, 1);

    X.XMapWindow(display, window);
    X.XFlush(display);
    wait_until_mapped(display, window);

    last_created_window = window;
    return window;
}

// Where the window is on the screen and how big its drawable area is.
// `right_handed` measures `y` up from the bottom of the screen rather than
// down from the top.
get_dimensions :: (window: Window_Type, right_handed: bool) -> x: s32, y: s32, width: s32, height: s32, success: bool {
    display := X.x_global_display;
    if !display || !window  return 0, 0, 0, 0, false;
    attributes: X.XWindowAttributes;
    if !X.XGetWindowAttributes(display, window, *attributes)  return 0, 0, 0, 0, false;
    x, y: s32;
    child: X.Window;
    X.XTranslateCoordinates(display, window, attributes.root, 0, 0, *x, *y, *child);
    if right_handed {
        screen := X.XDefaultScreen(display);
        y = X.XDisplayHeight(display, screen) - (y + attributes.height);
    }
    return x, y, attributes.width, attributes.height, true;
}

// Where the mouse pointer is, relative to the top-left corner of `window` —
// or, when `right_handed`, to its bottom-left corner with `y` going up. It
// may be outside the window.
get_mouse_pointer_position :: (window: Window_Type, right_handed: bool) -> x: int, y: int, success: bool {
    display := X.x_global_display;
    if !display || !window  return 0, 0, false;
    root, child: X.Window;
    root_x, root_y, x, y: s32;
    buttons: u32;
    if !X.XQueryPointer(display, window, *root, *child, *root_x, *root_y, *x, *y, *buttons)  return 0, 0, false;
    if right_handed {
        attributes: X.XWindowAttributes;
        X.XGetWindowAttributes(display, window, *attributes);
        y = attributes.height - y;
    }
    return x, y, true;
}

// The same, relative to the window that has the keyboard focus, or the last
// window created when none of the program's has.
get_mouse_pointer_position :: (right_handed := false) -> x: int, y: int, success: bool {
    display := X.x_global_display;
    if !display  return 0, 0, false;
    focus: X.Window;
    revert: s32;
    X.XGetInputFocus(display, *focus, *revert);
    window := last_created_window;
    if focus > 1 && focus != X.XDefaultRootWindow(display)  window = focus;
    x, y, success := get_mouse_pointer_position(window, right_handed);
    return x, y, success;
}

// Makes `window` fill the screen, or puts it back where `saved` says it was.
// The window manager does the resizing, so the size returned is the one
// asked for; the window's real size arrives as a resize from `Input`.
toggle_fullscreen :: (window: Window_Type, desire_fullscreen: bool, saved: *Saved_Window_Info) -> success: bool, width: s32, height: s32 {
    display := X.x_global_display;
    if !display || !window  return false, 0, 0;

    x, y, width, height, ok := get_dimensions(window, false);
    if !ok  return false, 0, 0;
    if desire_fullscreen == saved.fullscreen  return true, width, height;

    if desire_fullscreen {
        saved.x = x;
        saved.y = y;
        saved.width = width;
        saved.height = height;
    }

    screen := X.XDefaultScreen(display);
    event: X.XEvent;
    event.xclient.type = X.ClientMessage;
    event.xclient.window = window;
    event.xclient.message_type = X.XInternAtom(display, "_NET_WM_STATE", X.False);
    event.xclient.format = 32;
    event.xclient.data.l[0] = ifx desire_fullscreen then NET_WM_STATE_ADD else NET_WM_STATE_REMOVE;
    event.xclient.data.l[1] = xx X.XInternAtom(display, "_NET_WM_STATE_FULLSCREEN", X.False);
    event.xclient.data.l[3] = 1;
    X.XSendEvent(display, X.XRootWindow(display, screen), X.False, X.SubstructureRedirectMask | X.SubstructureNotifyMask, *event);
    X.XFlush(display);
    saved.fullscreen = desire_fullscreen;

    if desire_fullscreen  return true, X.XDisplayWidth(display, screen), X.XDisplayHeight(display, screen);
    return true, saved.width, saved.height;
}

#scope_file

EVENT_MASK :: X.KeyPressMask | X.KeyReleaseMask | X.ButtonPressMask | X.ButtonReleaseMask | X.PointerMotionMask | X.StructureNotifyMask | X.FocusChangeMask | X.ExposureMask | X.EnterWindowMask | X.LeaveWindowMask;

NET_WM_STATE_REMOVE :: 0;
NET_WM_STATE_ADD    :: 1;

GLX_RGBA           :: 4;
GLX_DOUBLEBUFFER   :: 5;
GLX_RED_SIZE       :: 8;
GLX_GREEN_SIZE     :: 9;
GLX_BLUE_SIZE      :: 10;
GLX_DEPTH_SIZE     :: 12;
GLX_STENCIL_SIZE   :: 13;
GLX_SAMPLE_BUFFERS :: 100000;
GLX_SAMPLES        :: 100001;

libgl :: #library,system "libGL";
glXChooseVisual :: (display: *X.Display, screen: s32, attributes: *s32) -> *X.XVisualInfo #foreign libgl;

last_created_window: Window_Type;

choose_gl_visual :: (display: *X.Display, screen: s32, msaa: s32) -> *X.XVisualInfo {
    attributes: [..] s32;
    attributes.allocator = temp;
    array_add(*attributes, GLX_RGBA, GLX_DOUBLEBUFFER, GLX_RED_SIZE, 8, GLX_GREEN_SIZE, 8, GLX_BLUE_SIZE, 8, GLX_DEPTH_SIZE, 24, GLX_STENCIL_SIZE, 8);
    if msaa > 0  array_add(*attributes, GLX_SAMPLE_BUFFERS, 1, GLX_SAMPLES, msaa);
    array_add(*attributes, 0);
    return glXChooseVisual(display, screen, attributes.data);
}

pixel_from_rgb :: (rgb: [3] float) -> u64 {
    channel :: (value: float) -> u64 {
        return cast(u64) (clamp(value, 0, 1) * 255 + 0.5);
    }
    return (channel(rgb[0]) << 16) | (channel(rgb[1]) << 8) | channel(rgb[2]);
}

// A window is mapped once the server says so; drawing or giving it the focus
// before then is lost. A window manager may take a while to agree, so this
// gives up after a second and lets the program carry on.
wait_until_mapped :: (display: *X.Display, window: X.Window) {
    for 1..1000 {
        event: X.XEvent;
        if X.XCheckTypedWindowEvent(display, window, X.MapNotify, *event)  return;
        sleep_milliseconds(1);
    }
}
