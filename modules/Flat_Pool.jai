//
// Flat_Pool: a bump allocator over one growing chain of blocks.
//
// Nothing is freed one piece at a time — `reset` gives everything back at
// once. That is the whole point: allocation is a pointer bump, and a phase of
// a program that allocates a great deal and then finishes pays nothing to
// clean up.
//
// A marker taken with `get_mark` and given to `set_mark` unwinds to a point,
// which is what makes it usable as a scratch arena inside a longer-lived one.
//

#import "Basic";
#import "Memory";

Flat_Pool :: struct {
    BLOCK_SIZE :: 65536;

    Block :: struct {
        next: *Block;
        size: s64;
        // The bytes follow the header.
    }

    first:   *Block;
    current: *Block;
    // How much of `current` is spoken for.
    occupied: s64;

    allocator: Allocator;
    alignment: s64 = 8;
}

Flat_Pool_Mark :: struct {
    block:    *Flat_Pool.Block;
    occupied: s64;
}

init :: (pool: *Flat_Pool, allocator := context.allocator, alignment := 8) {
    pool.first = null;
    pool.current = null;
    pool.occupied = 0;
    pool.allocator = allocator;
    pool.alignment = alignment;
}

// Keeps the blocks and starts filling them again, so a pool reset every frame
// allocates from the operating system only while it is still growing.
reset :: (pool: *Flat_Pool) {
    pool.current = pool.first;
    pool.occupied = 0;
}

// Gives every block back.
deinit :: (pool: *Flat_Pool) {
    block := pool.first;
    while block {
        next := block.next;
        free(block, pool_allocator(pool));
        block = next;
    }
    pool.first = null;
    pool.current = null;
    pool.occupied = 0;
}

get :: (pool: *Flat_Pool, size: s64) -> *void {
    if size <= 0  return null;

    wanted := align_forward(size, pool.alignment);
    if !pool.current || pool.occupied + wanted > pool.current.size {
        if !pool_advance(pool, wanted)  return null;
    }

    memory := block_data(pool.current) + pool.occupied;
    pool.occupied += wanted;
    return memory;
}

get_mark :: (pool: *Flat_Pool) -> Flat_Pool_Mark {
    return .{pool.current, pool.occupied};
}

// Unwinds to a mark. Everything handed out since is invalid afterwards.
set_mark :: (pool: *Flat_Pool, mark: Flat_Pool_Mark) {
    pool.current = mark.block;
    pool.occupied = mark.occupied;
}

// So a pool can be handed to anything taking an `Allocator`.
flat_pool_allocator_proc :: (mode: Allocator_Mode, requested_size: s64, old_size: s64, old_memory: *void, allocator_data: *void) -> *void {
    pool := cast(*Flat_Pool) allocator_data;
    if #complete mode == {
        case .ALLOCATE;
            return get(pool, requested_size);

        case .RESIZE;
            memory := get(pool, requested_size);
            if memory && old_memory && old_size > 0 {
                memcpy(memory, old_memory, min(old_size, requested_size));
            }
            return memory;

        // A pool gives back everything or nothing.
        case .FREE;
            return null;

        case .CAPS;
            caps := Allocator_Caps.HINT_I_AM_A_FAST_BUMP_ALLOCATOR;
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

allocator :: (pool: *Flat_Pool) -> Allocator {
    return .{flat_pool_allocator_proc, pool};
}

#scope_module

block_data :: (block: *Flat_Pool.Block) -> *u8 {
    return cast(*u8) block + size_of(Flat_Pool.Block);
}

pool_allocator :: (pool: *Flat_Pool) -> Allocator {
    if pool.allocator.proc  return pool.allocator;
    return context.allocator;
}

// Moves to the next block that can hold `wanted`, taking a new one from the
// allocator when the chain runs out. A block kept from before a `reset` is
// reused rather than freed.
pool_advance :: (pool: *Flat_Pool, wanted: s64) -> bool {
    if pool.current && pool.current.next && pool.current.next.size >= wanted {
        pool.current = pool.current.next;
        pool.occupied = 0;
        return true;
    }

    size := max(wanted, Flat_Pool.BLOCK_SIZE);
    memory := alloc(size_of(Flat_Pool.Block) + size, pool_allocator(pool));
    if !memory  return false;

    block := cast(*Flat_Pool.Block) memory;
    block.next = null;
    block.size = size;

    if pool.current {
        // A block already chained here is dropped rather than leaked: it was
        // too small, and the chain stays in the order it was made.
        block.next = pool.current.next;
        pool.current.next = block;
    } else {
        pool.first = block;
    }
    pool.current = block;
    pool.occupied = 0;
    pool.allocator = pool_allocator(pool);
    return true;
}

// The name this module answers to when an allocator is asked what it is.
CAPS_VERSION_STRING :: "modules/Flat_Pool";

// The older spelling of `deinit`.
fini :: (pool: *Flat_Pool) {
    deinit(pool);
}
