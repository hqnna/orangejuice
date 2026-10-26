//
// Unmapping_Allocator: a debugging allocator that gives every allocation
// pages of its own and unmaps them when it is freed.
//
// A block is placed at the *end* of its mapping, with an inaccessible guard
// page after it, so a write past the end faults at once rather than
// corrupting a neighbour, and reading or writing a block after it was freed
// faults too, because the pages are gone. A resize always moves, so a pointer
// kept across one is caught the same way. What that costs is a page and a
// system call or two per allocation: this is for finding a bug, not for
// shipping.
//
//     context.allocator = get_unmapping_allocator();
//
// The pointer handed out is 16-byte aligned, which leaves up to 15 bytes
// between the end of an odd-sized block and the guard page that an overrun
// can reach without faulting. The mapping's own address and size sit in the
// 16 bytes before the block.
//

#import "Basic";
POSIX :: #import "POSIX";

get_unmapping_allocator :: () -> Allocator {
    return .{unmapping_allocator_proc, null};
}

unmapping_allocator :: Allocator.{unmapping_allocator_proc, null};

unmapping_allocator_proc :: (mode: Allocator_Mode, requested_size: s64, old_size: s64, old_memory: *void, allocator_data: *void) -> *void {
    if #complete mode == {
        case .ALLOCATE;
            return map_block(requested_size);

        case .RESIZE;
            if requested_size <= 0 {
                unmap_block(old_memory);
                return null;
            }
            memory := map_block(requested_size);
            if memory && old_memory {
                memcpy(memory, old_memory, min(old_size, requested_size));
                unmap_block(old_memory);
            }
            return memory;

        case .FREE;
            unmap_block(old_memory);
            return null;

        case .CAPS;
            caps := Allocator_Caps.FREE | .ACTUALLY_RESIZE | .MULTIPLE_THREADS | .HINT_I_AM_A_DEBUG_ALLOCATOR;
            return cast(*void) caps;

        case .STARTUP;       #through;
        case .SHUTDOWN;      #through;
        case .THREAD_START;  #through;
        case .THREAD_STOP;   #through;
        case .CREATE_HEAP;   #through;
        case .DESTROY_HEAP;  #through;
        case .IS_THIS_YOURS;
            return null;
    }
}

// The size of a page on this machine, which is what a mapping is made of.
page_size :: () -> s64 {
    return cast(s64) getpagesize();
}

#scope_file

Header :: struct {
    base:   *void;
    length: s64;
}

ALIGNMENT :: 16;

map_block :: (size: s64) -> *void {
    if size <= 0  return null;

    page   := page_size();
    data   := (size + ALIGNMENT - 1) & ~(ALIGNMENT - 1);
    usable := ((data + size_of(Header) + page - 1) / page) * page;
    length := usable + page;

    base := POSIX.mmap(null, cast(u64) length, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if base == MAP_FAILED  return null;
    if mprotect((cast(*u8) base) + usable, cast(u64) page, PROT_NONE) != 0 {
        POSIX.munmap(base, cast(u64) length);
        return null;
    }

    memory := (cast(*u8) base) + usable - data;
    header := cast(*Header) (memory - size_of(Header));
    header.base   = base;
    header.length = length;
    return memory;
}

unmap_block :: (memory: *void) {
    if !memory  return;
    header := cast(*Header) ((cast(*u8) memory) - size_of(Header));
    POSIX.munmap(header.base, cast(u64) header.length);
}

// What POSIX fixes the same way on every system this targets.
PROT_NONE     :: 0;
PROT_READ     :: 1;
PROT_WRITE    :: 2;
MAP_PRIVATE   :: 0x02;
#if OS == .MACOS  MAP_ANONYMOUS :: 0x1000;
else              MAP_ANONYMOUS :: 0x20;
MAP_FAILED    :: cast(*void) -1;

libc :: #library,system "libc";
mprotect    :: (address: *void, length: u64, protection: s32) -> s32 #foreign libc;
getpagesize :: () -> s32 #foreign libc;
