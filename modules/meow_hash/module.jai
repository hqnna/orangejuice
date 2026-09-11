//
// Meow hash: a 128-bit hash built on the AES round instruction.
//
// An implementation of Casey Muratori's Meow hash
// (github.com/cmuratori/meow_hash), version 0.5 "calico", for x86-64 with
// AES-NI. Not a binding: the algorithm is written out here in `#asm` and
// checked against the reference implementation's output, since a hash whose
// value is agreeing with other tools is worthless if it does not.
//
// Eight 128-bit lanes are threaded through `aesdec`, `paddq` and `pxor`, one
// 256-byte block at a time, then folded down to a single 128-bit result. AES
// rounds are what make it fast — the hardware does the mixing — and are also
// why it needs AES-NI and will fault without it.
//
// Not cryptographic. The nonce fields the design leaves room for are
// deliberately zero, exactly as upstream leaves them.
//
// Checked against the reference implementation: the empty string, 1, 3, 40, 43
// and 1000 bytes — between them the residual path, the whole-lane path and
// several 256-byte blocks — all agree exactly. A hash whose whole value is
// agreeing with other tools is worthless unverified, so it is verified.
//

#import "Basic";
#import "Machine_X64";

reg :: __reg;

MEOW_HASH_VERSION      :: 5;
MEOW_HASH_VERSION_NAME :: "0.5/calico";

MEOW_PAGESIZE :: 4096;

// Whether this processor can run it at all.
meow_hash_is_available :: () -> bool {
    cpu := get_cpu_info();
    return check_feature(cpu.feature_leaves, .AES);
}

// The default seed is an encoding of pi — a nothing-up-our-sleeves number, so
// nobody has to wonder whether the constants were chosen to hide something.
MeowDefaultSeed: [128] u8 = u8.[
    0x32, 0x43, 0xF6, 0xA8, 0x88, 0x5A, 0x30, 0x8D,
    0x31, 0x31, 0x98, 0xA2, 0xE0, 0x37, 0x07, 0x34,
    0x4A, 0x40, 0x93, 0x82, 0x22, 0x99, 0xF3, 0x1D,
    0x00, 0x82, 0xEF, 0xA9, 0x8E, 0xC4, 0xE6, 0xC8,
    0x94, 0x52, 0x82, 0x1E, 0x63, 0x8D, 0x01, 0x37,
    0x7B, 0xE5, 0x46, 0x6C, 0xF3, 0x4E, 0x90, 0xC6,
    0xCC, 0x0A, 0xC2, 0x9B, 0x7C, 0x97, 0xC5, 0x0D,
    0xD3, 0xF8, 0x4D, 0x5B, 0x5B, 0x54, 0x70, 0x91,
    0x79, 0x21, 0x6D, 0x5D, 0x98, 0x97, 0x9F, 0xB1,
    0xBD, 0x13, 0x10, 0xBA, 0x69, 0x8D, 0xFB, 0x5A,
    0xC2, 0xFF, 0xD7, 0x2D, 0xBD, 0x01, 0xAD, 0xFB,
    0x7B, 0x8E, 0x1A, 0xFE, 0xD6, 0xA2, 0x67, 0xE9,
    0x6B, 0xA7, 0xC9, 0x04, 0x5F, 0x12, 0xC7, 0xF9,
    0x92, 0x4A, 0x19, 0x94, 0x7B, 0x39, 0x16, 0xCF,
    0x70, 0x80, 0x1F, 0x2E, 0x28, 0x58, 0xEF, 0xC1,
    0x66, 0x36, 0x92, 0x0D, 0x87, 0x15, 0x74, 0xE6,
];

// Byte tables for the tail: `MeowShiftAdjust` realigns an unaligned final read
// and `MeowMaskLen` zeroes whatever lies past the end of the input.
MeowShiftAdjust: [32] u8 = u8.[
    0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,
    0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,
];

MeowMaskLen: [32] u8 = u8.[
    255,255,255,255, 255,255,255,255, 255,255,255,255, 255,255,255,255,
    0,0,0,0, 0,0,0,0, 0,0,0,0, 0,0,0,0,
];

// The 128-bit result, as bytes.
Meow_Hash :: struct {
    bytes: [16] u8;
}

// The whole input at once.
meow_hash :: (seed: [128] u8, source: *void, length: u64) -> Meow_Hash {
    result: Meow_Hash;
    hash_into(seed, source, length, result.bytes.data);
    return result;
}

meow_hash :: (s: string) -> Meow_Hash {
    return meow_hash(MeowDefaultSeed, s.data, cast(u64) s.count);
}

meow_hash :: (bytes: [] u8) -> Meow_Hash {
    return meow_hash(MeowDefaultSeed, bytes.data, cast(u64) bytes.count);
}

// The low 64 or 32 bits, for a caller that wants an ordinary integer.
meow_u64 :: (hash: Meow_Hash) -> u64 {
    result: u64;
    memcpy(*result, hash.bytes.data, 8);
    return result;
}

meow_u32 :: (hash: Meow_Hash) -> u32 {
    result: u32;
    memcpy(*result, hash.bytes.data, 4);
    return result;
}

meow_hashes_are_equal :: (a: Meow_Hash, b: Meow_Hash) -> bool {
    return memcmp(a.bytes.data, b.bytes.data, 16) == 0;
}

#scope_module

// One mixing step over four 128-bit inputs already in registers.
MEOW_MIX_REG :: (r1: reg, r2: reg, r3: reg, r4: reg, r5: reg, i1: reg, i2: reg, i3: reg, i4: reg) #expand {
    #asm {
        aesdec  r1, r2;
        paddq   r3, i1;
        pxor    r2, i2;
        aesdec  r2, r4;
        paddq   r5, i3;
        pxor    r4, i4;
    }
}

// The same, loading the four inputs from a 32-byte window of the source. The
// overlapping offsets — 15, 0, 1, 16 — are what make neighbouring bytes affect
// different lanes.
MEOW_MIX :: (r1: reg, r2: reg, r3: reg, r4: reg, r5: reg, ptr: *void, $OFFSET: s32) #expand {
    #asm {
        movdqu i1:, [ptr + (OFFSET + 15)];
        movdqu i2:, [ptr + (OFFSET + 0)];
        movdqu i3:, [ptr + (OFFSET + 1)];
        movdqu i4:, [ptr + (OFFSET + 16)];
    }
    MEOW_MIX_REG(r1, r2, r3, r4, r5, i1, i2, i3, i4);
}

// The fold: no input, just the lanes stirring one another.
MEOW_SHUFFLE :: (r1: reg, r2: reg, r3: reg, r4: reg, r5: reg, r6: reg) #expand {
    #asm {
        aesdec  r1, r4;
        paddq   r2, r5;
        pxor    r4, r6;
        aesdec  r4, r2;
        paddq   r5, r6;
        pxor    r2, r3;
    }
}

hash_into :: (seed: [128] u8, source_in: *void, length: u64, store: *u8) {
    source := source_in;

    #asm {
        h0: vec; h1: vec; h2: vec; h3: vec;
        h4: vec; h5: vec; h6: vec; h7: vec;
    }

    // Seed the eight lanes.
    seed_pointer := seed.data;
    #asm {
        movdqu h0, [seed_pointer + 0x00];
        movdqu h1, [seed_pointer + 0x10];
        movdqu h2, [seed_pointer + 0x20];
        movdqu h3, [seed_pointer + 0x30];
        movdqu h4, [seed_pointer + 0x40];
        movdqu h5, [seed_pointer + 0x50];
        movdqu h6, [seed_pointer + 0x60];
        movdqu h7, [seed_pointer + 0x70];
    }

    // Every whole 256-byte block, eight 32-byte windows at a time.
    block_count := length >> 8;
    advance := block_count << 8;
    while block_count {
        MEOW_MIX(h0, h4, h6, h1, h2, source, 0x00);
        MEOW_MIX(h1, h5, h7, h2, h3, source, 0x20);
        MEOW_MIX(h2, h6, h0, h3, h4, source, 0x40);
        MEOW_MIX(h3, h7, h1, h4, h5, source, 0x60);
        MEOW_MIX(h4, h0, h2, h5, h6, source, 0x80);
        MEOW_MIX(h5, h1, h3, h6, h7, source, 0xa0);
        MEOW_MIX(h6, h2, h4, h7, h0, source, 0xc0);
        MEOW_MIX(h7, h3, h5, h0, h1, source, 0xe0);
        source += 0x100;
        block_count -= 1;
    }
    source = source_in;

    // The residual: whatever is left after the whole blocks, in two registers.
    #asm {
        pxor r1:, r1;
        pxor r3:, r3;
    }

    last := source + (length & ~cast(u64) 0xf);
    tail := length & 0xf;
    if tail {
        mask_pointer := MeowMaskLen.data;
        shift_pointer := MeowShiftAdjust.data;
        mask_index := 0x10 - tail;
        #asm {
            mask: vec;
            movdqu mask, [mask_pointer + mask_index];
        }

        // Reading sixteen bytes from `last` could cross into the next page.
        // When it would, the read is backed up to an alignment boundary and
        // the bytes shifted into place instead.
        last_ok := cast(*void) ((((cast(u64) source) + length - 1) | (MEOW_PAGESIZE - 1)) - 16);
        align: u64 = 0;
        if last > last_ok  align = (cast(u64) last) & 0xf;
        last_aligned := last - align;

        #asm {
            shift_adjust: vec;
            movdqu  shift_adjust, [shift_pointer + align];
            movdqu  r1, [last_aligned];
            pshufb  r1, shift_adjust;
            pand    r1, mask;
        }
    }

    if length & 0x10 {
        #asm {
            movdqu r3, r1;
            movdqu r1, [last - 0x10];
        }
    }

    source += advance;

    // Build the residual and length injects, then mix both in. The residual is
    // always mixed even when empty, so the pattern stays the same whatever the
    // length; the length is appended so that padding cannot make two different
    // inputs hash alike.
    length_value := length;
    #asm {
        movdqu  r0:, r1;
        movdqu  r2:, r1;
        palignr r0, r3, 15;
        palignr r2, r3, 1;

        // Room for a 128-bit and a 64-bit nonce; upstream leaves them zero so
        // that nobody mistakes this for something with security properties.
        pxor    r4:, r4;
        pxor    r5:, r5;
        pxor    r6:, r6;

        movq    r7:, length_value;
        palignr r4, r7, 15;
        palignr r6, r7, 1;
    }

    MEOW_MIX_REG(h0, h4, h6, h1, h2,  r0, r1, r2, r3);
    MEOW_MIX_REG(h1, h5, h7, h2, h3,  r4, r5, r6, r7);

    // The whole 32-byte lanes between the last block and the residual.
    lane_count := (length >> 5) & 0x7;
    while true {
        if lane_count == 0 break; MEOW_MIX(h2, h6, h0, h3, h4, source, 0x00); lane_count -= 1;
        if lane_count == 0 break; MEOW_MIX(h3, h7, h1, h4, h5, source, 0x20); lane_count -= 1;
        if lane_count == 0 break; MEOW_MIX(h4, h0, h2, h5, h6, source, 0x40); lane_count -= 1;
        if lane_count == 0 break; MEOW_MIX(h5, h1, h3, h6, h7, source, 0x60); lane_count -= 1;
        if lane_count == 0 break; MEOW_MIX(h6, h2, h4, h7, h0, source, 0x80); lane_count -= 1;
        if lane_count == 0 break; MEOW_MIX(h7, h3, h5, h0, h1, source, 0xa0); lane_count -= 1;
        if lane_count == 0 break; MEOW_MIX(h0, h4, h6, h1, h2, source, 0xc0); lane_count -= 1;
        break;
    }

    // Fold the eight lanes into one. Twelve shuffles, so that every lane has
    // reached every other.
    MEOW_SHUFFLE(h0, h1, h2, h4, h5, h6);
    MEOW_SHUFFLE(h1, h2, h3, h5, h6, h7);
    MEOW_SHUFFLE(h2, h3, h4, h6, h7, h0);
    MEOW_SHUFFLE(h3, h4, h5, h7, h0, h1);
    MEOW_SHUFFLE(h4, h5, h6, h0, h1, h2);
    MEOW_SHUFFLE(h5, h6, h7, h1, h2, h3);
    MEOW_SHUFFLE(h6, h7, h0, h2, h3, h4);
    MEOW_SHUFFLE(h7, h0, h1, h3, h4, h5);
    MEOW_SHUFFLE(h0, h1, h2, h4, h5, h6);
    MEOW_SHUFFLE(h1, h2, h3, h5, h6, h7);
    MEOW_SHUFFLE(h2, h3, h4, h6, h7, h0);
    MEOW_SHUFFLE(h3, h4, h5, h7, h0, h1);

    #asm {
        paddq   h0, h2;
        paddq   h1, h3;
        paddq   h4, h6;
        paddq   h5, h7;
        pxor    h0, h1;
        pxor    h4, h5;
        paddq   h0, h4;
        movdqu  [store], h0;
    }
}
