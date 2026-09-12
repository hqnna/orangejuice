//
// What POSIX is on macOS: the flags Darwin numbers, the layouts it fixes, the
// sizes libSystem gives its opaque objects, and the handful of C library
// entry points the portable half of the module leans on.
//
// The Linux side of this gets most of that from a generated binding of
// glibc's headers. There is no generated Darwin binding: what the standard
// library actually calls is small enough to declare, and every declaration
// here is against the signature the manual page gives.
//
// Everything is the 64-bit-inode, non-`$INODE64` form, which is the only form
// arm64 Darwin has: `stat` and `readdir` are themselves rather than the
// suffixed symbols an x86-64 Mac binds.
//

#scope_export

// ---------------------------------------------------------- file handles ---

O_RDONLY    :: 0x0;
O_WRONLY    :: 0x1;
O_RDWR      :: 0x2;
O_NONBLOCK  :: 0x0004;
O_APPEND    :: 0x0008;
O_CREAT     :: 0x0200;
O_TRUNC     :: 0x0400;
O_EXCL      :: 0x0800;
O_DIRECTORY :: 0x100000;
O_CLOEXEC   :: 0x1000000;

// ------------------------------------------------------------ file facts ---

// @Volatile: arm64 Darwin. `struct stat` is arranged by the kernel and this
// mirrors it field for field; the padding is part of the layout. This is the
// `_DARWIN_FEATURE_64_BIT_INODE` shape, which is the only one arm64 has.
stat_t :: struct {
    st_dev:       s32;
    st_mode:      u16;
    st_nlink:     u16;
    st_ino:       u64;
    st_uid:       u32;
    st_gid:       u32;
    st_rdev:      s32;
    __pad0:       s32;
    st_atime:     timespec;
    st_mtime:     timespec;
    st_ctime:     timespec;
    st_birthtime: timespec;
    st_size:      s64;
    st_blocks:    s64;
    st_blksize:   s32;
    st_flags:     u32;
    st_gen:       u32;
    st_lspare:    s32;
    st_qspare:    [2] s64;
}

// @Volatile: arm64 Darwin. `d_name` is a whole `MAXPATHLEN`, not the 256
// bytes Linux gives it.
dirent :: struct {
    d_ino:     u64;
    d_seekoff: u64;
    d_reclen:  u16;
    d_namlen:  u16;
    d_type:    DT;
    d_name:    [1024] u8;
}

// The file types `dirent.d_type` names. BSD numbered these and Linux took the
// numbers, so both systems agree.
DT :: enum u8 {
    UNKNOWN :: 0;
    FIFO    :: 1;
    CHR     :: 2;
    DIR     :: 4;
    BLK     :: 6;
    REG     :: 8;
    LNK     :: 10;
    SOCK    :: 12;
    WHT     :: 14;
}

S_ISDIR :: inline (mode: u32) -> bool {
    return (mode & S_IFMT) == S_IFDIR;
}

// --------------------------------------------------------------- errors ---

EPERM     :: 1;
ENOENT    :: 2;
EINTR     :: 4;
EIO       :: 5;
EBADF     :: 9;
ENOMEM    :: 12;
EACCES    :: 13;
EEXIST    :: 17;
ENOTDIR   :: 20;
EISDIR    :: 21;
EINVAL    :: 22;
ERANGE    :: 34;
// Darwin numbers `EAGAIN` 35 where Linux numbers it 11, and 11 is `EDEADLK`
// here — which is why this cannot be shared.
EAGAIN      :: 35;
EWOULDBLOCK :: EAGAIN;
ETIMEDOUT   :: 60;
ELOOP       :: 62;

// The reference's own name for the value `errno` holds, so that a signature
// says what the number means rather than that it is an `s32`.
OS_Error_Code :: #type,isa s32;

// Darwin keeps `errno` thread-local behind `__error`, which is the same
// arrangement glibc makes with a different name.
__error :: () -> *s32 #foreign libc;

errno_location :: inline () -> *s32 #no_context {
    return __error();
}

set_errno :: inline (value: OS_Error_Code) #no_context {
    (cast(*OS_Error_Code) __error()).* = value;
}

// ---------------------------------------------------------------- memory ---

MAP_SHARED  :: 0x0001;
MAP_PRIVATE :: 0x0002;

// Darwin's `MAP_ANON`, which is not the number Linux gives `MAP_ANONYMOUS`.
MAP_ANON      :: 0x1000;
MAP_ANONYMOUS :: MAP_ANON;

MAP_FAILED :: -1;

// Linux's "fault the pages in now" hint. Darwin has no equivalent, and a flag
// nobody asked for is no flag at all.
MAP_POPULATE :: 0x0;

// --------------------------------------------------------------- sockets ---

AF_INET6     :: 30;
SOL_SOCKET   :: 0xFFFF;
SO_REUSEADDR :: 0x0004;

// @Volatile: `sockaddr_in` is fixed by the kernel. BSD put a length byte
// where Linux has the high half of a 16-bit family, so the struct is the same
// size and the family is a byte. The address and port are network byte order,
// which is big-endian whatever the machine is.
sockaddr_in :: struct {
    sin_len:    u8;
    sin_family: u8;
    sin_port:   u16;
    sin_addr:   u32;
    sin_zero:   [8] u8;
}

// --------------------------------------------------------------- threads ---

// `pthread_t` is a pointer. The attribute, mutex and condition types are
// opaque blocks the library sizes: a signature word and Darwin's own
// `__PTHREAD_*_SIZE__` bytes behind it. @Volatile with libSystem.
pthread_t :: u64;

pthread_attr_t      :: struct { opaque: [64] u8; }
pthread_mutex_t     :: struct { opaque: [64] u8; }
pthread_cond_t      :: struct { opaque: [48] u8; }
pthread_mutexattr_t :: struct { opaque: [16] u8; }

// Darwin numbers a recursive mutex 2 where glibc numbers it 1, so the name
// glibc spells `_NP` is a different value here.
PTHREAD_MUTEX :: enum s32 {
    NORMAL     :: 0;
    ERRORCHECK :: 1;
    RECURSIVE  :: 2;
    DEFAULT    :: NORMAL;

    // glibc's spellings of the same three, so that a program written against
    // either set of names compiles.
    TIMED_NP      :: NORMAL;
    ERRORCHECK_NP :: ERRORCHECK;
    RECURSIVE_NP  :: RECURSIVE;
}

pthread_mutexattr_init    :: (attributes: *pthread_mutexattr_t) -> s32 #foreign pthread;
pthread_mutexattr_destroy :: (attributes: *pthread_mutexattr_t) -> s32 #foreign pthread;
pthread_mutexattr_settype :: (attributes: *pthread_mutexattr_t, kind: s32) -> s32 #foreign pthread;

pthread_cond_timedwait :: (cond: *pthread_cond_t, mutex: *pthread_mutex_t, deadline: *timespec) -> s32 #foreign pthread;

// ------------------------------------------------------------------ time ---

// Darwin's `clockid_t`. The numbers are its own: a monotonic clock is 6 here
// and 1 on Linux.
clockid_t :: enum u32 {
    REALTIME                 :: 0;
    MONOTONIC                :: 6;
    MONOTONIC_RAW            :: 4;
    MONOTONIC_RAW_APPROX     :: 5;
    UPTIME_RAW               :: 8;
    UPTIME_RAW_APPROX        :: 9;
    PROCESS_CPUTIME_ID       :: 12;
    THREAD_CPUTIME_ID        :: 16;

    CLOCK_REALTIME           :: REALTIME;
    CLOCK_MONOTONIC          :: MONOTONIC;
    CLOCK_MONOTONIC_RAW      :: MONOTONIC_RAW;
    CLOCK_PROCESS_CPUTIME_ID :: PROCESS_CPUTIME_ID;
    CLOCK_THREAD_CPUTIME_ID  :: THREAD_CPUTIME_ID;
}

// `suseconds_t` is 32 bits on Darwin, so the struct is padded out rather than
// two words.
timeval :: struct {
    tv_sec:  s64;
    tv_usec: s32;
    _pad:    s32;
}

clock_t :: u64;

clock :: () -> clock_t #foreign libc;
time  :: (store: *time_t) -> time_t #foreign libc;
sleep :: (seconds: u32) -> u32 #foreign libc;

// --------------------------------------------------------------- the C io ---

// `FILE` is opaque: only its address is ever passed.
FILE :: struct {}

fopen  :: (path: *u8, mode: *u8) -> *FILE #foreign libc;
fclose :: (stream: *FILE) -> s32 #foreign libc;
fread  :: (buffer: *void, size: u64, count: u64, stream: *FILE) -> u64 #foreign libc;
fwrite :: (buffer: *void, size: u64, count: u64, stream: *FILE) -> u64 #foreign libc;
fseek  :: (stream: *FILE, offset: s64, whence: s32) -> s32 #foreign libc;
ftell  :: (stream: *FILE) -> s64 #foreign libc;
feof   :: (stream: *FILE) -> s32 #foreign libc;
puts   :: (text: *u8) -> s32 #foreign libc;

// Darwin's `off_t` has always been 64 bits, so there is no `_64` form of the
// call to bind: the plain one is it.
ftello64 :: (stream: *FILE) -> s64 #foreign libc "ftello";

// ------------------------------------------------------- the file tree ---

access  :: (path: *u8, mode: s32) -> s32 #foreign libc;
link    :: (from: *u8, to: *u8) -> s32 #foreign libc;
symlink :: (target: *u8, path: *u8) -> s32 #foreign libc;

pread  :: (fd: s32, buffer: *void, count: u64, offset: s64) -> s64 #foreign libc;
pwrite :: (fd: s32, buffer: *void, count: u64, offset: s64) -> s64 #foreign libc;

// ------------------------------------------------------------ polling ---

POLLIN  :: 0x0001;
POLLOUT :: 0x0004;
POLLERR :: 0x0008;
POLLHUP :: 0x0010;

nfds_t :: u32;

pollfd :: struct {
    fd:      s32;
    events:  s16;
    revents: s16;
}

poll :: (fds: *pollfd, count: nfds_t, timeout: s32) -> s32 #foreign libc;

// ------------------------------------------------------------- the system ---

gethostname :: (name: *u8, length: u64) -> s32 #foreign libc;

uid_t :: u32;
gid_t :: u32;
pid_t :: s32;

size_t  :: u64;
ssize_t :: s64;

// @Volatile: arm64 Darwin. BSD's `struct passwd` carries three members
// glibc's does not.
passwd :: struct {
    pw_name:   *u8;
    pw_passwd: *u8;
    pw_uid:    uid_t;
    pw_gid:    gid_t;
    pw_change: s64;
    pw_class:  *u8;
    pw_gecos:  *u8;
    pw_dir:    *u8;
    pw_shell:  *u8;
    pw_expire: s64;
}

getuid     :: () -> uid_t #foreign libc;
getpwuid_r :: (uid: uid_t, entry: *passwd, buffer: *u8, size: u64, result: **passwd) -> s32 #foreign libc;
getpwnam_r :: (name: *u8, entry: *passwd, buffer: *u8, size: u64, result: **passwd) -> s32 #foreign libc;

signal :: (sig: s32, handler: *void) -> *void #foreign libc;

wait :: (status: *s32) -> pid_t #foreign libc;

// What `sysconf` answers about. Darwin's numbers, which are its own.
using _SC_definitions :: enum s32 {
    _SC_PAGESIZE           :: 29;
    _SC_NPROCESSORS_CONF   :: 57;
    _SC_NPROCESSORS_ONLN   :: 58;
    _SC_GETPW_R_SIZE_MAX   :: 71;
    _SC_HOST_NAME_MAX      :: 72;
}

// ------------------------------------------------------------- the vector ---

iovec :: struct {
    iov_base: *void;
    iov_len:  size_t;
}
