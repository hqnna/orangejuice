//
// Hashing bytes and the things made of them.
//
// FNV-1a, 64-bit: one multiply and one xor per byte, no tables, and good
// enough spread for a hash table's keys. It is not a cryptographic hash and
// nothing here should be used as one.
//

FNV_64_OFFSET :: 0xcbf2_9ce4_8422_2325;
FNV_64_PRIME  :: 0x0000_0100_0000_01b3;

// The seed to start a rolling hash from, so that several values can be hashed
// into one result: `h := fnv1a(a); h = fnv1a(b, h);`
fnv1a :: (bytes: *u8, count: s64, seed: u64 = FNV_64_OFFSET) -> u64 {
    hash := seed;
    index := 0;
    while index < count {
        hash ^= bytes[index];
        hash *= FNV_64_PRIME;
        index += 1;
    }
    return hash;
}

fnv1a :: (s: string, seed: u64 = FNV_64_OFFSET) -> u64 {
    return fnv1a(s.data, s.count, seed);
}

// The default for a hash table's key: a string hashes by its bytes, anything
// else by the bytes it occupies. A type holding a pointer therefore hashes by
// the pointer rather than by what it points at, which is why a key with
// indirection in it wants a hash procedure of its own.
get_hash :: (value: $T) -> u64 {
    #if T == string {
        return fnv1a(value);
    } else {
        local := value;
        return fnv1a(cast(*u8) *local, size_of(T));
    }
}

// Mixes a 64-bit value so that keys differing in the low bits land in
// different buckets. Thomas Wang's finalizer.
hash_u64 :: (value: u64) -> u64 {
    hash := value;
    hash = (~hash) + (hash << 21);
    hash = hash ^ (hash >> 24);
    hash = (hash + (hash << 3)) + (hash << 8);
    hash = hash ^ (hash >> 14);
    hash = (hash + (hash << 2)) + (hash << 4);
    hash = hash ^ (hash >> 28);
    hash = hash + (hash << 31);
    return hash;
}

// Folds another value into a hash already computed.
combine_hash :: (hash: u64, other: u64) -> u64 {
    return hash_u64(hash ^ (other + 0x9e37_79b9_7f4a_7c15 + (hash << 6) + (hash >> 2)));
}
