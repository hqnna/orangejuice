//
// An open-addressed hash table.
//
// Everything lives in one array of slots; a key that collides walks forward
// until it finds a free one. The step is the *triangular* numbers — 1, then 2
// more, then 3 more — which is guaranteed to reach every slot of a
// power-of-two table, and whose first few probes stay in the same cache line.
// That costs slightly more collisions per insertion than double hashing and
// makes each of them much cheaper, which is the trade that wins at size.
//
// A slot's hash doubles as its state: 0 means never used, 1 means the entry
// there was removed, and a real hash is at least 2. A lookup stops at a
// never-used slot and walks *past* a removed one, since whatever collided with
// it may lie beyond.
//
// The load factor is a parameter of the type, so a table that wants to trade
// memory for speed says so where it is declared.
//

#import "Basic";
#import "Hash";

Table :: struct (Key_Type: Type, Value_Type: Type,
                 given_hash_function: (Key_Type) -> u32 = null,
                 given_compare_function: (Key_Type, Key_Type) -> bool = null,
                 LOAD_FACTOR_PERCENT: u32 = 70,
                 REFILL_REMOVED := true
                ) {
    SIZE_MIN :: 32;

    // Valid items.
    count: int;

    // Slots we have memory for, and slots that cannot take a new item —
    // either holding one or marked removed.
    allocated:    int;
    slots_filled: int;

    allocator: Allocator;

    Entry :: struct {
        hash:  u32;
        key:   Key_Type;
        value: Value_Type;
    }

    entries: [] Entry;

    #if given_hash_function {
        hash_function :: given_hash_function;
    } else {
        hash_function :: x => get_hash(x);
    }

    #if given_compare_function {
        compare_function :: given_compare_function;
    } else {
        compare_function :: (a: Key_Type, b: Key_Type) -> bool { return a == b; };
    }
}

// The reserved hash values, exposed so that a caller writing its own iterator
// can tell a live slot from a dead one.
NEVER_OCCUPIED_HASH :: 0;
REMOVED_HASH        :: 1;
FIRST_VALID_HASH    :: 2;

// ------------------------------------------------------------- the walk ---

// Walks the probe sequence for `` `key ``, running `code` at each occupied
// slot with `` `index ``, `` `hash `` and `` `table `` in scope, and stopping
// at the first free one. Every lookup, insertion and removal here is this
// macro plus what to do at each slot.
Walk_Table :: (code: Code) #expand {
    mask := cast,trunc(u32) (`table.allocated - 1);

    `hash := `table.hash_function(`key);
    // The two reserved values are lifted out of the way, so a real hash never
    // looks like an empty or a removed slot.
    if hash < FIRST_VALID_HASH  hash += FIRST_VALID_HASH;

    `index := hash & mask;

    probe_increment: u32 = 1;
    while `table.entries[index].hash {
        #insert code;

        // The table is a power of two, so the mask is the wrap.
        index = (index + probe_increment) & mask;
        probe_increment += 1;
    }
}

// ------------------------------------------------------------- lifecycle ---

init :: (using table: *Table, slots_to_allocate: s64 = 0) {
    resize(table, ifx slots_to_allocate > 0 then next_power_of_two(slots_to_allocate) else SIZE_MIN);
}

deinit :: (table: *Table) {
    if table.entries.data  free(table.entries.data,, allocator = table_allocator(table));
    table.entries.data = null;
    table.entries.count = 0;
    table.count = 0;
    table.slots_filled = 0;
    table.allocated = 0;
}

// Empties the table without giving its memory back.
table_reset :: (using table: *Table) {
    for * entries  it.hash = NEVER_OCCUPIED_HASH;
    count = 0;
    slots_filled = 0;
}

// Fresh storage of exactly `slots_to_allocate` slots. The caller is
// responsible for moving anything that was in the old storage.
resize :: (using table: *Table, slots_to_allocate: s64 = 0) {
    wanted := ifx slots_to_allocate > 0 then slots_to_allocate else SIZE_MIN;
    if !allocator.proc  allocator = context.allocator;

    entries.data = cast(*Entry) alloc(wanted * size_of(Entry),, allocator = allocator);
    entries.count = wanted;
    allocated = wanted;
    for * entries  it.hash = NEVER_OCCUPIED_HASH;
}

// Rehashes into a bigger table — or into one the same size, when the old one
// was mostly removal markers and clearing those is enough.
expand :: (table: *Table) {
    old_entries := table.entries;

    new_allocated: s64;
    if (table.count * 2 + 1) * 100 < table.allocated * table.LOAD_FACTOR_PERCENT {
        // Doubling would still fit in what we have, so the space is going on
        // removal markers rather than on items: rehash in place.
        new_allocated = table.allocated;
    } else {
        new_allocated = table.allocated * 2;
    }
    if new_allocated < table.SIZE_MIN  new_allocated = table.SIZE_MIN;

    resize(table, new_allocated);

    // `table_add` counts them back up as it puts them in.
    table.count = 0;
    table.slots_filled = 0;
    for * entry: old_entries {
        if entry.hash >= FIRST_VALID_HASH  table_add(table, entry.key, entry.value);
    }

    if old_entries.data  free(old_entries.data,, allocator = table_allocator(table));
}

// Room for `items` more without a resize in between — which is what keeps
// pointers taken during a run of insertions valid.
table_ensure_space :: (table: *Table, items: s64) {
    if (table.slots_filled + items) * 100 >= table.allocated * table.LOAD_FACTOR_PERCENT  expand(table);
}

// ------------------------------------------------------------ the entries ---

// Adds the pair whether or not the key is already there, so a key added twice
// is present twice and the first one added is the one found. `table_set` is
// what replaces.
table_add :: (table: *Table, key: table.Key_Type, value: table.Value_Type) -> *table.Value_Type {
    #assert table.LOAD_FACTOR_PERCENT < 100;  // A table that can fill completely loops forever on a miss.

    if (table.slots_filled + 1) * 100 >= table.allocated * table.LOAD_FACTOR_PERCENT  expand(table);
    assert(table.slots_filled < table.allocated);

    Walk_Table(#code {
        #if table.REFILL_REMOVED {
            if table.entries[index].hash == REMOVED_HASH {
                // Reusing a removed slot: the increment below puts the count
                // back where it was.
                table.slots_filled -= 1;
                break;
            }
        }
    });

    table.count += 1;
    table.slots_filled += 1;

    entry := *table.entries[index];
    entry.hash = hash;
    entry.key = key;
    entry.value = value;
    return *entry.value;
}

// Replaces the value where the key is already present, and adds it otherwise.
table_set :: (table: *Table, key: table.Key_Type, value: table.Value_Type) -> *table.Value_Type {
    found := table_find_pointer(table, key);
    if found {
        found.* = value;
        return found;
    }
    return table_add(table, key, value);
}

// The value in the table, so that it can be written through. Only good until
// the next insertion: growing moves everything.
table_find_pointer :: (table: *Table, key: table.Key_Type) -> *table.Value_Type {
    if !table.allocated  return null;

    Walk_Table(#code {
        entry := *table.entries[index];
        if entry.hash == hash {
            if inline table.compare_function(entry.key, key)  return *entry.value;
        }
    });
    return null;
}

// `success` must be read: the value is uninitialised when it is false.
table_find :: (table: *Table, key: table.Key_Type) -> (table.Value_Type, success: bool #must) {
    pointer := inline table_find_pointer(table, key);
    if pointer  return pointer.*, true;

    dummy: table.Value_Type;
    return dummy, false;
}

table_contains :: (table: *Table, key: table.Key_Type) -> bool {
    return table_find_pointer(table, key) != null;
}

// Every value stored under `key`, in probe order — `table_add` puts a repeated
// key in another slot rather than replacing. The result is in temporary
// storage, so a caller iterating it has nothing to free.
table_find_multiple :: (table: *Table, key: table.Key_Type) -> [] table.Value_Type {
    results: [..] table.Value_Type;
    results.allocator = temp;
    if !table.allocated  return results;

    Walk_Table(#code {
        entry := *table.entries[index];
        if entry.hash == hash {
            if inline table.compare_function(entry.key, key)  array_add(*results, entry.value);
        }
    });
    return results;
}

// Leaves a marker rather than a hole, so probes that ran past this slot still
// reach what is beyond it.
table_remove :: (table: *Table, key: table.Key_Type) -> (success: bool, value: table.Value_Type) {
    if !table.allocated {
        dummy: table.Value_Type;
        return false, dummy;
    }

    Walk_Table(#code {
        entry := *table.entries[index];
        if entry.hash == hash && inline table.compare_function(entry.key, key) {
            entry.hash = REMOVED_HASH;
            table.count -= 1;
            return true, entry.value;
        }
    });

    dummy: table.Value_Type;
    return false, dummy;
}

// A pointer to the value under `key`, adding a zeroed one when there is none —
// which is what a caller that wants to fill the value in place wants.
find_or_add :: (table: *Table, key: table.Key_Type) -> (entry: *table.Value_Type, newly_added: bool) {
    value := table_find_pointer(table, key);
    if value  return value, false;

    fresh: table.Value_Type;
    return table_add(table, key, fresh), true;
}

// ------------------------------------------------------------- iterating ---

// `for table` walks the live entries, with `it` the value and `it_index` the
// key — which reads the way a map should (**L§7.14**).
for_expansion :: (table: *Table, body: Code, flags: For_Flags) #expand {
    for slot: 0..table.allocated - 1 {
        entry := *table.entries[slot];
        if entry.hash < FIRST_VALID_HASH  continue;

        `it_index := entry.key;
        #if flags & .POINTER {
            `it := *entry.value;
        } else {
            `it := entry.value;
        }
        #insert (break = break slot, continue = continue slot) body;
    }
}

next_power_of_two :: inline (x: int) -> int {
    assert(x != 0);
    p := 1;
    while x > p  p += p;
    return p;
}

#scope_module

table_allocator :: (table: *Table) -> Allocator {
    if table.allocator.proc  return table.allocator;
    return context.allocator;
}
