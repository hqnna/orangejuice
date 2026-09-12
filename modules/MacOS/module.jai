//
// The part of libSystem orangejuice's own modules actually call.
//
// The sibling of `Linux`, for the other Unix orangejuice targets: the same
// surface, declared against the same documented C signatures, with the values
// Darwin gives the constants that are not the same on both. The standard
// library wants to allocate, write bytes to a file descriptor, map pages and
// stop the process, and that is all this is.
//
// macOS has no `libc` of its own to link: every one of these lives in
// `libSystem`, which is what `-lc` resolves to there.
//

#scope_export

// ------------------------------------------------------------- allocation ---

malloc  :: (size: u64) -> *void #foreign libc;
calloc  :: (count: u64, size: u64) -> *void #foreign libc;
realloc :: (memory: *void, size: u64) -> *void #foreign libc;
free    :: (memory: *void) #foreign libc;

// C11 aligned allocation, which Darwin has had since macOS 10.15.
// `alignment` must be a power of two and a multiple of the pointer size;
// `size` must be a multiple of `alignment`.
aligned_alloc :: (alignment: u64, size: u64) -> *void #foreign libc;

// ---------------------------------------------------------------- output ---

STDIN_FILENO  :: 0;
STDOUT_FILENO :: 1;
STDERR_FILENO :: 2;

// Returns how many bytes it took, or -1. A short write is not an error, so a
// caller writing a whole buffer has to loop.
write :: (fd: s32, buffer: *void, count: u64) -> s64 #foreign libc;
read  :: (fd: s32, buffer: *void, count: u64) -> s64 #foreign libc;

// ----------------------------------------------------------- the process ---

exit :: (status: s32) #foreign libc;

// --------------------------------------------------------------- signals ---

SIGTRAP :: 5;

raise :: (signal: s32) -> s32 #foreign libc;

// ------------------------------------------------------------------ time ---

timespec :: struct {
    tv_sec:  s64;
    tv_nsec: s64;
}

// ---------------------------------------------------------------- memory ---

PROT_READ  :: 0x1;
PROT_WRITE :: 0x2;

MAP_PRIVATE :: 0x0002;

// Darwin's `MAP_ANON`, which is not the number Linux gives `MAP_ANONYMOUS`.
MAP_ANONYMOUS :: 0x1000;

mmap   :: (address: *void, length: u64, protection: s32, flags: s32, fd: s32, offset: s64) -> *void #foreign libc;
munmap :: (address: *void, length: u64) -> s32 #foreign libc;

#scope_module

libc :: #library,system "libc";
