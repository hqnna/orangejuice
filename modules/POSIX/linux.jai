//
// What POSIX is on Linux: the flags the kernel numbers, the layouts it fixes,
// and the sizes glibc gives its opaque objects.
//
// The generated binding sits behind this — `generated.jai` is produced from
// glibc's own headers by `tools/cbind`, which is where everything not
// declared by hand comes from.
//

#load "generated.jai";

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
O_DIRECTORY :: 0x10000;

// ------------------------------------------------------------ file facts ---

// @Volatile: x86-64 Linux. `struct stat` is arranged by the kernel and this
// mirrors it field for field; the padding is part of the layout.
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
    // Each is a `timespec` — seconds and nanoseconds — laid out the way the
    // kernel writes it.
    st_atime:   timespec;
    st_mtime:   timespec;
    st_ctime:   timespec;
    __glibc_reserved: [3] s64;
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
// types are opaque blocks the library sizes; these are the x86-64 sizes and
// are @Volatile with glibc.
pthread_t :: u64;

pthread_attr_t  :: struct { opaque: [56] u8; }
pthread_mutex_t :: struct { opaque: [40] u8; }
pthread_cond_t  :: struct { opaque: [48] u8; }
