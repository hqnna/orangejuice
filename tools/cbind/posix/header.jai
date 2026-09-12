//
// The glibc surface: the types, constants, procedures and globals that
// `POSIX/module.jai` and `POSIX/linux.jai` do not declare by hand.
//
// This file is one Linux architecture's. `POSIX/linux.jai` loads it under
// `#if CPU ==`, because 299 of these declarations differ between x86-64 and
// arm64 — the syscall numbers above all, but also `__nlink_t`, `__blksize_t`,
// `O_NOFOLLOW`, `O_DIRECT` and the pthread and machine-context layouts.
//
// The Darwin side of the module is `macos.jai`, which has no generated
// binding behind it.
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

