//
// POSIX: the C library surface the standard library and its callers use.
//
// The reference vendors a generated binding of the whole of POSIX — 25,000
// lines. This is the part programs actually call, declared by hand against
// the documented signatures. Layouts that the kernel fixes (`stat`, `dirent`)
// are written for x86-64 Linux and marked, because they are wrong everywhere
// else.
//

#load "generated.jai";

#scope_export

// ------------------------------------------------------------- allocation ---

malloc  :: (size: u64) -> *void #foreign libc;
calloc  :: (count: u64, size: u64) -> *void #foreign libc;
realloc :: (memory: *void, size: u64) -> *void #foreign libc;
free    :: (memory: *void) #foreign libc;

// ---------------------------------------------------------- file handles ---

STDIN_FILENO  :: 0;
STDOUT_FILENO :: 1;
STDERR_FILENO :: 2;

O_RDONLY   :: 0x0;
O_WRONLY   :: 0x1;
O_RDWR     :: 0x2;
O_CREAT    :: 0x40;
O_EXCL     :: 0x80;
O_TRUNC    :: 0x200;
O_APPEND   :: 0x400;
O_NONBLOCK :: 0x800;
O_DIRECTORY :: 0x10000;

SEEK_SET :: 0;
SEEK_CUR :: 1;
SEEK_END :: 2;

// A short read or write is not an error, so a caller moving a whole buffer
// has to loop until it has.
open  :: (path: *u8, flags: s32, mode: u32 = 0) -> s32 #foreign libc;
close :: (fd: s32) -> s32 #foreign libc;
read  :: (fd: s32, buffer: *void, count: u64) -> s64 #foreign libc;
write :: (fd: s32, buffer: *void, count: u64) -> s64 #foreign libc;
lseek :: (fd: s32, offset: s64, whence: s32) -> s64 #foreign libc;
fsync :: (fd: s32) -> s32 #foreign libc;
pipe  :: (fds: *[2] s32) -> s32 #foreign libc;
dup2  :: (old: s32, new: s32) -> s32 #foreign libc;

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
    st_atime:     s64;
    st_atime_ns:  s64;
    st_mtime:     s64;
    st_mtime_ns:  s64;
    st_ctime:     s64;
    st_ctime_ns:  s64;
    __unused:   [3] s64;
}

S_IFMT   :: 0xF000;   // 0o170000
S_IFDIR  :: 0x4000;   // 0o040000
S_IFREG  :: 0x8000;   // 0o100000
S_IFLNK  :: 0xA000;   // 0o120000

stat  :: (path: *u8, result: *stat_t) -> s32 #foreign libc;
lstat :: (path: *u8, result: *stat_t) -> s32 #foreign libc;
fstat :: (fd: s32, result: *stat_t) -> s32 #foreign libc;

// ---------------------------------------------------------- the file tree ---

unlink :: (path: *u8) -> s32 #foreign libc;
rmdir  :: (path: *u8) -> s32 #foreign libc;
mkdir  :: (path: *u8, mode: u32) -> s32 #foreign libc;
rename :: (from: *u8, to: *u8) -> s32 #foreign libc;
chdir  :: (path: *u8) -> s32 #foreign libc;

getcwd   :: (buffer: *u8, size: u64) -> *u8 #foreign libc;
readlink :: (path: *u8, buffer: *u8, size: u64) -> s64 #foreign libc;
realpath :: (path: *u8, resolved: *u8) -> *u8 #foreign libc;

// @Volatile: x86-64 Linux.
dirent :: struct {
    d_ino:    u64;
    d_off:    s64;
    d_reclen: u16;
    d_type:   u8;
    d_name:   [256] u8;
}

DT_UNKNOWN :: 0;
DT_DIR     :: 4;
DT_REG     :: 8;
DT_LNK     :: 10;

DIR :: struct {}   // opaque

opendir  :: (path: *u8) -> *DIR #foreign libc;
readdir  :: (dir: *DIR) -> *dirent #foreign libc;
closedir :: (dir: *DIR) -> s32 #foreign libc;

// ------------------------------------------------------------ processes ---

fork    :: () -> s32 #foreign libc;
execvp  :: (file: *u8, argv: **u8) -> s32 #foreign libc;
waitpid :: (pid: s32, status: *s32, options: s32) -> s32 #foreign libc;
_exit   :: (status: s32) #foreign libc;
exit    :: (status: s32) #foreign libc;
system  :: (command: *u8) -> s32 #foreign libc;
getpid  :: () -> s32 #foreign libc;
getenv  :: (name: *u8) -> *u8 #foreign libc;

// What `waitpid` packs into its status word.
WIFEXITED :: (status: s32) -> bool {
    return (status & 0x7f) == 0;
}

WEXITSTATUS :: (status: s32) -> s32 {
    return (status >> 8) & 0xff;
}

WIFSIGNALED :: (status: s32) -> bool {
    // A low byte that is neither 0 (exited) nor 0x7f (stopped) is a signal
    // number; the +1 >> 1 is the idiomatic way to say "not 0 and not 0x7f".
    return ((status & 0x7f) + 1) >> 1 > 0;
}

WTERMSIG :: (status: s32) -> s32 {
    return status & 0x7f;
}

// waitpid options.
WNOHANG   :: 1;
WUNTRACED :: 2;

// -------------------------------------------------------------- signals ---

kill :: (pid: s32, signal: s32) -> s32 #foreign libc;

SIGINT  :: 2;
SIGKILL :: 9;
SIGTERM :: 15;

// ------------------------------------------------------ descriptor flags ---

fcntl :: (fd: s32, command: s32, argument: s32) -> s32 #foreign libc;

F_GETFD :: 1;
F_SETFD :: 2;
F_GETFL :: 3;
F_SETFL :: 4;

// --------------------------------------------------------------- errors ---

EPERM  :: 1;
ENOENT :: 2;
EINTR  :: 4;
EIO    :: 5;
EBADF  :: 9;
EAGAIN :: 11;
ENOMEM :: 12;
EACCES :: 13;
EEXIST :: 17;
ENOTDIR :: 20;
EISDIR :: 21;
EINVAL :: 22;

// glibc keeps `errno` thread-local behind a function, so it cannot simply be
// declared as a global here.
__errno_location :: () -> *s32 #foreign libc;

errno :: () -> s32 {
    return __errno_location().*;
}

strerror :: (code: s32) -> *u8 #foreign libc;

// ------------------------------------------------------------------ time ---

CLOCK_REALTIME  :: 0;
CLOCK_MONOTONIC :: 1;

timespec :: struct {
    tv_sec:  s64;
    tv_nsec: s64;
}

CLOCK_MONOTONIC_RAW :: 4;

time_t :: s64;

// C's broken-down time. The members are C's, oddities included: the year
// counts from 1900 and the month from zero.
tm :: struct {
    tm_sec:    s32;
    tm_min:    s32;
    tm_hour:   s32;
    tm_mday:   s32;
    tm_mon:    s32;
    tm_year:   s32;
    tm_wday:   s32;
    tm_yday:   s32;
    tm_isdst:  s32;
    _pad:      s32;
    tm_gmtoff: s64;
    tm_zone:   *u8;
}

clock_gettime :: (clock: s32, result: *timespec) -> s32 #foreign libc;
nanosleep     :: (request: *timespec, remaining: *timespec) -> s32 #foreign libc;

// The `_r` forms write into storage the caller owns, which is what makes them
// safe to call from more than one thread.
gmtime_r    :: (t: *time_t, result: *tm) -> *tm #foreign libc;
localtime_r :: (t: *time_t, result: *tm) -> *tm #foreign libc;

// The reverse: broken-down time back to a count of seconds, read as UTC by
// `timegm` and as local time by `timelocal`.
timegm    :: (parts: *tm) -> time_t #foreign libc;
timelocal :: (parts: *tm) -> time_t #foreign libc;

// ---------------------------------------------------------------- memory ---

PROT_READ  :: 0x1;
PROT_WRITE :: 0x2;

MAP_PRIVATE   :: 0x02;
MAP_ANONYMOUS :: 0x20;

mmap   :: (address: *void, length: u64, protection: s32, flags: s32, fd: s32, offset: s64) -> *void #foreign libc;
munmap :: (address: *void, length: u64) -> s32 #foreign libc;

#scope_module

libc :: #system_library "c";

#scope_export

// ---------------------------------------------------------- sockets ---

AF_INET     :: 2;
AF_INET6    :: 10;
SOCK_STREAM :: 1;
SOCK_DGRAM  :: 2;

SOL_SOCKET   :: 1;
SO_REUSEADDR :: 2;

SHUT_RD   :: 0;
SHUT_WR   :: 1;
SHUT_RDWR :: 2;

// @Volatile: `sockaddr_in` is fixed by the kernel. The address and port are
// network byte order, which is big-endian whatever the machine is.
sockaddr_in :: struct {
    sin_family: u16;
    sin_port:   u16;
    sin_addr:   u32;
    sin_zero:   [8] u8;
}

socket   :: (domain: s32, kind: s32, protocol: s32) -> s32 #foreign libc;
bind     :: (fd: s32, address: *sockaddr_in, length: u32) -> s32 #foreign libc;
listen   :: (fd: s32, backlog: s32) -> s32 #foreign libc;
accept   :: (fd: s32, address: *sockaddr_in, length: *u32) -> s32 #foreign libc;
connect  :: (fd: s32, address: *sockaddr_in, length: u32) -> s32 #foreign libc;
shutdown :: (fd: s32, how: s32) -> s32 #foreign libc;
setsockopt :: (fd: s32, level: s32, option: s32, value: *void, length: u32) -> s32 #foreign libc;
getsockname :: (fd: s32, address: *sockaddr_in, length: *u32) -> s32 #foreign libc;

send :: (fd: s32, buffer: *void, length: u64, flags: s32) -> s64 #foreign libc;
recv :: (fd: s32, buffer: *void, length: u64, flags: s32) -> s64 #foreign libc;

htons :: (value: u16) -> u16 #foreign libc;
ntohs :: (value: u16) -> u16 #foreign libc;
htonl :: (value: u32) -> u32 #foreign libc;
inet_addr :: (text: *u8) -> u32 #foreign libc;

// ------------------------------------------------- shared memory ---

shm_open   :: (name: *u8, flags: s32, mode: u32) -> s32 #foreign librt;
shm_unlink :: (name: *u8) -> s32 #foreign librt;
ftruncate  :: (fd: s32, length: s64) -> s32 #foreign libc;

MAP_SHARED :: 0x01;

#scope_module

// glibc folded librt into libc; naming libc works on a modern system.
librt :: #system_library "c";

#scope_export

// ------------------------------------------------------ file watching ---

// inotify: the kernel tells the process about changes rather than the process
// asking over and over.
inotify_init1  :: (flags: s32) -> s32 #foreign libc;
inotify_add_watch :: (fd: s32, path: *u8, mask: u32) -> s32 #foreign libc;
inotify_rm_watch  :: (fd: s32, watch: s32) -> s32 #foreign libc;

IN_NONBLOCK :: 0x800;

IN_ACCESS   :: 0x1;
IN_MODIFY   :: 0x2;
IN_ATTRIB   :: 0x4;
IN_CLOSE_WRITE :: 0x8;
IN_MOVED_FROM  :: 0x40;
IN_MOVED_TO    :: 0x80;
IN_CREATE   :: 0x100;
IN_DELETE   :: 0x200;
IN_DELETE_SELF :: 0x400;
IN_MOVE_SELF   :: 0x800;

// @Volatile: the kernel writes these back to back, each followed by `len`
// bytes of name.
inotify_event :: struct {
    wd:     s32;
    mask:   u32;
    cookie: u32;
    len:    u32;
}

// ----------------------------------------------------------- threads ---

// `pthread_t` is a pointer-sized handle on Linux. The attribute and mutex
// types are opaque blocks the library sizes; these are the x86-64 sizes and
// are @Volatile with glibc.
pthread_t :: u64;

pthread_attr_t :: struct { opaque: [56] u8; }
pthread_mutex_t :: struct { opaque: [40] u8; }
pthread_cond_t :: struct { opaque: [48] u8; }

pthread_create  :: (thread: *pthread_t, attributes: *pthread_attr_t, start: (*void) -> *void #c_call, argument: *void) -> s32 #foreign pthread;
pthread_join    :: (thread: pthread_t, result: **void) -> s32 #foreign pthread;
pthread_detach  :: (thread: pthread_t) -> s32 #foreign pthread;
pthread_self    :: () -> pthread_t #foreign pthread;

pthread_mutex_init    :: (mutex: *pthread_mutex_t, attributes: *void) -> s32 #foreign pthread;
pthread_mutex_destroy :: (mutex: *pthread_mutex_t) -> s32 #foreign pthread;
pthread_mutex_lock    :: (mutex: *pthread_mutex_t) -> s32 #foreign pthread;
pthread_mutex_trylock :: (mutex: *pthread_mutex_t) -> s32 #foreign pthread;
pthread_mutex_unlock  :: (mutex: *pthread_mutex_t) -> s32 #foreign pthread;

pthread_cond_init      :: (cond: *pthread_cond_t, attributes: *void) -> s32 #foreign pthread;
pthread_cond_destroy   :: (cond: *pthread_cond_t) -> s32 #foreign pthread;
pthread_cond_wait      :: (cond: *pthread_cond_t, mutex: *pthread_mutex_t) -> s32 #foreign pthread;
pthread_cond_signal    :: (cond: *pthread_cond_t) -> s32 #foreign pthread;
pthread_cond_broadcast :: (cond: *pthread_cond_t) -> s32 #foreign pthread;

// How many cores the process may run on.
sysconf :: (name: s32) -> s64 #foreign libc;
_SC_NPROCESSORS_ONLN :: 84;

#scope_module

// glibc folded libpthread into libc itself in 2.34; naming libc for both is
// what works on a modern system and does no harm on an older one.
pthread :: #system_library "c";
