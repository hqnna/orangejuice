//
// A growing array whose items never move.
//
// Storage comes in buckets of a fixed size; adding an item fills a slot in one
// of them rather than reallocating anything, so a pointer taken to an item
// stays good for as long as the item does. That is the whole point of it — a
// `[..]` array is faster to walk and cheaper per item, but every pointer into
// one dies the next time it grows.
//
// A removed slot is marked free rather than filled in from the end, so the
// items keep the positions they were given and a `Bucket_Locator` stays
// meaningful. `for` skips the free ones.
//

Bucket_Array :: struct (type: Type, items_per_bucket: int, always_iterate_by_pointer := false) {
    count: s64;

    allocator: Allocator;

    My_Bucket :: Bucket(type, items_per_bucket);

    all_buckets:    [..] *My_Bucket;
    // The ones with a free slot, so adding never has to look through the full
    // buckets at all.
    unfull_buckets: [..] *My_Bucket;
}

Bucket :: struct (type: Type, items_per_bucket: int) {
    occupied: [items_per_bucket] bool;
    data:     [items_per_bucket] type = ---;

    // Where the search for a free slot starts. Everything below it is taken.
    lowest_maybe_not_occupied: s32;
    bucket_index: u32;

    count := 0;
}

// Where one item lives. A locator survives anything but the removal of the
// item it names.
Bucket_Locator :: struct {
    bucket_index: u32;
    // Signed so that -1 can mean "nowhere".
    slot_index:   s32;
}

// Gives every bucket back and leaves the array as though it were fresh.
bucket_array_reset :: (using array: *Bucket_Array) {
    for all_buckets  free(it,, allocator);
    array_reset(*all_buckets);
    array_reset(*unfull_buckets);

    count = 0;
}

bucket_array_add :: (using array: *Bucket_Array, item: array.type) -> (locator: Bucket_Locator, pointer: *array.type) {
    pointer, locator := find_and_occupy_empty_slot(array, initialize = false);
    pointer.* = item;

    return locator, pointer;
}

bucket_array_find :: (using array: *Bucket_Array, locator: Bucket_Locator) -> array.type {
    bucket := all_buckets[locator.bucket_index];
    assert(bucket.occupied[locator.slot_index] == true);
    return bucket.data[locator.slot_index];
}

bucket_array_remove :: (using array: *Bucket_Array, locator: Bucket_Locator) {
    bucket := all_buckets[locator.bucket_index];
    assert(bucket.occupied[locator.slot_index] == true);

    was_full := bucket.count == bucket.items_per_bucket;

    bucket.occupied[locator.slot_index] = false;

    if locator.slot_index < bucket.lowest_maybe_not_occupied {
        bucket.lowest_maybe_not_occupied = locator.slot_index;
    }

    bucket.count -= 1;
    array.count  -= 1;

    // A bucket that was full is not on the unfull list, and now belongs there.
    if was_full {
        assert(array_find(array.unfull_buckets, bucket) == false);
        array_add(*array.unfull_buckets, bucket);
    }
}

// Takes a slot without putting anything in it, and hands back where it is.
// `initialize` false leaves the bytes alone, which is what `bucket_array_add`
// wants since it is about to overwrite them.
find_and_occupy_empty_slot :: (using array: *Bucket_Array, $initialize := true) -> (pointer: *array.type, locator: Bucket_Locator) {
    if !unfull_buckets.count  add_bucket(array);
    assert(unfull_buckets.count > 0);

    bucket := unfull_buckets[0];

    index := -1;
    for bucket.lowest_maybe_not_occupied..bucket.items_per_bucket - 1 {
        if !bucket.occupied[it] {
            index = it;
            break;
        }
    }

    assert(index != -1);

    bucket.occupied[index] = true;
    bucket.count += 1;
    bucket.lowest_maybe_not_occupied = cast(s32) (index + 1);
    assert(bucket.count <= bucket.items_per_bucket);

    memory := *bucket.data[index];
    #if initialize {
        ini :: initializer_of(bucket.type);
        #if ini  ini(memory);
        else     memset(memory, 0, size_of(bucket.type));
    }

    array.count += 1;

    if bucket.count == bucket.items_per_bucket {
        removed := array_unordered_remove_by_value(*array.unfull_buckets, bucket);
        assert(removed == 1);
    }

    locator: Bucket_Locator;
    locator.bucket_index = bucket.bucket_index;
    locator.slot_index   = cast(s32) index;

    return memory, locator;
}

// `it_index` counts the *items*, not the slots, so a removed slot does not
// leave a gap in it. `remove` inside the loop body removes the item being
// visited.
for_expansion :: (array: *$T/Bucket_Array, body: Code, flags: For_Flags) #expand {
    `it_index: int;

    REVERSE :: cast(bool) (flags & .REVERSE);

    #if REVERSE  it_index = array.count;
    else         it_index = -1;

    // A struct may ask to always be visited by pointer, which is right when
    // its items are big enough that copying one is the expensive part.
    DO_POINTER :: (flags & .POINTER) || array.always_iterate_by_pointer;

    for <=REVERSE bucket, bi: array.all_buckets {
        for <=REVERSE *=DO_POINTER  `it, i: bucket.data {
            if !bucket.occupied[i]  continue;

            #if REVERSE  it_index -= 1;
            else         it_index += 1;

            // `break` leaves the *outer* loop, since the inner one is only a
            // bucket's worth of the array.
            #insert (break = break bucket, remove = {
                locator: Bucket_Locator;
                locator.bucket_index = cast(u32) bi;
                locator.slot_index   = cast(s32) i;
                bucket_array_remove(array, locator);
            }) body;
        }
    }
}

// By position rather than by locator, which only means anything while no slot
// below `index` has been freed.
operator *[] :: (using array: Bucket_Array, index: int) -> *array.type {
    bucket_index := index / items_per_bucket;
    slot_index   := index % items_per_bucket;
    bucket := all_buckets[bucket_index];
    assert(bucket.occupied[slot_index] == true);
    return *bucket.data[slot_index];
}

#scope_file

#import "Basic";

add_bucket :: (using array: *Bucket_Array) -> *array.My_Bucket {
    assert(unfull_buckets.count == 0);

    // The array's own allocator reaches the two bucket lists on the first add,
    // which is the earliest moment it is known.
    if !all_buckets.count && allocator.proc {
        all_buckets.allocator    = allocator;
        unfull_buckets.allocator = allocator;
    }

    new_bucket := New(My_Bucket,, allocator);
    new_bucket.bucket_index = cast(u32) all_buckets.count;
    // Catches the overflow of the 32-bit index rather than letting two buckets
    // answer to one number.
    assert(new_bucket.bucket_index == all_buckets.count);

    array_add(*all_buckets,    new_bucket);
    array_add(*unfull_buckets, new_bucket);

    return new_bucket;
}
