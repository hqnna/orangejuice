//
// The part of libc orangejuice's own modules actually call.
//
// The reference distribution vendors a generated binding of the whole POSIX
// surface — twenty-five thousand lines of it. Nothing here needs that: the
// standard library wants to allocate, write bytes to a file descriptor, ask
// the clock what time it is and stop the process. Those are declared by hand,
// against the C library's own documented signatures.
//

#scope_export

#load "generated.jai";

// ------------------------------------------------------------- allocation ---

malloc  :: (size: u64) -> *void #foreign libc;
calloc  :: (count: u64, size: u64) -> *void #foreign libc;
realloc :: (memory: *void, size: u64) -> *void #foreign libc;
free    :: (memory: *void) #foreign libc;

// glibc's aligned allocation. `alignment` must be a power of two and a
// multiple of the pointer size; `size` must be a multiple of `alignment`.
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

// ------------------------------------------------------------------ time ---


timespec :: struct {
    tv_sec:  s64;
    tv_nsec: s64;
}


// ---------------------------------------------------------------- memory ---

PROT_READ  :: 0x1;
PROT_WRITE :: 0x2;

MAP_PRIVATE   :: 0x02;
MAP_ANONYMOUS :: 0x20;

mmap   :: (address: *void, length: u64, protection: s32, flags: s32, fd: s32, offset: s64) -> *void #foreign libc;
munmap :: (address: *void, length: u64) -> s32 #foreign libc;

#scope_module

libc :: #library,system "libc";
