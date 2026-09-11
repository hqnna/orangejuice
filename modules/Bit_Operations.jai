//
// Counting and finding bits.
//
// Written as ordinary arithmetic rather than as the instructions the machine
// has for them: `popcount` is the SWAR trick, the scans are binary searches.
// A back end that recognises the shapes will emit `popcnt`, `tzcnt` and
// `lzcnt` anyway, and where it does not these are still a handful of cycles.
//

// How many bits are set.
popcount :: (value: u64) -> s64 {
    // Sum bits pairwise, then in nibbles, then in bytes, then multiply so the
    // top byte holds the total of all eight.
    x := value;
    x = x - ((x >> 1) & 0x5555_5555_5555_5555);
    x = (x & 0x3333_3333_3333_3333) + ((x >> 2) & 0x3333_3333_3333_3333);
    x = (x + (x >> 4)) & 0x0f0f_0f0f_0f0f_0f0f;
    return cast(s64) ((x * 0x0101_0101_0101_0101) >> 56);
}

popcount :: (value: u32) -> s64 { return popcount(cast(u64) value); }
popcount :: (value: u16) -> s64 { return popcount(cast(u64) value); }
popcount :: (value: u8)  -> s64 { return popcount(cast(u64) value); }

// How many zero bits above the highest set one; 64 for zero.
count_leading_zeros :: (value: u64) -> s64 {
    if value == 0  return 64;
    x := value;
    count := 0;
    if (x & 0xffff_ffff_0000_0000) == 0 { count += 32; x <<= 32; }
    if (x & 0xffff_0000_0000_0000) == 0 { count += 16; x <<= 16; }
    if (x & 0xff00_0000_0000_0000) == 0 { count +=  8; x <<=  8; }
    if (x & 0xf000_0000_0000_0000) == 0 { count +=  4; x <<=  4; }
    if (x & 0xc000_0000_0000_0000) == 0 { count +=  2; x <<=  2; }
    if (x & 0x8000_0000_0000_0000) == 0 { count +=  1; }
    return count;
}

// How many zero bits below the lowest set one; 64 for zero.
count_trailing_zeros :: (value: u64) -> s64 {
    if value == 0  return 64;
    x := value;
    count := 0;
    if (x & 0x0000_0000_ffff_ffff) == 0 { count += 32; x >>= 32; }
    if (x & 0x0000_0000_0000_ffff) == 0 { count += 16; x >>= 16; }
    if (x & 0x0000_0000_0000_00ff) == 0 { count +=  8; x >>=  8; }
    if (x & 0x0000_0000_0000_000f) == 0 { count +=  4; x >>=  4; }
    if (x & 0x0000_0000_0000_0003) == 0 { count +=  2; x >>=  2; }
    if (x & 0x0000_0000_0000_0001) == 0 { count +=  1; }
    return count;
}

// One *plus* the index of the highest set bit, searching from the top down.
// Zero means the input had no bits set, which is what leaves every real answer
// positive and needs no sentinel.
bit_scan_reverse :: (value: u64) -> s32 {
    if value == 0  return 0;
    return cast(s32) (64 - count_leading_zeros(value));
}

// One plus the index of the lowest set bit, searching from the bottom up.
bit_scan_forward :: (value: u64) -> s32 {
    if value == 0  return 0;
    return cast(s32) (count_trailing_zeros(value) + 1);
}

rotate_left :: (value: u64, amount: u64) -> u64 {
    shift := amount & 63;
    if shift == 0  return value;
    return (value << shift) | (value >> (64 - shift));
}

rotate_right :: (value: u64, amount: u64) -> u64 {
    shift := amount & 63;
    if shift == 0  return value;
    return (value >> shift) | (value << (64 - shift));
}

byte_swap :: (value: u64) -> u64 {
    x := value;
    x = ((x & 0x00ff_00ff_00ff_00ff) <<  8) | ((x & 0xff00_ff00_ff00_ff00) >>  8);
    x = ((x & 0x0000_ffff_0000_ffff) << 16) | ((x & 0xffff_0000_ffff_0000) >> 16);
    return (x << 32) | (x >> 32);
}

byte_swap :: (value: u32) -> u32 {
    x := value;
    x = ((x & 0x00ff_00ff) << 8) | ((x & 0xff00_ff00) >> 8);
    return (x << 16) | (x >> 16);
}

byte_swap :: (value: u16) -> u16 {
    return cast(u16) ((cast(u32) value << 8) | (cast(u32) value >> 8));
}

is_power_of_two :: (value: u64) -> bool {
    return value != 0 && (value & (value - 1)) == 0;
}

// The smallest power of two not below `value`; 1 for zero.
next_power_of_two :: (value: u64) -> u64 {
    if value <= 1  return 1;
    return cast(u64) 1 << cast(u64) (64 - count_leading_zeros(value - 1));
}

// Whether two runs of bytes are the same.
bits_equal :: inline (a: *void, b: *void, length_in_bytes: s64) -> bool #deprecated "Do this directly; this is too trivial to burden the Bit_Operations module with." {
    return memcmp(a, b, length_in_bytes) == 0;
}

bits_equal :: inline (a: *$T, b: *T) -> bool #deprecated "Do this directly; this is too trivial to burden the Bit_Operations module with." {
    return memcmp(a, b, size_of(T)) == 0;
}

// Reverses the bytes of whatever it is given, where it stands.
byte_swap_in_place :: (input: *$T) #expand {
    input.* = byte_swap(input.*);
}

// The offset of the first byte that differs, or `n` when they all match.
first_different_byte :: (a: *void, b: *void, n: u64) -> u64 #deprecated "Copy this into your own project if you want it; it's going away." {
    left := cast(*u8) a;
    right := cast(*u8) b;
    for 0..cast(s64) n - 1 {
        if left[it] != right[it]  return cast(u64) it;
    }
    return n;
}
