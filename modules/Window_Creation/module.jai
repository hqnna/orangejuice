//
// Window_Creation: opens the windows a program draws into and reads input
// from.
//
//     window := create_window(800, 600, "My Program");
//     Simp.set_render_target(window);
//
// A window is a `Window_Type` (see the `Window_Type` module, which this one
// re-declares so that a program importing only `Window_Creation` and `Input`
// can name the type). It is shown as soon as it is made; `Input` reports
// what happens to it, `Simp` draws into it, and a window that asks to close
// is a `.QUIT` event rather than a window that goes away by itself.
//
// `width` and `height` are the size of the drawable area. `window_x` and
// `window_y`, when not -1, place its top-left corner on the screen, which a
// window manager may overrule. `background_color_rgb` is what the window
// shows before anything is drawn into it. `wanted_msaa` asks for a
// multisampled framebuffer; when the display cannot give one, the window
// gets an ordinary framebuffer instead. `window_class` is the name the
// window system groups the program's windows under, the window's name when
// empty.
//
// On Linux a window is an X11 window on `X11`'s shared `x_global_display`,
// with a visual GLX can render to (double-buffered RGBA with depth and
// stencil) when the display offers one and the display's default visual
// otherwise. Every window takes part in the `WM_DELETE_WINDOW` protocol, so
// closing it from the window manager is a request the program can refuse.
//

#import "Basic";
WT :: #import "Window_Type";

Window_Type :: WT.Window_Type;
INVALID_WINDOW :: WT.INVALID_WINDOW;

// Where a window was before `toggle_fullscreen` made it fill the screen, so
// that it can go back.
Saved_Window_Info :: struct {
    x: s32;
    y: s32;
    width: s32;
    height: s32;
    fullscreen: bool;
}

#if OS == .LINUX {
    #load "linux.jai";
}
