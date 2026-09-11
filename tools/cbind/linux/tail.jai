
// ---------------------------------------------------------- hand-written ---

// The three io_uring calls have no libc wrappers: a program makes the syscall
// itself. The numbers are the kernel's.
//
// `syscall` is C-variadic, so each argument travels on its own rather than
// gathered into a slice (**L§7.11**).
syscall :: (number: s64, args: ..Any) -> s64 #foreign libc;
SYS_io_uring_setup    :: 425;
SYS_io_uring_enter    :: 426;
SYS_io_uring_register :: 427;

io_uring_setup :: (entries: u32, params: *io_uring_params) -> s32 {
    return cast(s32) syscall(SYS_io_uring_setup, entries, params);
}

io_uring_enter :: (fd: s32, to_submit: u32, min_complete: u32, flags: u32, sig: *void, sz: s64) -> s32 {
    return cast(s32) syscall(SYS_io_uring_enter, fd, to_submit, min_complete, flags, sig, sz);
}

io_uring_register :: (fd: s32, opcode: u32, arg: *void, nr_args: u32) -> s32 {
    return cast(s32) syscall(SYS_io_uring_register, fd, opcode, arg, nr_args);
}

// `epoll_ctl`'s operation, which the header spells as three `#define`s.
Epoll_Ctl :: enum s32 {
    ADD :: 1;
    DEL :: 2;
    MOD :: 3;
}

// Mach's "no error", which the reference carries for the platforms that have
// it; zero everywhere.
KERN_SUCCESS :: 0;

// The read/write flags an io_uring submission carries.
__kernel_rwf_t :: s32;

// `<sys/epoll.h>` defines these two outside the event enum.
EPOLLNVAL        :: 0x20;
EPOLL_URING_WAKE :: 0x8000_0000;
