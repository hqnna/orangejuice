//
// CRC-32, the IEEE 802.3 polynomial — what zip, gzip and PNG use.
//
// Reflected form: the polynomial 0x04C11DB7 written backwards as 0xEDB88320,
// bits shifted right rather than left. The table of 256 entries is built once
// on first use rather than written out, which costs a few microseconds and
// saves a kilobyte of source.
//
// A checksum, not a hash: it detects accidental corruption and nothing else.
//

POLYNOMIAL : u32 : 0xEDB8_8320;

// The value a fresh CRC starts at and is finished with. Both are the same
// all-ones word; the inversion at each end is what makes leading zeroes in the
// input change the result.
INITIAL : u32 : 0xFFFF_FFFF;

crc32 :: (bytes: *u8, count: s64) -> u32 {
    return crc32_end(crc32_update(crc32_begin(), bytes, count));
}

crc32 :: (s: string) -> u32 {
    return crc32(s.data, s.count);
}

// Piecewise, for input that does not arrive all at once.
crc32_begin :: () -> u32 {
    return INITIAL;
}

crc32_update :: (running: u32, bytes: *u8, count: s64) -> u32 {
    table := crc_table();
    crc := running;
    index := 0;
    while index < count {
        slot := cast(s64) ((crc ^ cast(u32) bytes[index]) & 0xff);
        crc = table[slot] ^ (crc >> 8);
        index += 1;
    }
    return crc;
}

crc32_update :: (running: u32, s: string) -> u32 {
    return crc32_update(running, s.data, s.count);
}

crc32_end :: (running: u32) -> u32 {
    return running ^ INITIAL;
}

#scope_module

table_storage: [256] u32;
table_built := false;

crc_table :: () -> [] u32 {
    if !table_built {
        for index: 0..255 {
            crc := cast(u32) index;
            for 0..7 {
                if crc & 1  crc = POLYNOMIAL ^ (crc >> 1);
                else        crc = crc >> 1;
            }
            table_storage[index] = crc;
        }
        table_built = true;
    }
    result: [] u32;
    result.data = table_storage.data;
    result.count = table_storage.count;
    return result;
}

// The 64-bit variant, ECMA-182's polynomial. The same shift-and-xor as the
// 32-bit one, over a table built the same way.
crc64 :: (s: string) -> u64 {
    crc: u64 = 0xffff_ffff_ffff_ffff;
    for 0..s.count - 1 {
        crc = CRC64_TABLE[(crc ^ s[it]) & 0xff] ^ (crc >> 8);
    }
    return crc ^ 0xffff_ffff_ffff_ffff;
}

#scope_module

CRC64_POLYNOMIAL :: 0xC96C_5795_D787_0F42;

CRC64_TABLE :: #run -> [256] u64 {
    table: [256] u64;
    for byte: 0..255 {
        remainder := cast(u64) byte;
        for 1..8 {
            if remainder & 1  remainder = (remainder >> 1) ^ CRC64_POLYNOMIAL;
            else              remainder = remainder >> 1;
        }
        table[byte] = remainder;
    }
    return table;
};

#scope_export
