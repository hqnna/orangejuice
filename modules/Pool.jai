//
// Pool: fixed-size items, reused rather than returned.
//
// Where `Flat_Pool` hands out bytes of any size and frees none of them, this
// hands out one size and takes them back — a freed item goes on a free list
// threaded through the item's own storage, so reuse costs a pointer read and
// the pool never gives memory back to the allocator until it is thrown away.
//
// That makes it right for things a program makes and unmakes constantly and
// which are all the same shape: nodes, particles, handles.
//

#import "Basic";

Pool :: struct (T: Type) {
    ITEMS_PER_BLOCK :: 256;

    Block :: struct {
        next: *Block;
        // The items follow the header.
    }

    // A free item's storage holds the next free one. An item must therefore be
    // at least pointer-sized, which `size_of` guarantees for anything worth
    // pooling.
    Free_Item :: struct {
        next: *Free_Item;
    }

    blocks:    *Block;
    free_list: *Free_Item;
    // Handed out and not yet given back.
    live:      s64;
    allocator: Allocator;
}

init :: (pool: *Pool($T), allocator := context.allocator) {
    pool.blocks = null;
    pool.free_list = null;
    pool.live = 0;
    pool.allocator = allocator;
}

// One item, zeroed. Null when the allocator could not give the pool a block.
get :: (pool: *Pool($T)) -> *T {
    if !pool.free_list {
        if !pool_add_block(pool)  return null;
    }
    item := pool.free_list;
    pool.free_list = item.next;
    pool.live += 1;

    memory := cast(*T) item;
    memset(memory, 0, size_of(T));
    return memory;
}

// Back on the free list. Reading through the pointer afterwards reads the free
// list, which is a use-after-free the same as any other.
release :: (pool: *Pool($T), item: *T) {
    if !item  return;
    node := cast(*Pool(T).Free_Item) item;
    node.next = pool.free_list;
    pool.free_list = node;
    pool.live -= 1;
}

// Every item back on the free list at once, without giving the blocks up.
reset :: (pool: *Pool($T)) {
    pool.free_list = null;
    pool.live = 0;
    block := pool.blocks;
    while block {
        pool_thread_block(pool, block);
        block = block.next;
    }
}

deinit :: (pool: *Pool($T)) {
    allocator := pool_allocator(pool);
    block := pool.blocks;
    while block {
        next := block.next;
        free(block, allocator);
        block = next;
    }
    pool.blocks = null;
    pool.free_list = null;
    pool.live = 0;
}

#scope_module

pool_allocator :: (pool: *Pool($T)) -> Allocator {
    if pool.allocator.proc  return pool.allocator;
    return context.allocator;
}

block_items :: (pool: *Pool($T), block: *Pool(T).Block) -> *T {
    return cast(*T) (cast(*u8) block + size_of(Pool(T).Block));
}

// Puts every item of a block on the free list, back to front so that the
// first item handed out afterwards is the lowest address — which keeps a
// freshly reset pool handing out items in order.
pool_thread_block :: (pool: *Pool($T), block: *Pool(T).Block) {
    items := block_items(pool, block);
    index := pool.ITEMS_PER_BLOCK - 1;
    while index >= 0 {
        node := cast(*Pool(T).Free_Item) (items + index);
        node.next = pool.free_list;
        pool.free_list = node;
        index -= 1;
    }
}

pool_add_block :: (pool: *Pool($T)) -> bool {
    #assert(size_of(T) >= size_of(*void), "A pooled item must be at least pointer-sized: a free one holds the next free one in its own storage.");

    bytes := size_of(Pool(T).Block) + size_of(T) * pool.ITEMS_PER_BLOCK;
    memory := alloc(bytes, pool_allocator(pool));
    if !memory  return false;

    block := cast(*Pool(T).Block) memory;
    block.next = pool.blocks;
    pool.blocks = block;
    pool.allocator = pool_allocator(pool);
    pool_thread_block(pool, block);
    return true;
}
