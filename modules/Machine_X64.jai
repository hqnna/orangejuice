//
// What only an x86-64 machine can answer or do.
//
// The generic forms of the bit routines live in `Bit_Operations`, written as
// arithmetic so they work anywhere. What is here is the instructions
// themselves, and the CPUID table that says which of them this chip has.
//
// The enums below are the hardware's own numbering, not ours: a feature flag
// *is* `32 * leaf + bit`, so a flag names both the CPUID register it lives in
// and its position in that register, and `check_feature` is one shift and one
// mask (**L§15**).
//

#import "Basic";
#import "String";

x86_Vendor :: enum u8 {
    UNKNOWN :: 0;
    INTEL   :: 1;
    AMD     :: 2;
}

// One CPUID result register's worth of feature bits. The name says what goes
// in EAX (and ECX, where the leaf takes a subleaf) and which register the bits
// come back in; `EXT` means the top bit of the EAX input is set.
//
// It is `u16` rather than `u8` so that the `32 *` below stays in range.
x86_Feature_Leaf :: enum u16 {
    _01H_EDX     :: 0;
    _01H_ECX     :: 1;
    _19H_EBX     :: 2;

    _01H_EXT_EDX :: 3;
    _01H_EXT_ECX :: 4;
    _08H_EXT_EBX :: 5;

    _07H_00H_EBX :: 6;
    _07H_00H_ECX :: 7;
    _07H_00H_EDX :: 8;
    _07H_01H_EAX :: 9;
    _0DH_01H_EAX :: 10;

    NUMBER       :: 11;
}

// Every feature bit, as `32 * leaf + bit` — which is what makes the whole set
// one flat bit array a leaf index can be recovered from.
x86_Feature_Flag :: enum u16 {
    FPU                  :: 32 * x86_Feature_Leaf._01H_EDX     + 0;
    VME                  :: 32 * x86_Feature_Leaf._01H_EDX     + 1;
    DE                   :: 32 * x86_Feature_Leaf._01H_EDX     + 2;
    PSE                  :: 32 * x86_Feature_Leaf._01H_EDX     + 3;
    TSC                  :: 32 * x86_Feature_Leaf._01H_EDX     + 4;
    MSR                  :: 32 * x86_Feature_Leaf._01H_EDX     + 5;
    PAE                  :: 32 * x86_Feature_Leaf._01H_EDX     + 6;
    MCE                  :: 32 * x86_Feature_Leaf._01H_EDX     + 7;
    CX8                  :: 32 * x86_Feature_Leaf._01H_EDX     + 8;
    APIC                 :: 32 * x86_Feature_Leaf._01H_EDX     + 9;
    SEP                  :: 32 * x86_Feature_Leaf._01H_EDX     + 11;
    MTRR                 :: 32 * x86_Feature_Leaf._01H_EDX     + 12;
    PGE                  :: 32 * x86_Feature_Leaf._01H_EDX     + 13;
    MCA                  :: 32 * x86_Feature_Leaf._01H_EDX     + 14;
    CMOV                 :: 32 * x86_Feature_Leaf._01H_EDX     + 15;
    PAT                  :: 32 * x86_Feature_Leaf._01H_EDX     + 16;
    PSE_36               :: 32 * x86_Feature_Leaf._01H_EDX     + 17;
    PSN                  :: 32 * x86_Feature_Leaf._01H_EDX     + 18;
    CLFLUSH              :: 32 * x86_Feature_Leaf._01H_EDX     + 19;
    DS                   :: 32 * x86_Feature_Leaf._01H_EDX     + 21;
    ACPI                 :: 32 * x86_Feature_Leaf._01H_EDX     + 22;
    MMX                  :: 32 * x86_Feature_Leaf._01H_EDX     + 23;
    FXSR                 :: 32 * x86_Feature_Leaf._01H_EDX     + 24;
    SSE                  :: 32 * x86_Feature_Leaf._01H_EDX     + 25;
    SSE2                 :: 32 * x86_Feature_Leaf._01H_EDX     + 26;
    SS                   :: 32 * x86_Feature_Leaf._01H_EDX     + 27;
    HTT                  :: 32 * x86_Feature_Leaf._01H_EDX     + 28;
    TM                   :: 32 * x86_Feature_Leaf._01H_EDX     + 29;
    IA64                 :: 32 * x86_Feature_Leaf._01H_EDX     + 30;
    PBE                  :: 32 * x86_Feature_Leaf._01H_EDX     + 31;
    SSE3                 :: 32 * x86_Feature_Leaf._01H_ECX     + 0;
    PCLMULQDQ            :: 32 * x86_Feature_Leaf._01H_ECX     + 1;
    DTEST64              :: 32 * x86_Feature_Leaf._01H_ECX     + 2;
    MONITOR              :: 32 * x86_Feature_Leaf._01H_ECX     + 3;
    DS_CPL               :: 32 * x86_Feature_Leaf._01H_ECX     + 4;
    VMX                  :: 32 * x86_Feature_Leaf._01H_ECX     + 5;
    SMX                  :: 32 * x86_Feature_Leaf._01H_ECX     + 6;
    EIST                 :: 32 * x86_Feature_Leaf._01H_ECX     + 7;
    TM2                  :: 32 * x86_Feature_Leaf._01H_ECX     + 8;
    SSSE3                :: 32 * x86_Feature_Leaf._01H_ECX     + 9;
    CNXT_ID              :: 32 * x86_Feature_Leaf._01H_ECX     + 10;
    SDBG                 :: 32 * x86_Feature_Leaf._01H_ECX     + 11;
    FMA                  :: 32 * x86_Feature_Leaf._01H_ECX     + 12;
    CX16                 :: 32 * x86_Feature_Leaf._01H_ECX     + 13;
    XTPR                 :: 32 * x86_Feature_Leaf._01H_ECX     + 14;
    PDCM                 :: 32 * x86_Feature_Leaf._01H_ECX     + 15;
    PCID                 :: 32 * x86_Feature_Leaf._01H_ECX     + 17;
    DCA                  :: 32 * x86_Feature_Leaf._01H_ECX     + 18;
    SSE4_1               :: 32 * x86_Feature_Leaf._01H_ECX     + 19;
    SSE4_2               :: 32 * x86_Feature_Leaf._01H_ECX     + 20;
    X2APIC               :: 32 * x86_Feature_Leaf._01H_ECX     + 21;
    MOVBE                :: 32 * x86_Feature_Leaf._01H_ECX     + 22;
    POPCNT               :: 32 * x86_Feature_Leaf._01H_ECX     + 23;
    TSC_DEADLINE         :: 32 * x86_Feature_Leaf._01H_ECX     + 24;
    AES                  :: 32 * x86_Feature_Leaf._01H_ECX     + 25;
    XSAVE                :: 32 * x86_Feature_Leaf._01H_ECX     + 26;
    OSXSAVE              :: 32 * x86_Feature_Leaf._01H_ECX     + 27;
    AVX                  :: 32 * x86_Feature_Leaf._01H_ECX     + 28;
    F16C                 :: 32 * x86_Feature_Leaf._01H_ECX     + 29;
    RDRAND               :: 32 * x86_Feature_Leaf._01H_ECX     + 30;
    HYPERVISOR           :: 32 * x86_Feature_Leaf._01H_ECX     + 31;
    AESKLE               :: 32 * x86_Feature_Leaf._19H_EBX     + 0;
    AESKLE_WIDE          :: 32 * x86_Feature_Leaf._19H_EBX     + 2;
    KEY_LOCKER_MSR       :: 32 * x86_Feature_Leaf._19H_EBX     + 4;
    SYSCALL_SYSRET       :: 32 * x86_Feature_Leaf._01H_EXT_EDX + 11;
    MP                   :: 32 * x86_Feature_Leaf._01H_EXT_EDX + 19;
    NX                   :: 32 * x86_Feature_Leaf._01H_EXT_EDX + 20;
    MMX_EXT              :: 32 * x86_Feature_Leaf._01H_EXT_EDX + 22;
    FXSR_OPT             :: 32 * x86_Feature_Leaf._01H_EXT_EDX + 25;
    PDPE1GB              :: 32 * x86_Feature_Leaf._01H_EXT_EDX + 26;
    RDTSCP               :: 32 * x86_Feature_Leaf._01H_EXT_EDX + 27;
    _64BIT_MODE          :: 32 * x86_Feature_Leaf._01H_EXT_EDX + 29;
    _3DNOW_EXT           :: 32 * x86_Feature_Leaf._01H_EXT_EDX + 30;
    _3DNOW               :: 32 * x86_Feature_Leaf._01H_EXT_EDX + 31;
    LAHF_SAHF            :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 0;
    CMP_LEGACY           :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 1;
    SVM                  :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 2;
    EXTAPIC              :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 3;
    CR8_LEGACY           :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 4;
    ABM                  :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 5;
    SSE4A                :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 6;
    MISALIGNED_SSE       :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 7;
    _3DNOW_PREFETCH      :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 8;
    OSVW                 :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 9;
    IBS                  :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 10;
    XOP                  :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 11;
    SKINIT               :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 12;
    WDT                  :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 13;
    LWP                  :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 15;
    FMA4                 :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 16;
    TCE                  :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 17;
    NODE_ID              :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 19;
    TBM                  :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 21;
    TOPOLOGY_EXTENSIONS  :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 22;
    PERFCTR_CORE         :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 23;
    PERFCTR_NB           :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 24;
    DBX                  :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 26;
    PERFTSC              :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 27;
    PCX_L2I              :: 32 * x86_Feature_Leaf._01H_EXT_ECX + 28;
    CLZERO               :: 32 * x86_Feature_Leaf._08H_EXT_EBX + 0;
    INVLPGB              :: 32 * x86_Feature_Leaf._08H_EXT_EBX + 3;
    RDPRU                :: 32 * x86_Feature_Leaf._08H_EXT_EBX + 4;
    MCOMMIT              :: 32 * x86_Feature_Leaf._08H_EXT_EBX + 8;
    WBNOINVD             :: 32 * x86_Feature_Leaf._08H_EXT_EBX + 9;
    FSGSBASE             :: 32 * x86_Feature_Leaf._07H_00H_EBX + 0;
    TSC_ADJUST           :: 32 * x86_Feature_Leaf._07H_00H_EBX + 1;
    SGX                  :: 32 * x86_Feature_Leaf._07H_00H_EBX + 2;
    BMI1                 :: 32 * x86_Feature_Leaf._07H_00H_EBX + 3;
    HLE                  :: 32 * x86_Feature_Leaf._07H_00H_EBX + 4;
    AVX2                 :: 32 * x86_Feature_Leaf._07H_00H_EBX + 5;
    FDP_EXCPTN_ONLY      :: 32 * x86_Feature_Leaf._07H_00H_EBX + 6;
    SMEP                 :: 32 * x86_Feature_Leaf._07H_00H_EBX + 7;
    BMI2                 :: 32 * x86_Feature_Leaf._07H_00H_EBX + 8;
    ENHANCED_REP         :: 32 * x86_Feature_Leaf._07H_00H_EBX + 9;
    INVPCID              :: 32 * x86_Feature_Leaf._07H_00H_EBX + 10;
    RTM                  :: 32 * x86_Feature_Leaf._07H_00H_EBX + 11;
    RDT_M                :: 32 * x86_Feature_Leaf._07H_00H_EBX + 12;
    DEP_FPU_CS_DS        :: 32 * x86_Feature_Leaf._07H_00H_EBX + 13;
    MPX                  :: 32 * x86_Feature_Leaf._07H_00H_EBX + 14;
    RDT_A                :: 32 * x86_Feature_Leaf._07H_00H_EBX + 15;
    AVX512F              :: 32 * x86_Feature_Leaf._07H_00H_EBX + 16;
    AVX512DQ             :: 32 * x86_Feature_Leaf._07H_00H_EBX + 17;
    RDSEED               :: 32 * x86_Feature_Leaf._07H_00H_EBX + 18;
    ADX                  :: 32 * x86_Feature_Leaf._07H_00H_EBX + 19;
    SMAP                 :: 32 * x86_Feature_Leaf._07H_00H_EBX + 20;
    AVX512_IFMA          :: 32 * x86_Feature_Leaf._07H_00H_EBX + 21;
    PCOMMIT              :: 32 * x86_Feature_Leaf._07H_00H_EBX + 22;
    CLFLUSHOPT           :: 32 * x86_Feature_Leaf._07H_00H_EBX + 23;
    CLWB                 :: 32 * x86_Feature_Leaf._07H_00H_EBX + 24;
    INTEL_PT             :: 32 * x86_Feature_Leaf._07H_00H_EBX + 25;
    AVX512PF             :: 32 * x86_Feature_Leaf._07H_00H_EBX + 26;
    AVX512ER             :: 32 * x86_Feature_Leaf._07H_00H_EBX + 27;
    AVX512CD             :: 32 * x86_Feature_Leaf._07H_00H_EBX + 28;
    SHA                  :: 32 * x86_Feature_Leaf._07H_00H_EBX + 29;
    AVX512BW             :: 32 * x86_Feature_Leaf._07H_00H_EBX + 30;
    AVX512VL             :: 32 * x86_Feature_Leaf._07H_00H_EBX + 31;
    PREFETCHWT1          :: 32 * x86_Feature_Leaf._07H_00H_ECX + 0;
    AVX512_VBMI          :: 32 * x86_Feature_Leaf._07H_00H_ECX + 1;
    UMIP                 :: 32 * x86_Feature_Leaf._07H_00H_ECX + 2;
    PKU                  :: 32 * x86_Feature_Leaf._07H_00H_ECX + 3;
    OSPKE                :: 32 * x86_Feature_Leaf._07H_00H_ECX + 4;
    WAITPKG              :: 32 * x86_Feature_Leaf._07H_00H_ECX + 5;
    AVX512_VBMI2         :: 32 * x86_Feature_Leaf._07H_00H_ECX + 6;
    CET_SS               :: 32 * x86_Feature_Leaf._07H_00H_ECX + 7;
    GFNI                 :: 32 * x86_Feature_Leaf._07H_00H_ECX + 8;
    VAES                 :: 32 * x86_Feature_Leaf._07H_00H_ECX + 9;
    VPCLMULQDQ           :: 32 * x86_Feature_Leaf._07H_00H_ECX + 10;
    AVX512_VNNI          :: 32 * x86_Feature_Leaf._07H_00H_ECX + 11;
    AVX512_BITALG        :: 32 * x86_Feature_Leaf._07H_00H_ECX + 12;
    TME_EN               :: 32 * x86_Feature_Leaf._07H_00H_ECX + 13;
    AVX512_VPOPCNTDQ     :: 32 * x86_Feature_Leaf._07H_00H_ECX + 14;
    FIVE_LEVEL_PAGING    :: 32 * x86_Feature_Leaf._07H_00H_ECX + 16;
    MAWAU_0              :: 32 * x86_Feature_Leaf._07H_00H_ECX + 17;
    MAWAU_1              :: 32 * x86_Feature_Leaf._07H_00H_ECX + 18;
    MAWAU_2              :: 32 * x86_Feature_Leaf._07H_00H_ECX + 19;
    MAWAU_3              :: 32 * x86_Feature_Leaf._07H_00H_ECX + 20;
    MAWAU_4              :: 32 * x86_Feature_Leaf._07H_00H_ECX + 21;
    RDPID                :: 32 * x86_Feature_Leaf._07H_00H_ECX + 22;
    KL                   :: 32 * x86_Feature_Leaf._07H_00H_ECX + 23;
    CLDEMOTE             :: 32 * x86_Feature_Leaf._07H_00H_ECX + 25;
    MOVDIRI              :: 32 * x86_Feature_Leaf._07H_00H_ECX + 27;
    MOVDIR64B            :: 32 * x86_Feature_Leaf._07H_00H_ECX + 28;
    ENQCMD               :: 32 * x86_Feature_Leaf._07H_00H_ECX + 29;
    SGX_LC               :: 32 * x86_Feature_Leaf._07H_00H_ECX + 30;
    PKS                  :: 32 * x86_Feature_Leaf._07H_00H_ECX + 31;
    AVX512_4VNNIW        :: 32 * x86_Feature_Leaf._07H_00H_EDX + 2;
    AVX512_4FMAPS        :: 32 * x86_Feature_Leaf._07H_00H_EDX + 3;
    FSRM                 :: 32 * x86_Feature_Leaf._07H_00H_EDX + 4;
    AVX512_VP2INTERSECT  :: 32 * x86_Feature_Leaf._07H_00H_EDX + 8;
    SRBDS_CTRL           :: 32 * x86_Feature_Leaf._07H_00H_EDX + 9;
    MD_CLEAR             :: 32 * x86_Feature_Leaf._07H_00H_EDX + 10;
    TSX_FORCE_ABORT      :: 32 * x86_Feature_Leaf._07H_00H_EDX + 13;
    SERIALIZE            :: 32 * x86_Feature_Leaf._07H_00H_EDX + 14;
    HYBRID               :: 32 * x86_Feature_Leaf._07H_00H_EDX + 15;
    TSXLDTRK             :: 32 * x86_Feature_Leaf._07H_00H_EDX + 16;
    PCONFIG              :: 32 * x86_Feature_Leaf._07H_00H_EDX + 18;
    LBR                  :: 32 * x86_Feature_Leaf._07H_00H_EDX + 19;
    CET_IBT              :: 32 * x86_Feature_Leaf._07H_00H_EDX + 20;
    AMX_BF16             :: 32 * x86_Feature_Leaf._07H_00H_EDX + 22;
    AVX512_FP16          :: 32 * x86_Feature_Leaf._07H_00H_EDX + 23;
    AMX_TILE             :: 32 * x86_Feature_Leaf._07H_00H_EDX + 24;
    AMX_INT8             :: 32 * x86_Feature_Leaf._07H_00H_EDX + 25;
    SPEC_CTRL            :: 32 * x86_Feature_Leaf._07H_00H_EDX + 26;
    STIBP                :: 32 * x86_Feature_Leaf._07H_00H_EDX + 27;
    L1D_FLUSH            :: 32 * x86_Feature_Leaf._07H_00H_EDX + 28;
    ARCH_CAPABILITIES    :: 32 * x86_Feature_Leaf._07H_00H_EDX + 29;
    CORE_CAPABILITIES    :: 32 * x86_Feature_Leaf._07H_00H_EDX + 30;
    SSBD                 :: 32 * x86_Feature_Leaf._07H_00H_EDX + 31;
    AVX_VNNI             :: 32 * x86_Feature_Leaf._07H_01H_EAX + 4;
    AVX512_BF16          :: 32 * x86_Feature_Leaf._07H_01H_EAX + 5;
    FZRM                 :: 32 * x86_Feature_Leaf._07H_01H_EAX + 10;
    FSRS                 :: 32 * x86_Feature_Leaf._07H_01H_EAX + 11;
    FSRCS                :: 32 * x86_Feature_Leaf._07H_01H_EAX + 12;
    HRESET               :: 32 * x86_Feature_Leaf._07H_01H_EAX + 22;
    LAM                  :: 32 * x86_Feature_Leaf._07H_01H_EAX + 26;
    XSAVEOPT             :: 32 * x86_Feature_Leaf._0DH_01H_EAX + 0;
    XSAVEC               :: 32 * x86_Feature_Leaf._0DH_01H_EAX + 1;
    XGETBV_ECX1          :: 32 * x86_Feature_Leaf._0DH_01H_EAX + 2;
    XSAVES_XRSTORS       :: 32 * x86_Feature_Leaf._0DH_01H_EAX + 3;
    XFD                  :: 32 * x86_Feature_Leaf._0DH_01H_EAX + 4;
}

// What `get_cpu_info` found out.
Cpu_X86 :: struct {
    vendor: x86_Vendor;
    feature_leaves: [x86_Feature_Leaf.NUMBER] u32;
}

// Asks the chip what it is and what it can do. A leaf is only asked for when
// the chip says it goes that far: CPUID answers a leaf past its maximum with
// whatever the highest one holds, which would read as a wild set of features.
get_cpu_info :: () -> Cpu_X86 {
    cpu: Cpu_X86;

    max_basic:    u32 = ---;
    max_extended: u32 = ---;

    // Leaf 0 gives the highest basic leaf in EAX and the twelve bytes of the
    // vendor string across EBX, EDX, ECX — in that order, which is why the
    // chunks below are not read in register order.
    vendor_0: u32 = ---;
    vendor_1: u32 = ---;
    vendor_2: u32 = ---;

    #asm {
        xor.d a: gpr === a, a;
        xor.d c: gpr === c, c;
        cpuid a, b:, c, d:;
        mov.d max_basic, a;

        mov.d vendor_0, b;
        mov.d vendor_1, d;
        mov.d vendor_2, c;

        mov.d a, 0x8000_0000;
        xor.d c, c;
        cpuid a, b, c, d;
        mov.d max_extended, a;
    }

    if vendor_0 == 0x756E_6547 && vendor_1 == 0x4965_6E69 && vendor_2 == 0x6C65_746E  cpu.vendor = .INTEL;   // "GenuineIntel"
    if vendor_0 == 0x6874_7541 && vendor_1 == 0x6974_6E65 && vendor_2 == 0x444D_4163  cpu.vendor = .AMD;     // "AuthenticAMD"

    leaf_01_edx := *cpu.feature_leaves[x86_Feature_Leaf._01H_EDX];
    leaf_01_ecx := *cpu.feature_leaves[x86_Feature_Leaf._01H_ECX];

    if max_basic >= 0x01 #asm {
        mov.d a: gpr === a, 0x01;
        xor.d c: gpr === c, c;
        cpuid a, b:, c, d:;
        mov.d [leaf_01_edx], d;
        mov.d [leaf_01_ecx], c;
    }

    leaf_19_ebx := *cpu.feature_leaves[x86_Feature_Leaf._19H_EBX];

    if max_basic >= 0x19 #asm {
        mov.d a: gpr === a, 0x19;
        xor.d c: gpr === c, c;
        cpuid a, b:, c, d:;
        mov.d [leaf_19_ebx], b;
    }

    leaf_ext_01_edx := *cpu.feature_leaves[x86_Feature_Leaf._01H_EXT_EDX];
    leaf_ext_01_ecx := *cpu.feature_leaves[x86_Feature_Leaf._01H_EXT_ECX];

    if max_extended >= 0x8000_0001 #asm {
        mov.d a: gpr === a, 0x8000_0001;
        xor.d c: gpr === c, c;
        cpuid a, b:, c, d:;
        mov.d [leaf_ext_01_edx], d;
        mov.d [leaf_ext_01_ecx], c;
    }

    leaf_ext_08_ebx := *cpu.feature_leaves[x86_Feature_Leaf._08H_EXT_EBX];

    if max_extended >= 0x8000_0008 #asm {
        mov.d a: gpr === a, 0x8000_0008;
        xor.d c: gpr === c, c;
        cpuid a, b:, c, d:;
        mov.d [leaf_ext_08_ebx], b;
    }

    leaf_07_00_ebx := *cpu.feature_leaves[x86_Feature_Leaf._07H_00H_EBX];
    leaf_07_00_ecx := *cpu.feature_leaves[x86_Feature_Leaf._07H_00H_ECX];
    leaf_07_00_edx := *cpu.feature_leaves[x86_Feature_Leaf._07H_00H_EDX];
    leaf_07_01_eax := *cpu.feature_leaves[x86_Feature_Leaf._07H_01H_EAX];

    if max_basic >= 0x07 #asm {
        mov.d a: gpr === a, 0x07;
        xor.d c: gpr === c, c;
        cpuid a, b:, c, d:;
        mov.d [leaf_07_00_ebx], b;
        mov.d [leaf_07_00_ecx], c;
        mov.d [leaf_07_00_edx], d;

        mov.d a, 0x07;
        mov.d c, 0x01;
        cpuid a, b, c, d;
        mov.d [leaf_07_01_eax], a;
    }

    leaf_0d_01_eax := *cpu.feature_leaves[x86_Feature_Leaf._0DH_01H_EAX];

    if max_basic >= 0x0D #asm {
        mov.d a: gpr === a, 0x0D;
        mov.d c: gpr === c, 0x01;
        cpuid a, b:, c, d:;
        mov.d [leaf_0d_01_eax], a;
    }

    return cpu;
}

// The flag's own value says which leaf to look in and which bit of it to test.
check_feature :: (leaves: [x86_Feature_Leaf.NUMBER] u32, flag: x86_Feature_Flag) -> bool {
    return (leaves[flag >> 5] & (cast(u32) 1 << (flag & 31))) != 0;
}

// A compilation may claim a feature the machine it is built on does not have,
// or take one away from a chip that has it, which is what these two are for.
enable_feature :: (leaves: *[x86_Feature_Leaf.NUMBER] u32, flag: x86_Feature_Flag) {
    leaves.*[flag >> 5] |= cast(u32) 1 << (flag & 31);
}

disable_feature :: (leaves: *[x86_Feature_Leaf.NUMBER] u32, flag: x86_Feature_Flag) {
    leaves.*[flag >> 5] &= ~(cast(u32) 1 << (flag & 31));
}

// ------------------------------------------------- the instructions themselves ---

// Which cache level the prefetched line should land in. `NTA` means the line
// is wanted once and should not evict anything.
Prefetch_Hint :: enum {
    T0  :: 0;
    T1  :: 1;
    T2  :: 2;
    NTA :: 3;
}

prefetch :: (pointer: *void, $hint: Prefetch_Hint) #expand {
    #if hint == .NTA {
        #asm { prefetchnta [pointer]; }
    } else #if hint == .T0 {
        #asm { prefetcht0 [pointer]; }
    } else #if hint == .T1 {
        #asm { prefetcht1 [pointer]; }
    } else #if hint == .T2 {
        #asm { prefetcht2 [pointer]; }
    } else {
        #assert false "'hint' argument to prefetch is out of range.";
    }
}

// The processor's cycle counter. Good for measuring very short things and
// misleading for anything else: it is not a clock, and a thread that moves
// between cores may see it jump.
rdtsc :: () -> u64 #expand {
    result: u64 = ---;

    // The count comes back split across EDX:EAX, so the high half is shifted
    // up and folded in.
    #asm {
        result === a;

        rdtsc high:, result;
        shl.q high, 32;
        or.q  result, high;
    }

    return result;
}

// The same counter, plus the processor id the reading was taken on — which is
// what tells a caller whether two readings are comparable at all.
rdtscp :: () -> (timestamp: u64, msr: u32) #expand {
    result: u64 = ---;
    msr:    u32 = ---;

    #asm {
        result === a;
        msr === c;

        rdtscp high:, result, msr;
        shl.q  high, 32;
        or.q   result, high;
    }

    return result, msr;
}

// Hardware entropy straight from the sampler rather than from the generator
// seeded by it. Slower than `rdrand`, and it fails far more often; almost
// nothing needs it.
rdseed :: () -> (success: bool, value: u64) #expand {
    success: bool = ---;
    value:   u64  = ---;

    #asm {
        rdseed.q value;
        setc     success;
    }

    return success, value;
}

// The hardware random number generator. It can report failure, and Intel's own
// guidance is to retry about ten times before giving up.
rdrand :: () -> (success: bool, value: u64) #expand {
    success: bool = ---;
    value:   u64  = ---;

    #asm {
        rdrand.q value;
        setc     success;
    }

    return success, value;
}

mfence :: () #expand { #asm { mfence; } }
lfence :: () #expand { #asm { lfence; } }
sfence :: () #expand { #asm { sfence; } }

// Tells the chip this is a spin-wait, which is what keeps a tight retry loop
// from being punished for the memory-order violation it looks like.
pause :: () #expand { #asm { pause; } }

// ------------------------------------------------------------ build options ---

// The x86 half of a workspace's `machine_options`: a compilation may be told
// to assume features the build machine does not have.
Machine_Options_X86 :: struct {
    features: struct {
        leaves: [x86_Feature_Leaf.NUMBER] u32;
    }
}

// The polymorphism is what keeps `#import "Compiler"` out of this module
// unless a metaprogram actually asks for the options.
get_machine_options_x86 :: (build_options: *$T) -> *Machine_Options_X86 #modify {
    #import "Compiler";
    return (T == Build_Options);
} {
    #assert size_of(Machine_Options_X86) <= MACHINE_OPTIONS_SIZE;
    return cast(*Machine_Options_X86) *build_options.machine_options;
}
