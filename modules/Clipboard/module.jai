//
// Clipboard: the system clipboard, as text.
//
//     os_clipboard_set_text("copied");
//     text := os_clipboard_get_text();   // allocated with context.allocator
//
// On Linux the clipboard is the X selection called CLIPBOARD (XWayland
// bridges it to a Wayland session), and it is not a buffer anywhere: the
// program that copied keeps the text and hands it to whoever asks, for as
// long as it stays the selection's owner. So setting the text makes this
// module the owner on a display connection of its own, and a thread it
// starts the first time answers the requests other programs send — as
// UTF8_STRING, STRING, TEXT or text/plain — without the program's event loop
// having to know about any of it.
//
// Getting the text asks the owner to convert the selection to UTF8_STRING,
// falling back to Latin-1 STRING for an owner that offers nothing else, and
// waits up to `CLIPBOARD_TIMEOUT_MS` for the answer. When this program is the
// owner, its own copy is returned without a round trip. A selection large
// enough that its owner sends it in INCR pieces is not reassembled; that
// returns the empty string, as does having no X display.
//
// Written from the ICCCM's description of selections (§2) and Xlib's
// declarations, which `X11` binds.
//

#if OS != .LINUX {
    #assert false "Clipboard is implemented over X11, which is Linux's; this target has no clipboard backend in orangejuice's distribution.";
}

#import "Basic";
#import "Thread";
X :: #import "X11";

CLIPBOARD_TIMEOUT_MS :: 1000;

// Makes `text` the clipboard's contents. False when there is no display to
// own the selection on.
os_clipboard_set_text :: (text: string) -> bool {
    if !start_owner()  return false;

    lock(*owner.mutex);
    free(owner.text);
    owner.text = copy_string(text, owner_allocator());
    X.XSetSelectionOwner(owner.display, owner.atoms.CLIPBOARD, owner.window, X.CurrentTime);
    owner.owned = X.XGetSelectionOwner(owner.display, owner.atoms.CLIPBOARD) == owner.window;
    X.XFlush(owner.display);
    owned := owner.owned;
    unlock(*owner.mutex);
    return owned;
}

// What the clipboard holds, as UTF-8, or the empty string.
os_clipboard_get_text :: () -> string {
    if owner.started {
        lock(*owner.mutex);
        defer unlock(*owner.mutex);
        if owner.owned && X.XGetSelectionOwner(owner.display, owner.atoms.CLIPBOARD) == owner.window {
            return copy_string(owner.text);
        }
    }

    display := X.XOpenDisplay(null);
    if !display  return "";
    defer X.XCloseDisplay(display);

    atoms := intern_atoms(display);
    if X.XGetSelectionOwner(display, atoms.CLIPBOARD) == X.None  return "";

    root := X.XDefaultRootWindow(display);
    window := X.XCreateSimpleWindow(display, root, 0, 0, 1, 1, 0, 0, 0);
    defer X.XDestroyWindow(display, window);

    text, ok := request(display, window, *atoms, atoms.UTF8_STRING);
    if !ok  text = request(display, window, *atoms, X.XA_STRING);
    return text;
}

#scope_file

Atoms :: struct {
    CLIPBOARD:   X.Atom;
    TARGETS:     X.Atom;
    UTF8_STRING: X.Atom;
    TEXT:        X.Atom;
    TEXT_PLAIN:  X.Atom;
    INCR:        X.Atom;
    PROPERTY:    X.Atom;
}

intern_atoms :: (display: *X.Display) -> Atoms {
    atoms: Atoms;
    atoms.CLIPBOARD   = X.XInternAtom(display, "CLIPBOARD", X.False);
    atoms.TARGETS     = X.XInternAtom(display, "TARGETS", X.False);
    atoms.UTF8_STRING = X.XInternAtom(display, "UTF8_STRING", X.False);
    atoms.TEXT        = X.XInternAtom(display, "TEXT", X.False);
    atoms.TEXT_PLAIN  = X.XInternAtom(display, "text/plain;charset=utf-8", X.False);
    atoms.INCR        = X.XInternAtom(display, "INCR", X.False);
    atoms.PROPERTY    = X.XInternAtom(display, "OJ_CLIPBOARD", X.False);
    return atoms;
}

// Asks the owner for the selection as `target` and waits for its answer.
request :: (display: *X.Display, window: X.Window, atoms: *Atoms, target: X.Atom) -> string, bool {
    X.XConvertSelection(display, atoms.CLIPBOARD, target, atoms.PROPERTY, window, X.CurrentTime);
    X.XFlush(display);

    event: X.XEvent;
    waited := 0;
    while true {
        if X.XCheckTypedWindowEvent(display, window, X.SelectionNotify, *event)  break;
        if waited >= CLIPBOARD_TIMEOUT_MS  return "", false;
        sleep_milliseconds(2);
        waited += 2;
    }
    if event.xselection.property == X.None  return "", false;

    type: X.Atom;
    format: s32;
    count, remaining: u64;
    data: *u8;
    status := X.XGetWindowProperty(display, window, atoms.PROPERTY, 0, 0x7fff_ffff, X.True, X.AnyPropertyType, *type, *format, *count, *remaining, *data);
    if status != X.Success  return "", false;
    defer if data  X.XFree(data);
    if type == atoms.INCR || format != 8  return "", false;

    result: string;
    result.count = cast(s64) count;
    result.data  = data;
    return copy_string(result), true;
}

Owner :: struct {
    started: bool;
    owned:   bool;
    display: *X.Display;
    window:  X.Window;
    atoms:   Atoms;
    text:    string;
    mutex:   Mutex;
    thread:  Thread;
}

owner: Owner;

// The text outlives whatever allocator the caller had pushed.
owner_allocator :: () -> Allocator {
    return context.default_allocator;
}

start_owner :: () -> bool {
    if owner.started  return true;
    X.XInitThreads();
    owner.display = X.XOpenDisplay(null);
    if !owner.display  return false;
    owner.atoms  = intern_atoms(owner.display);
    owner.window = X.XCreateSimpleWindow(owner.display, X.XDefaultRootWindow(owner.display), 0, 0, 1, 1, 0, 0, 0);
    init(*owner.mutex);
    owner.started = true;
    thread_init(*owner.thread, serve);
    thread_start(*owner.thread);
    return true;
}

serve :: (thread: *Thread) -> s64 {
    while true {
        event: X.XEvent;
        X.XNextEvent(owner.display, *event);
        if event.type == {
            case X.SelectionRequest;
                answer(*event.xselectionrequest);
            case X.SelectionClear;
                lock(*owner.mutex);
                owner.owned = false;
                unlock(*owner.mutex);
        }
    }
    return 0;
}

answer :: (request: *X.XSelectionRequestEvent) {
    using owner.atoms;
    reply: X.XEvent;
    reply.xselection.type      = X.SelectionNotify;
    reply.xselection.display   = request.display;
    reply.xselection.requestor = request.requestor;
    reply.xselection.selection = request.selection;
    reply.xselection.target    = request.target;
    reply.xselection.time      = request.time;
    // An obsolete client leaves the property unset and means the target.
    property := ifx request.property == X.None then request.target else request.property;
    reply.xselection.property = property;

    lock(*owner.mutex);
    if request.selection != CLIPBOARD || !owner.owned {
        reply.xselection.property = X.None;
    } else if request.target == TARGETS {
        offered := X.Atom.[TARGETS, UTF8_STRING, X.XA_STRING, TEXT, TEXT_PLAIN];
        X.XChangeProperty(owner.display, request.requestor, property, X.XA_ATOM, 32, X.PropModeReplace, cast(*u8) offered.data, offered.count);
    } else if request.target == UTF8_STRING || request.target == X.XA_STRING || request.target == TEXT || request.target == TEXT_PLAIN {
        X.XChangeProperty(owner.display, request.requestor, property, request.target, 8, X.PropModeReplace, owner.text.data, cast(s32) owner.text.count);
    } else {
        reply.xselection.property = X.None;
    }
    unlock(*owner.mutex);

    X.XSendEvent(owner.display, request.requestor, X.False, 0, *reply);
    X.XFlush(owner.display);
}
