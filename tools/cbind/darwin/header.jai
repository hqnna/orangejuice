//
// The Darwin surface: the types, constants, procedures and globals that
// `Darwin/module.jai` does not declare by hand.
//
// What is here is what only Darwin has — kqueue, `sysctl`, the Mach clock,
// dyld — the way `Linux/generated.jai` holds what only Linux has. The C
// library surface both systems share is `POSIX`.
//
// Generated from the system's own headers rather than typed out, the way the
// reference generates its bindings — a constant's value is whatever
// `<sys/event.h>` says it is on the machine the compiler was built on, and a
// procedure's parameter names are the ones the SDK gave them. What is here is
// therefore a projection of the C ABI, not a transcription of anybody's file.
//
// The generator lives in `tools/cbind`; `tools/cbind/README.md` says how to
// run it again.
//

#scope_export
