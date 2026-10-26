//
// Window_Type: what a window is, to the modules that pass one around.
//
// `Window_Creation` makes windows, `Input` reports what happens to them and
// `Simp` draws into them; none of them should have to import the others to
// name the type, so it lives here on its own. A program that only stores a
// window imports this and nothing else:
//
//     the_window: Window_Type;
//
// On Linux a window is an X11 window, which Xlib calls `Window` and the X
// protocol an XID — an unsigned 32-bit resource id that Xlib carries in a C
// `unsigned long`. `INVALID_WINDOW` is X's `None`, which no window is.
//
// orangejuice's distribution has no window system for macOS yet; there a
// window is an opaque pointer, so that a program that merely names the type
// still compiles.
//

#if OS == .LINUX {
    Window_Type :: u64;
    INVALID_WINDOW : Window_Type : 0;
} else {
    Window_Type :: *void;
    INVALID_WINDOW : Window_Type : null;
}
