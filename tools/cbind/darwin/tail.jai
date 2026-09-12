
// ------------------------------------------------------- the hand-written ---
//
// C shapes the generator does not cover, written out here.

// `EV_SET` is a macro, so there is nothing in the headers for the generator to
// read. It fills in a `kevent` the way `<sys/event.h>` spells it.
EV_SET :: inline (event: *kevent, ident: u64, filter: s16, flags: u16, fflags: u32, data: s64, udata: *void) {
    event.ident  = ident;
    event.filter = filter;
    event.flags  = flags;
    event.fflags = fflags;
    event.data   = data;
    event.udata  = udata;
}
