//
// Routines that only make sense on x86-64 (**L§15**).
//
// The generic forms of these live in `Bit_Operations`, written as arithmetic
// so they work anywhere. These are the instructions themselves, for code that
// knows what it is running on and wants the one-cycle version.
//

#import "Basic";
#import "String";

// How many bits are set. `popcnt` is SSE4.2; every x86-64 worth targeting has
// had it since 2008.
popcount :: (value: u64) -> s64 {
    result: u64;
    #asm { popcnt.q result, value; }
    return cast(s64) result;
}

// The index of the lowest set bit. Undefined for zero, which is why the
// generic `Bit_Operations.count_trailing_zeros` is the one to reach for when
// zero is possible.
bit_scan_forward :: (value: u64) -> s64 {
    result: u64;
    #asm { bsf.q result, value; }
    return cast(s64) result;
}

// The index of the highest set bit. Undefined for zero.
bit_scan_reverse :: (value: u64) -> s64 {
    result: u64;
    #asm { bsr.q result, value; }
    return cast(s64) result;
}

byte_swap :: (value: u64) -> u64 {
    result := value;
    #asm { bswap.q result; }
    return result;
}

// ------------------------------------------------------------ the clock ---

// The processor's cycle counter. Useful for measuring very short things, and
// misleading for anything else: it is not a clock, the frequency varies, and
// a thread can move between cores that disagree about it.
read_timestamp_counter :: () -> u64 {
    low: u32;
    high: u32;
    #asm { rdtsc low, high; }
    return (cast(u64) high << 32) | cast(u64) low;
}

// ------------------------------------------------------- what this chip is ---

CPU_Info :: struct {
    eax: u32;
    ebx: u32;
    ecx: u32;
    edx: u32;
}

cpuid :: (leaf: u32, subleaf: u32 = 0) -> CPU_Info {
    result: CPU_Info;
    a := leaf;
    c := subleaf;
    b: u32;
    d: u32;
    #asm {
        cpuid a, b, c, d;
    }
    result.eax = a;
    result.ebx = b;
    result.ecx = c;
    result.edx = d;
    return result;
}

// The feature bits worth asking about, from leaf 1 and leaf 7.
Feature :: enum {
    SSE4_2;
    AVX;
    AVX2;
    AES;
    BMI1;
    BMI2;
    POPCNT;
}

has_feature :: (feature: Feature) -> bool {
    if feature == {
        case .SSE4_2;  return (cpuid(1).ecx & (1 << 20)) != 0;
        case .POPCNT;  return (cpuid(1).ecx & (1 << 23)) != 0;
        case .AES;     return (cpuid(1).ecx & (1 << 25)) != 0;
        case .AVX;     return (cpuid(1).ecx & (1 << 28)) != 0;
        case .BMI1;    return (cpuid(7).ebx & (1 << 3))  != 0;
        case .AVX2;    return (cpuid(7).ebx & (1 << 5))  != 0;
        case .BMI2;    return (cpuid(7).ebx & (1 << 8))  != 0;
        case;          return false;
    }
}

// The brand string the chip reports, from leaves 0x80000002..4.
get_cpu_brand :: (allocator := context.allocator) -> string {
    // Twelve registers of four bytes each, in order.
    bytes: [48] u8;
    at := 0;
    for leaf: 0..2 {
        info := cpuid(cast(u32) (0x8000_0002 + leaf));
        for word: u32.[info.eax, info.ebx, info.ecx, info.edx] {
            for shift: 0..3 {
                bytes[at] = cast(u8) ((word >> cast(u32) (8 * shift)) & 0xff);
                at += 1;
            }
        }
    }
    // The string is zero-terminated and often has leading spaces.
    text := to_string(bytes.data);
    return copy_string(trim(text), allocator);
}
