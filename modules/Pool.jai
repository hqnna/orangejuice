//
// Pool: many allocations of many sizes, one lifetime.
//
// The pool hands out bytes from a large block and never takes them back one at
// a time — `free` on a pool allocator does nothing. When the work is finished
// the whole pool is `reset` (its blocks kept for reuse) or `release`d (its
// blocks given back). That is the right shape for a parse tree, a frame's
// worth of scratch, or anything else built up and thrown away together.
//
// An allocation at least `oversized_size` big gets a block of its own rather
// than being fitted into the current one, so one huge request cannot waste the
// rest of a block.
//
// `Flat_Pool` is the simpler thing to reach for when one contiguous region
// will do; this one grows by asking its `block_allocator` for another block.
//

#import "Basic";

POOL_BLOCK_SIZE_DEFAULT     :: 65536;
POOL_OVERSIZED_SIZE_DEFAULT :: 6554;

// What `overwrite_memory` fills freed bytes with — a value that is neither a
// plausible pointer nor a plausible small integer, so reading through a stale
// pointer goes wrong loudly rather than quietly.
STAMP :: 0xcc;

Pool :: struct {
    // The size of a block as asked of the allocator; a caller gets a little
    // less than this, since the block's own header lives in it.
    memblock_size:  s64 = POOL_BLOCK_SIZE_DEFAULT;
    oversized_size: s64 = POOL_OVERSIZED_SIZE_DEFAULT;
    // Must be a power of two.
    alignment := 8;

    current_memblock: *Memblock;
    current_pos:      *u8;
    bytes_left:       s64;

    block_allocator: Allocator;

    // Blocks a reset kept, blocks in use, and the oversized allocations that
    // never went through a block at all.
    unused_memblocks:        *Memblock;
    used_memblocks:          *Memblock;
    out_of_band_allocations: *Memblock;

    // For finding a pointer kept across a reset: the bytes are stamped rather
    // than left holding what they used to.
    overwrite_memory        := false;
    // For an allocator that wants its memory back promptly rather than
    // recycled here.
    free_memblocks_on_reset := false;
}

Memblock :: struct {
    next: *Memblock;
    // How big the allocation was, header included, so `release` can hand the
    // same number back to the allocator.
    total_size: s64;
}

// Says where the pool's blocks come from. Calling it is optional — the first
// `get` uses `context.allocator` — but a pool that is about to *become* the
// context allocator has to be told beforehand, or it would ask itself for its
// own first block.
set_allocators :: (pool: *Pool, block_allocator: Allocator = .{}) {
    if pool.block_allocator.proc {
        log_error("Attempt to set allocators on a Pool that has already been used.\n");
        return;
    }

    if block_allocator.proc {
        pool.block_allocator = block_allocator;
        return;
    }

    if context.allocator.proc == pool_allocator_proc && context.allocator.data == pool {
        log_error("A Pool is taking its block allocator from the context, but the context allocator is that same Pool, which would loop forever. Call set_allocators on the Pool before pushing it onto context.allocator.\n");
        pool.block_allocator = context.default_allocator;
        return;
    }

    pool.block_allocator = context.allocator;
}

// `nbytes` from the pool, aligned to `pool.alignment`. Null when the block
// allocator could not give the pool room.
get :: (pool: *Pool, nbytes: s64) -> *void {
    if nbytes <= 0  return null;
    if !pool.block_allocator.proc  set_allocators(pool);

    if nbytes >= pool.oversized_size  return pool_oversized(pool, nbytes);

    wanted := pool_align_up(nbytes, pool.alignment);
    if wanted > pool.bytes_left {
        if !pool_new_block(pool)  return null;
    }

    result := pool.current_pos;
    pool.current_pos += wanted;
    pool.bytes_left -= wanted;
    return result;
}

// Every byte the pool handed out is free again; the blocks stay, so the next
// round of allocations does not go back to the allocator.
reset :: (pool: *Pool) {
    // The current block goes on the used list first, so one loop covers all
    // of them.
    if pool.current_memblock {
        pool_push(*pool.used_memblocks, pool.current_memblock);
        pool.current_memblock = null;
    }
    pool.current_pos = null;
    pool.bytes_left = 0;

    pool_free_list(pool, *pool.out_of_band_allocations);

    if pool.free_memblocks_on_reset {
        pool_free_list(pool, *pool.used_memblocks);
        pool_free_list(pool, *pool.unused_memblocks);
        return;
    }

    block := pool.used_memblocks;
    while block {
        next := block.next;
        if pool.overwrite_memory  pool_stamp(block);
        pool_push(*pool.unused_memblocks, block);
        block = next;
    }
    pool.used_memblocks = null;
}

// A reset, and then the blocks go back to the allocator too.
release :: (pool: *Pool) {
    reset(pool);
    pool_free_list(pool, *pool.used_memblocks);
    pool_free_list(pool, *pool.unused_memblocks);
}

// The `Allocator_Proc` shape, so a pool can be the context allocator:
//
//     set_allocators(*pool);
//     context.allocator = .{pool_allocator_proc, *pool};
//
// A free is ignored, and a resize copies into fresh bytes, since the pool has
// no way to give the old ones back.
pool_allocator_proc :: (mode: Allocator_Mode, requested_size: s64, old_size: s64, old_memory: *void, allocator_data: *void) -> *void {
    pool := cast(*Pool) allocator_data;
    if !pool  return null;

    if mode == {
        case .ALLOCATE;
            return get(pool, requested_size);

        case .RESIZE;
            memory := get(pool, requested_size);
            if !memory  return null;
            keep := min(old_size, requested_size);
            if old_memory && keep > 0  memcpy(memory, old_memory, keep);
            return memory;

        case .FREE;
            return null;

        case;
            return null;
    }
}

#scope_module

pool_align_up :: (value: s64, alignment: s64) -> s64 {
    if alignment <= 1  return value;
    return (value + alignment - 1) & ~(alignment - 1);
}

pool_push :: (list: **Memblock, block: *Memblock) {
    block.next = list.*;
    list.* = block;
}

// A request too big to be worth fitting into a block gets exactly the block it
// needs, kept on its own list so it is freed by a reset rather than recycled.
pool_oversized :: (pool: *Pool, nbytes: s64) -> *void {
    header := pool_align_up(size_of(Memblock), pool.alignment);
    total := header + nbytes;

    memory := alloc(total, pool.block_allocator);
    if !memory  return null;

    block := cast(*Memblock) memory;
    block.total_size = total;
    pool_push(*pool.out_of_band_allocations, block);
    return cast(*u8) memory + header;
}

// The current block is retired and another taken — from the unused list when
// a reset left one there, otherwise from the allocator.
pool_new_block :: (pool: *Pool) -> bool {
    if pool.current_memblock {
        pool_push(*pool.used_memblocks, pool.current_memblock);
        pool.current_memblock = null;
    }

    header := pool_align_up(size_of(Memblock), pool.alignment);

    block: *Memblock;
    if pool.unused_memblocks {
        block = pool.unused_memblocks;
        pool.unused_memblocks = block.next;
        block.next = null;
    } else {
        memory := alloc(pool.memblock_size, pool.block_allocator);
        if !memory  return false;
        block = cast(*Memblock) memory;
        block.next = null;
        block.total_size = pool.memblock_size;
    }

    pool.current_memblock = block;
    pool.current_pos = cast(*u8) block + header;
    pool.bytes_left = block.total_size - header;
    return true;
}

pool_stamp :: (block: *Memblock) {
    header := size_of(Memblock);
    if block.total_size <= header  return;
    memset(cast(*u8) block + header, STAMP, block.total_size - header);
}

pool_free_list :: (pool: *Pool, list: **Memblock) {
    block := list.*;
    while block {
        next := block.next;
        free(block, pool.block_allocator);
        block = next;
    }
    list.* = null;
}

#scope_export

// The older spelling of the block size, and the overhead a block carries.
POOL_BUCKET_SIZE_DEFAULT :: POOL_BLOCK_SIZE_DEFAULT;
HEADER_SIZE :: size_of(Memblock);

// How much the pool is holding: the bytes inside its blocks, and how many
// oversized allocations sit outside them.
calculate_used_memory :: (pool: Pool) -> (inband_size: s64, outband_count: s64) {
    inband := 0;
    block := pool.used_memblocks;
    while block {
        inband += block.total_size;
        block = block.next;
    }
    if pool.current_memblock  inband += pool.current_memblock.total_size;

    outband := 0;
    extra := pool.out_of_band_allocations;
    while extra {
        outband += 1;
        extra = extra.next;
    }
    return inband, outband;
}
