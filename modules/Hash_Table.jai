//
// Hash_Table: keys to values, open addressing.
//
// Buckets hold their entry inline rather than pointing at one, and a
// collision walks forward to the next free slot (linear probing), which keeps
// a lookup to one cache line most of the time. A removed slot is marked rather
// than emptied, so a probe that walked past it still finds what is beyond.
//
// The table grows when it is 70% full. Growing rehashes everything, so a
// pointer into the table is only good until the next `table_add`.
//

#import "Basic";
#import "Hash";

Table :: struct (Key_Type: Type, Value_Type: Type, given_hash: (Key_Type) -> u64 = null, given_compare: (Key_Type, Key_Type) -> bool = null) {
    SIZE_MIN  :: 32;
    // Numerator and denominator rather than a float, so that growing is exact
    // whatever the size.
    LOAD_NUM  :: 7;
    LOAD_DEN  :: 10;

    Entry :: struct {
        hash:  u64;   // 0 = never used, 1 = removed; a real hash is >= 2.
        key:   Key_Type;
        value: Value_Type;
    }

    count:     s64;   // live entries
    occupied:  s64;   // live entries plus removed ones
    allocated: s64;
    entries:   [] Entry;
    allocator: Allocator;
}

NEVER_OCCUPIED :: 0;
REMOVED        :: 1;
FIRST_VALID    :: 2;

init :: (table: *Table($K, $V), size := 0, allocator := context.allocator) {
    table.allocator = allocator;
    wanted := ifx size > 0 then size else table.SIZE_MIN;
    table_allocate(table, round_up_to_power_of_two(wanted));
}

deinit :: (table: *Table($K, $V)) {
    if table.entries.data  free(table.entries.data, table_allocator(table));
    table.entries.data = null;
    table.entries.count = 0;
    table.count = 0;
    table.occupied = 0;
    table.allocated = 0;
}

table_reset :: (table: *Table($K, $V)) {
    for * table.entries  it.hash = NEVER_OCCUPIED;
    table.count = 0;
    table.occupied = 0;
}

// Adds an entry whether or not the key is already there, so a key added twice
// is present twice. `table_set` is what replaces.
table_add :: (table: *Table($K, $V), key: K, value: V) -> *V {
    if table.allocated == 0  init(table);
    if (table.occupied + 1) * table.LOAD_DEN >= table.allocated * table.LOAD_NUM {
        table_grow(table);
    }

    hash := table_hash(table, key);
    index := cast(s64) (hash & cast(u64) (table.allocated - 1));
    while table.entries[index].hash >= FIRST_VALID {
        index += 1;
        if index >= table.allocated  index = 0;
    }

    entry := *table.entries[index];
    if entry.hash == NEVER_OCCUPIED  table.occupied += 1;
    entry.hash = hash;
    entry.key = key;
    entry.value = value;
    table.count += 1;
    return *entry.value;
}

// Replaces the value where the key is already present, and adds it otherwise.
table_set :: (table: *Table($K, $V), key: K, value: V) -> *V {
    found := table_find_pointer(table, key);
    if found {
        found.* = value;
        return found;
    }
    return table_add(table, key, value);
}

table_find :: (table: *Table($K, $V), key: K) -> (value: V, found: bool) {
    pointer := table_find_pointer(table, key);
    if pointer  return pointer.*, true;
    empty: V;
    return empty, false;
}

// The value in the table, so that it can be written through. Only good until
// the next `table_add`: growing moves everything.
table_find_pointer :: (table: *Table($K, $V), key: K) -> *V {
    if !table.allocated  return null;

    hash := table_hash(table, key);
    index := cast(s64) (hash & cast(u64) (table.allocated - 1));
    // A probe stops at a slot that was never used; it walks past removed ones,
    // because whatever collided with them may lie beyond.
    while table.entries[index].hash != NEVER_OCCUPIED {
        entry := *table.entries[index];
        if entry.hash == hash && table_keys_match(table, entry.key, key) {
            return *entry.value;
        }
        index += 1;
        if index >= table.allocated  index = 0;
    }
    return null;
}

table_contains :: (table: *Table($K, $V), key: K) -> bool {
    return table_find_pointer(table, key) != null;
}

// Leaves a marker rather than a hole, so probes that ran past this slot still
// reach what is beyond it.
table_remove :: (table: *Table($K, $V), key: K) -> bool {
    if !table.allocated  return false;

    hash := table_hash(table, key);
    index := cast(s64) (hash & cast(u64) (table.allocated - 1));
    while table.entries[index].hash != NEVER_OCCUPIED {
        entry := *table.entries[index];
        if entry.hash == hash && table_keys_match(table, entry.key, key) {
            entry.hash = REMOVED;
            table.count -= 1;
            return true;
        }
        index += 1;
        if index >= table.allocated  index = 0;
    }
    return false;
}

// `for table` gives the values, with `it_index` the key — which is what the
// reference's tables do, so a loop written for one reads the same here
// (**L§6.6**).
for_expansion :: (table: *Table($K, $V), body: Code, flags: For_Flags) #expand {
    for index: 0..table.allocated - 1 {
        entry := *table.entries[index];
        if entry.hash < FIRST_VALID  continue;

        `it_index := entry.key;
        #if flags & .POINTER {
            `it := *entry.value;
        } else {
            `it := entry.value;
        }
        #insert body;
    }
}

// Every value stored under `key`, in the order the probe reaches them —
// `table_add` puts a repeated key in a second slot rather than replacing, and
// this is what reads them all back.
//
// The result is in temporary storage by default, so a caller iterating it does
// not have to free anything; pass an allocator to keep it.
table_find_multiple :: (table: *Table($K, $V), key: K, allocator := temp) -> [] V {
    found: [..] V;
    found.allocator = allocator;
    if !table.allocated  return found;

    hash := table_hash(table, key);
    index := cast(s64) (hash & cast(u64) (table.allocated - 1));
    while table.entries[index].hash != NEVER_OCCUPIED {
        entry := *table.entries[index];
        if entry.hash == hash && table_keys_match(table, entry.key, key) {
            array_add(*found, entry.value);
        }
        index += 1;
        if index >= table.allocated  index = 0;
    }
    return found;
}

// The same, as pointers into the table, so a caller can write through them.
table_find_multiple_pointers :: (table: *Table($K, $V), key: K, allocator := temp) -> [] *V {
    found: [..] *V;
    found.allocator = allocator;
    if !table.allocated  return found;

    hash := table_hash(table, key);
    index := cast(s64) (hash & cast(u64) (table.allocated - 1));
    while table.entries[index].hash != NEVER_OCCUPIED {
        entry := *table.entries[index];
        if entry.hash == hash && table_keys_match(table, entry.key, key) {
            array_add(*found, *entry.value);
        }
        index += 1;
        if index >= table.allocated  index = 0;
    }
    return found;
}

#scope_module

table_allocator :: (table: *Table($K, $V)) -> Allocator {
    if table.allocator.proc  return table.allocator;
    return context.allocator;
}

table_hash :: (table: *Table($K, $V), key: K) -> u64 {
    hash: u64;
    #if table.given_hash {
        hash = table.given_hash(key);
    } else {
        hash = get_hash(key);
    }
    // 0 and 1 mean "never used" and "removed", so a real hash never takes
    // them.
    if hash < FIRST_VALID  hash += FIRST_VALID;
    return hash;
}

table_keys_match :: (table: *Table($K, $V), a: K, b: K) -> bool {
    #if table.given_compare {
        return table.given_compare(a, b);
    } else #if K == string {
        return equal(a, b);
    } else {
        return a == b;
    }
}

table_allocate :: (table: *Table($K, $V), size: s64) {
    allocator := table_allocator(table);
    bytes := size * size_of(type_of(table.entries[0]));
    memory := alloc_zeroed(bytes, allocator);
    if !memory  return;

    table.entries.data = cast(*type_of(table.entries[0])) memory;
    table.entries.count = size;
    table.allocated = size;
    table.count = 0;
    table.occupied = 0;
    table.allocator = allocator;
}

// Rehashes into a table twice the size, dropping the removed markers on the
// way — which is what keeps a table that is added to and removed from in equal
// measure from filling up with them.
table_grow :: (table: *Table($K, $V)) {
    old := table.entries;
    old_allocated := table.allocated;
    allocator := table_allocator(table);

    table_allocate(table, max(old_allocated * 2, table.SIZE_MIN));
    if !table.entries.data {
        table.entries = old;
        table.allocated = old_allocated;
        return;
    }

    for index: 0..old_allocated - 1 {
        entry := *old[index];
        if entry.hash < FIRST_VALID  continue;
        table_add(table, entry.key, entry.value);
    }
    if old.data  free(old.data, allocator);
}

round_up_to_power_of_two :: (value: s64) -> s64 {
    result := 1;
    while result < value  result *= 2;
    return result;
}
