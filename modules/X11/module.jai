//
// X11: a binding to Xlib, the C client library of the X Window System.
//
// `generated.jai` is projected from the system's own headers by
// `tools/cbind`: every procedure `Xlib.h`, `Xutil.h`, `Xresource.h` and
// `XKBlib.h` declare, the event structs and the `XEvent` union, the protocol
// constants of `X.h`, the predefined atoms of `Xatom.h`, the cursor shapes
// of `cursorfont.h`, the keysyms of `keysym.h` and the input-method names
// (`XNClientWindow`) as strings. What Xlib writes as function-like macros —
// `DefaultScreen(display)` — it also exports as procedures, `XDefaultScreen`,
// and those are what is bound.
//
// The library is linked as `#library,system`, which `pkg-config x11` finds
// (**C§11**).
//
// `x_global_display` is the one connection a program that does not want to
// manage its own can share — `Clipboard` opens one there if nobody has.
//

#scope_module

libx11 :: #library,system "libX11";

#scope_export

x_global_display: *Display;

// Opens `x_global_display` unless it already is, and says whether it is.
init_global_display :: () -> bool {
    if !x_global_display  x_global_display = XOpenDisplay(null);
    return x_global_display != null;
}

#load "generated.jai";
