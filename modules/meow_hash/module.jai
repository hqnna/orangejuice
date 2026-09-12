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
// **This module is x86-64, and there is no fallback.** The algorithm *is* a
// sequence of `aesdec` instructions; a software AES round would be a different
// thing wearing this one's name, and slow enough to defeat the point of it.
// The language's `#asm` is x86-64 too (**L§15**), so on another architecture
// the whole module is one `#assert` rather than fifteen errors from inside its
// own `#asm` blocks. Guard the import:
//
//     #if CPU == .X64  Meow :: #import "meow_hash";
//
// `Hash` is the portable hash to reach for instead.
//

#if CPU != .X64 {

#assert false "meow_hash is x86-64 only: it is built out of AES-NI instructions, which the language's '#asm' has on no other architecture. Guard the import with '#if CPU == .X64', or use the 'Hash' module for a portable hash.";

} else {

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

// How far ahead to prefetch, and how many blocks it is worth doing it for. A
// large input needs the prefetch to reach full speed; a small one runs faster
// without it, since the prefetches compete for the same ports as the work.
MEOW_PREFETCH       :: 4096;
MEOW_PREFETCH_LIMIT :: 0x3ff;

// A hook for dumping the eight lanes between steps while debugging. It does
// nothing; the parameters are here so that a caller can fill it in.
MEOW_DUMP_STATE :: (T: string, r0: reg, r1: reg, r2: reg, r3: reg, r4: reg, r5: reg, r6: reg, r7: reg) #expand {
}

// One mixing step over four 128-bit inputs already in registers. Two AES
// rounds, which is where the mixing strength comes from, with the additions
// and xors keeping the lanes coupled.
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

// Half of a 128-bit hash register as an ordinary integer.
MeowU64From :: (Hash: reg, $INDEX: int) -> u64 #expand {
    result: u64 = ---;
    #asm { pextrq result, Hash, INDEX; }
    return result;
}

MeowU32From :: (Hash: reg, $INDEX: int) -> u32 #expand {
    result: u32 = ---;
    #asm { pextrd result, Hash, INDEX; }
    return result;
}

// Seeds the eight hash registers from the 128-byte seed.
MeowBegin :: (Seed: [128] u8, h0: reg, h1: reg, h2: reg, h3: reg, h4: reg, h5: reg, h6: reg, h7: reg) #expand {
    SeedPtr := Seed.data;
    #asm {
        movdqu h0, [SeedPtr + 0x00];
        movdqu h1, [SeedPtr + 0x10];
        movdqu h2, [SeedPtr + 0x20];
        movdqu h3, [SeedPtr + 0x30];
        movdqu h4, [SeedPtr + 0x40];
        movdqu h5, [SeedPtr + 0x50];
        movdqu h6, [SeedPtr + 0x60];
        movdqu h7, [SeedPtr + 0x70];
    }

    MEOW_DUMP_STATE("Seed", h0, h1, h2, h3, h4, h5, h6, h7);
}

// Every whole 256-byte block, eight 32-byte windows at a time. The two loops
// are the same work with and without the prefetch.
MeowAbsorbBlocks :: (BlockCount: u64, Source: *void, h0: reg, h1: reg, h2: reg, h3: reg, h4: reg, h5: reg, h6: reg, h7: reg) #expand {
    remaining := BlockCount;
    at := Source;

    if remaining > MEOW_PREFETCH_LIMIT {
        while remaining {
            #asm {
                prefetcht0 [at + (MEOW_PREFETCH + 0x00)];
                prefetcht0 [at + (MEOW_PREFETCH + 0x40)];
                prefetcht0 [at + (MEOW_PREFETCH + 0x80)];
                prefetcht0 [at + (MEOW_PREFETCH + 0xc0)];
            }

            MEOW_MIX(h0, h4, h6, h1, h2, at, 0x00);
            MEOW_MIX(h1, h5, h7, h2, h3, at, 0x20);
            MEOW_MIX(h2, h6, h0, h3, h4, at, 0x40);
            MEOW_MIX(h3, h7, h1, h4, h5, at, 0x60);
            MEOW_MIX(h4, h0, h2, h5, h6, at, 0x80);
            MEOW_MIX(h5, h1, h3, h6, h7, at, 0xa0);
            MEOW_MIX(h6, h2, h4, h7, h0, at, 0xc0);
            MEOW_MIX(h7, h3, h5, h0, h1, at, 0xe0);

            at += 0x100;
            remaining -= 1;
        }
    } else {
        while remaining {
            MEOW_MIX(h0, h4, h6, h1, h2, at, 0x00);
            MEOW_MIX(h1, h5, h7, h2, h3, at, 0x20);
            MEOW_MIX(h2, h6, h0, h3, h4, at, 0x40);
            MEOW_MIX(h3, h7, h1, h4, h5, at, 0x60);
            MEOW_MIX(h4, h0, h2, h5, h6, at, 0x80);
            MEOW_MIX(h5, h1, h3, h6, h7, at, 0xa0);
            MEOW_MIX(h6, h2, h4, h7, h0, at, 0xc0);
            MEOW_MIX(h7, h3, h5, h0, h1, at, 0xe0);

            at += 0x100;
            remaining -= 1;
        }
    }
}

// Mixes in the residual and the length, then the whole 32-byte lanes left
// over, then folds the eight lanes down into `h0`. `Store128` non-null gets
// the eight lanes before the fold.
MeowEnd :: (Len: u64, Source: *void, r1: reg, r3: reg, h0: reg, h1: reg, h2: reg, h3: reg, h4: reg, h5: reg, h6: reg, h7: reg, Store128: *void) #expand {
    length_value := Len;
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

    MEOW_DUMP_STATE("PostBlocks", h0, h1, h2, h3, h4, h5, h6, h7);
    MEOW_DUMP_STATE("Residuals", r0, r1, r2, r3, r4, r5, r6, r7);

    // The residual is always mixed, even when it is empty, so that the pattern
    // is the same whatever the length; the length goes in after it, so that
    // the 32-byte padding cannot make two different inputs hash alike.
    MEOW_MIX_REG(h0, h4, h6, h1, h2,  r0, r1, r2, r3);
    MEOW_MIX_REG(h1, h5, h7, h2, h3,  r4, r5, r6, r7);

    MEOW_DUMP_STATE("PostAppend", h0, h1, h2, h3, h4, h5, h6, h7);

    lane_count := (Len >> 5) & 0x7;
    while true {
        if lane_count == 0 break; MEOW_MIX(h2, h6, h0, h3, h4, Source, 0x00); lane_count -= 1;
        if lane_count == 0 break; MEOW_MIX(h3, h7, h1, h4, h5, Source, 0x20); lane_count -= 1;
        if lane_count == 0 break; MEOW_MIX(h4, h0, h2, h5, h6, Source, 0x40); lane_count -= 1;
        if lane_count == 0 break; MEOW_MIX(h5, h1, h3, h6, h7, Source, 0x60); lane_count -= 1;
        if lane_count == 0 break; MEOW_MIX(h6, h2, h4, h7, h0, Source, 0x80); lane_count -= 1;
        if lane_count == 0 break; MEOW_MIX(h7, h3, h5, h0, h1, Source, 0xa0); lane_count -= 1;
        if lane_count == 0 break; MEOW_MIX(h0, h4, h6, h1, h2, Source, 0xc0); lane_count -= 1;
        break;
    }

    MEOW_DUMP_STATE("PostLanes", h0, h1, h2, h3, h4, h5, h6, h7);

    // Twelve shuffles, so that every lane has reached every other.
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

    MEOW_DUMP_STATE("PostMix", h0, h1, h2, h3, h4, h5, h6, h7);

    #if !is_constant(Store128) {
        if Store128 #asm {
            movdqu [Store128 + 0x00], h0;
            movdqu [Store128 + 0x10], h1;
            movdqu [Store128 + 0x20], h2;
            movdqu [Store128 + 0x30], h3;
            movdqu [Store128 + 0x40], h4;
            movdqu [Store128 + 0x50], h5;
            movdqu [Store128 + 0x60], h6;
            movdqu [Store128 + 0x70], h7;
        }
    }

    #asm {
        paddq   h0, h2;
        paddq   h1, h3;
        paddq   h4, h6;
        paddq   h5, h7;
        pxor    h0, h1;
        pxor    h4, h5;
        paddq   h0, h4;
    }

    MEOW_DUMP_STATE("PostFold", h0, h1, h2, h3, h4, h5, h6, h7);
}

// The whole input at once. The answer is left in `h0`, which the caller has to
// declare — a register cannot be returned.
MeowHash :: (Seed: [128] u8, Len: u64, Source: *void, h0: reg) #expand {
    #asm {
        h1: vec; h2: vec; h3: vec; h4: vec; h5: vec; h6: vec; h7: vec;
    }

    MeowBegin(Seed, h0, h1, h2, h3, h4, h5, h6, h7);

    BlockCount := Len >> 8;
    Advance := BlockCount << 8;
    MeowAbsorbBlocks(BlockCount, Source, h0, h1, h2, h3, h4, h5, h6, h7);

    #asm {
        pxor r1:, r1;
        pxor r3:, r3;
    }

    Last := Source + (Len & ~cast(u64) 0xf);
    Len8 := Len & 0xf;
    if Len8 {
        MeowMaskLenPtr := MeowMaskLen.data;
        MeowShiftAdjustPtr := MeowShiftAdjust.data;

        MaskIndex := 0x10 - Len8;
        #asm {
            movdqu Mask:, [MeowMaskLenPtr + MaskIndex];
        }

        // Reading sixteen bytes from `Last` could cross into the next page.
        // Where it would, the read is backed up to an alignment boundary and
        // the bytes shifted into place instead.
        LastOk := cast(*void) (((cast(u64) Source + Len - 1) | (MEOW_PAGESIZE - 1)) - 16);

        Align: u64 = 0;
        if Last > LastOk  Align = (cast(u64) Last) & 0xf;

        LastAligned := Last - Align;

        #asm {
            movdqu  ShiftAdjust:, [MeowShiftAdjustPtr + Align];
            movdqu  r1, [LastAligned];
            pshufb  r1, ShiftAdjust;
            pand    r1, Mask;
        }
    }

    if Len & 0x10 #asm {
        movdqu r3, r1;
        movdqu r1, [Last - 0x10];
    }

    Tail := Source + Advance;
    MeowEnd(Len, Tail, r1, r3, h0, h1, h2, h3, h4, h5, h6, h7, null);
}

// -------------------------------------------------------------- streaming ---

// For input that does not arrive all at once. The register file is kept in
// memory between calls, since a register cannot live across one; `Pad` is
// there so that the tail may be over-read by a whole register without running
// off the end.
meow_state :: struct {
    RegisterFile: [256] u8;
    TotalLengthInBytes: u64;

    BufferLen: u32;

    Buffer: [256] u8;
    Pad: [32] u8;
}

MeowBegin :: (State: *meow_state, Seed: [128] u8) {
    #asm { h0: vec; h1: vec; h2: vec; h3: vec; h4: vec; h5: vec; h6: vec; h7: vec; }
    MeowBegin(Seed, h0, h1, h2, h3, h4, h5, h6, h7);

    RegisterFilePtr := State.RegisterFile.data;
    #asm {
        movdqu [RegisterFilePtr + 0x00], h0;
        movdqu [RegisterFilePtr + 0x10], h1;
        movdqu [RegisterFilePtr + 0x20], h2;
        movdqu [RegisterFilePtr + 0x30], h3;
        movdqu [RegisterFilePtr + 0x40], h4;
        movdqu [RegisterFilePtr + 0x50], h5;
        movdqu [RegisterFilePtr + 0x60], h6;
        movdqu [RegisterFilePtr + 0x70], h7;
    }

    State.BufferLen = 0;
    State.TotalLengthInBytes = 0;
}

MeowAbsorb :: (State: *meow_state, Len: u64, Source: *void) {
    RegisterFilePtr := State.RegisterFile.data;
    #asm {
        movdqu h0:, [RegisterFilePtr + 0x00];
        movdqu h1:, [RegisterFilePtr + 0x10];
        movdqu h2:, [RegisterFilePtr + 0x20];
        movdqu h3:, [RegisterFilePtr + 0x30];
        movdqu h4:, [RegisterFilePtr + 0x40];
        movdqu h5:, [RegisterFilePtr + 0x50];
        movdqu h6:, [RegisterFilePtr + 0x60];
        movdqu h7:, [RegisterFilePtr + 0x70];
    }

    remaining := Len;
    at := Source;

    State.TotalLengthInBytes += Len;

    // Whatever the last call left over is topped up to a block first.
    if State.BufferLen {
        Fill: u64 = cast(u64) State.Buffer.count - State.BufferLen;
        if Fill > remaining  Fill = remaining;

        remaining -= Fill;
        while Fill {
            State.Buffer[State.BufferLen] = (cast(*u8) at).*;
            State.BufferLen += 1;
            at += 1;
            Fill -= 1;
        }

        if State.BufferLen == State.Buffer.count {
            one: u64 = 1;
            MeowAbsorbBlocks(one, State.Buffer.data, h0, h1, h2, h3, h4, h5, h6, h7);
            State.BufferLen = 0;
        }
    }

    BlockCount := remaining >> 8;
    Advance := BlockCount << 8;
    MeowAbsorbBlocks(BlockCount, at, h0, h1, h2, h3, h4, h5, h6, h7);

    remaining -= Advance;
    at += Advance;

    while remaining {
        State.Buffer[State.BufferLen] = (cast(*u8) at).*;
        State.BufferLen += 1;
        at += 1;
        remaining -= 1;
    }

    #asm {
        movdqu [RegisterFilePtr + 0x00], h0;
        movdqu [RegisterFilePtr + 0x10], h1;
        movdqu [RegisterFilePtr + 0x20], h2;
        movdqu [RegisterFilePtr + 0x30], h3;
        movdqu [RegisterFilePtr + 0x40], h4;
        movdqu [RegisterFilePtr + 0x50], h5;
        movdqu [RegisterFilePtr + 0x60], h6;
        movdqu [RegisterFilePtr + 0x70], h7;
    }
}

// Finishes a streamed hash, leaving the answer in `h0`.
MeowEnd :: (State: *meow_state, h0: reg, Store128: *void = null) #expand {
    RegisterFilePtr := State.RegisterFile.data;
    #asm {
        movdqu h0,  [RegisterFilePtr + 0x00];
        movdqu h1:, [RegisterFilePtr + 0x10];
        movdqu h2:, [RegisterFilePtr + 0x20];
        movdqu h3:, [RegisterFilePtr + 0x30];
        movdqu h4:, [RegisterFilePtr + 0x40];
        movdqu h5:, [RegisterFilePtr + 0x50];
        movdqu h6:, [RegisterFilePtr + 0x60];
        movdqu h7:, [RegisterFilePtr + 0x70];
    }

    #asm {
        pxor r1:, r1;
        pxor r3:, r3;
    }

    Source := State.Buffer.data;
    Len := State.TotalLengthInBytes;

    // The buffer has `Pad` after it, so no alignment dance is needed here.
    Last := Source + (Len & 0xf0);
    Len8 := Len & 0xf;
    if Len8 {
        MeowMaskLenPtr := MeowMaskLen.data;
        MaskIndex := 0x10 - Len8;
        #asm {
            movdqu  Mask:, [MeowMaskLenPtr + MaskIndex];
            movdqu  r1, [Last];
            pand    r1, Mask;
        }
    }

    if State.TotalLengthInBytes & 0x10 #asm {
        movdqu r3, r1;
        movdqu r1, [Last - 0x10];
    }

    MeowEnd(Len, Source, r1, r3, h0, h1, h2, h3, h4, h5, h6, h7, Store128);
}

// ------------------------------------------------------ the simple wrapper ---

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

hash_into :: (seed: [128] u8, source: *void, length: u64, store: *u8) {
    #asm { h0: vec; }
    MeowHash(seed, length, source, h0);
    #asm { movdqu [store], h0; }
}

}
