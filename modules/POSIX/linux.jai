//
// What POSIX is on Linux: the flags the kernel numbers, the layouts it fixes,
// and the sizes glibc gives its opaque objects.
//
// Some of that is per *architecture* rather than per system. `struct stat` is
// arranged differently on arm64 than on x86-64, glibc sizes its opaque pthread
// objects differently, and arm64 overrides `O_DIRECTORY` — so those branch on
// `CPU` below. `the_posix_declarations_match_the_system_headers` in
// `crates/driver/tests/distribution.rs` measures every one of them against the
// system's own headers, on whichever machine it runs.
//
// The generated binding sits behind this — `generated.jai` is produced from
// glibc's own headers by `tools/cbind`, which is where everything not
// declared by hand comes from.
//

// The generated glibc binding, which is the *architecture's* and not just the
// system's: 299 of its declarations differ between the two. Most are syscall
// numbers — `SYS_exit` is 60 on x86-64 and 93 on arm64 — but `__nlink_t` is
// `u64` against `u32`, `__blksize_t` is `s64` against `s32`, `O_NOFOLLOW` and
// `O_DIRECT` are different numbers, and the pthread attribute types and the
// whole machine-context family are laid out differently.
#if CPU == .ARM64 {
    #load "generated_arm64.jai";
} else {
    #load "generated_x64.jai";
}

#scope_export

// ---------------------------------------------------------- file handles ---

O_RDONLY    :: 0x0;
O_WRONLY    :: 0x1;
O_RDWR      :: 0x2;
O_CREAT     :: 0x40;
O_EXCL      :: 0x80;
O_TRUNC     :: 0x200;
O_APPEND    :: 0x400;
O_NONBLOCK  :: 0x800;

// arm64 is one of the architectures that overrides the generic value.
#if CPU == .ARM64 {
    O_DIRECTORY :: 0x4000;
} else {
    O_DIRECTORY :: 0x10000;
}

// ------------------------------------------------------------ file facts ---

// @Volatile: `struct stat` is arranged by the kernel and this mirrors it field
// for field; the padding is part of the layout. The two architectures do not
// arrange it the same way — x86-64 puts `st_nlink` before `st_mode` and gives
// the whole thing 144 bytes, where arm64 takes the asm-generic shape and 128.
#if CPU == .ARM64 {
    stat_t :: struct {
        st_dev:     u64;
        st_ino:     u64;
        st_mode:    u32;
        st_nlink:   u32;
        st_uid:     u32;
        st_gid:     u32;
        st_rdev:    u64;
        __pad1:     u64;
        st_size:    s64;
        st_blksize: s32;
        __pad2:     s32;
        st_blocks:  s64;
        // Each is a `timespec` — seconds and nanoseconds — laid out the way
        // the kernel writes it.
        st_atime:   timespec;
        st_mtime:   timespec;
        st_ctime:   timespec;
        __unused:   [2] s32;
    }
} else {
    stat_t :: struct {
        st_dev:     u64;
        st_ino:     u64;
        st_nlink:   u64;
        st_mode:    u32;
        st_uid:     u32;
        st_gid:     u32;
        __pad0:     u32;
        st_rdev:    u64;
        st_size:    s64;
        st_blksize: s64;
        st_blocks:  s64;
        st_atime:   timespec;
        st_mtime:   timespec;
        st_ctime:   timespec;
        __glibc_reserved: [3] s64;
    }
}

// @Volatile: x86-64 Linux.
dirent :: struct {
    d_ino:    u64;
    d_off:    s64;
    d_reclen: u16;
    d_type:   DT;
    d_name:   [256] u8;
}

// --------------------------------------------------------------- errors ---

EPERM   :: 1;
ENOENT  :: 2;
EINTR   :: 4;
EIO     :: 5;
EBADF   :: 9;
EAGAIN  :: 11;
ENOMEM  :: 12;
EACCES  :: 13;
EEXIST  :: 17;
ENOTDIR :: 20;
EISDIR  :: 21;
EINVAL  :: 22;

// glibc keeps `errno` thread-local behind a function, so it cannot simply be
// declared as a global here. `__errno_location` is the name glibc gives it,
// and `errno_location` is what the portable half of the module asks for.
__errno_location :: () -> *s32 #foreign libc;

errno_location :: inline () -> *s32 #no_context {
    return __errno_location();
}

// ---------------------------------------------------------------- memory ---

MAP_PRIVATE   :: 0x02;
MAP_ANONYMOUS :: 0x20;
MAP_SHARED    :: 0x01;

// --------------------------------------------------------------- sockets ---

AF_INET6     :: 10;
SOL_SOCKET   :: 1;
SO_REUSEADDR :: 2;

// @Volatile: `sockaddr_in` is fixed by the kernel. The address and port are
// network byte order, which is big-endian whatever the machine is.
sockaddr_in :: struct {
    sin_family: u16;
    sin_port:   u16;
    sin_addr:   u32;
    sin_zero:   [8] u8;
}

// --------------------------------------------------------------- threads ---

// `pthread_t` is a pointer-sized handle on Linux. The attribute and mutex
// types are opaque blocks the library sizes, and glibc does not size them the
// same on both architectures — `__SIZEOF_PTHREAD_MUTEX_T` is 40 on x86-64 and
// 48 on arm64. @Volatile with glibc.
pthread_t :: u64;

pthread_cond_t :: struct { opaque: [48] u8; }

#if CPU == .ARM64 {
    pthread_attr_t  :: struct { opaque: [64] u8; }
    pthread_mutex_t :: struct { opaque: [48] u8; }
} else {
    pthread_attr_t  :: struct { opaque: [56] u8; }
    pthread_mutex_t :: struct { opaque: [40] u8; }
}
