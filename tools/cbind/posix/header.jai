//
// The glibc surface: the types, constants, procedures and globals that
// `POSIX/module.jai` and `POSIX/linux.jai` do not declare by hand.
//
// This file is Linux's alone. `POSIX/module.jai` loads it only under
// `#if OS == .LINUX`, beside `linux.jai`; the Darwin side of the module is
// `macos.jai`, which has no generated binding behind it.
//
// Generated from the system's own C headers rather than typed out, the way the
// reference generates its bindings — a constant's value is whatever
// `<fcntl.h>` says it is on the machine the compiler was built on, and a
// procedure's parameter names are the ones glibc gave them. What is here is
// therefore a projection of the C ABI, not a transcription of anybody's file.
//
// The generator lives in `tools/cbind`; `tools/cbind/README.md` says how to
// run it again.
//

#scope_export

