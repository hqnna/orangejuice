//
// Bucket_Array: many items whose addresses never move.
//
// A `[..] T` reallocates when it grows, so a pointer into one is only good
// until the next add. This keeps a list of fixed buckets instead: adding never
// moves what is already there, so a pointer handed out stays valid for the
// life of the array. That is what makes it right for things other structures
// point at.
//
// An item removed leaves its slot occupied-no-longer rather than shuffling
// what follows, so indices stay stable too.
//

#import "Basic";

Bucket_Array :: struct (T: Type, ITEMS_PER_BUCKET := 128) {
    Bucket :: struct {
        count:    s64;          // how many slots have ever been used
        occupied: [ITEMS_PER_BUCKET] bool;
        items:    [ITEMS_PER_BUCKET] T;
    }

    buckets:   [..] *Bucket;
    count:     s64;             // live items
    allocator: Allocator;
}

// Where an item sits. Stays correct as the array grows.
Bucket_Locator :: struct {
    bucket: s64;
    slot:   s64;
}

INVALID_LOCATOR :: Bucket_Locator.{-1, -1};

init :: (array: *Bucket_Array($T, $N), allocator := context.allocator) {
    array.allocator = allocator;
    array.buckets.allocator = allocator;
    array.count = 0;
}

deinit :: (array: *Bucket_Array($T, $N)) {
    for array.buckets  free(it, bucket_allocator(array));
    array_free(*array.buckets);
    array.count = 0;
}

// The item's address and where it went. The address stays good until the item
// is removed or the array is thrown away.
bucket_array_add :: (array: *Bucket_Array($T, $N), value: T) -> (pointer: *T, locator: Bucket_Locator) {
    locator := first_free_slot(array);
    if locator.bucket < 0 {
        if !add_bucket(array)  return null, INVALID_LOCATOR;
        locator = .{array.buckets.count - 1, 0};
    }

    bucket := array.buckets[locator.bucket];
    bucket.items[locator.slot] = value;
    bucket.occupied[locator.slot] = true;
    if locator.slot >= bucket.count  bucket.count = locator.slot + 1;
    array.count += 1;
    return *bucket.items[locator.slot], locator;
}

// Null when the locator names a slot nothing lives in.
bucket_array_find :: (array: *Bucket_Array($T, $N), locator: Bucket_Locator) -> *T {
    if locator.bucket < 0 || locator.bucket >= array.buckets.count  return null;
    bucket := array.buckets[locator.bucket];
    if locator.slot < 0 || locator.slot >= N       return null;
    if !bucket.occupied[locator.slot]              return null;
    return *bucket.items[locator.slot];
}

// Leaves the slot free for the next add. Nothing else moves, so every other
// pointer and locator stays valid.
bucket_array_remove :: (array: *Bucket_Array($T, $N), locator: Bucket_Locator) -> bool {
    if locator.bucket < 0 || locator.bucket >= array.buckets.count  return false;
    bucket := array.buckets[locator.bucket];
    if locator.slot < 0 || locator.slot >= N       return false;
    if !bucket.occupied[locator.slot]              return false;

    bucket.occupied[locator.slot] = false;
    array.count -= 1;
    return true;
}

// Skips the empty slots, so a loop sees only what is live. `it_index` counts
// the items seen rather than naming a slot.
for_expansion :: (array: *Bucket_Array($T, $N), body: Code, flags: For_Flags) #expand {
    `it_index := 0;
    for bucket_index: 0..array.buckets.count - 1 {
        bucket := array.buckets[bucket_index];
        for slot: 0..bucket.count - 1 {
            if !bucket.occupied[slot]  continue;

            #if flags & .POINTER {
                `it := *bucket.items[slot];
            } else {
                `it := bucket.items[slot];
            }
            #insert body;
            `it_index += 1;
        }
    }
}

#scope_module

bucket_allocator :: (array: *Bucket_Array($T, $N)) -> Allocator {
    if array.allocator.proc  return array.allocator;
    return context.allocator;
}

first_free_slot :: (array: *Bucket_Array($T, $N)) -> Bucket_Locator {
    for bucket_index: 0..array.buckets.count - 1 {
        bucket := array.buckets[bucket_index];
        // Past the high-water mark is free by definition; below it, only what
        // was given back.
        if bucket.count < N  return .{bucket_index, bucket.count};
        for slot: 0..bucket.count - 1 {
            if !bucket.occupied[slot]  return .{bucket_index, slot};
        }
    }
    return INVALID_LOCATOR;
}

add_bucket :: (array: *Bucket_Array($T, $N)) -> bool {
    allocator := bucket_allocator(array);
    memory := alloc_zeroed(size_of(Bucket_Array(T, N).Bucket), allocator);
    if !memory  return false;
    array.buckets.allocator = allocator;
    array_add(*array.buckets, cast(*Bucket_Array(T, N).Bucket) memory);
    return true;
}
