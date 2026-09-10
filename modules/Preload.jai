//
// Preload: the declarations every program is compiled with (**L§17**).
//
// This module is an ABI rather than a library. The compiler reads the layout
// of the `Type_Info` family out of *this file* when it builds a program's type
// table, and looks several names up directly, so a member renamed here is a
// member the compiler can no longer find. The declarations marked @Volatile
// below are the ones it reads; everything else is ordinary code.
//
// orangejuice's own distribution. Written against `docs/language.md` §17,
// which is the normative description of what a Preload must provide.
//

// ---------------------------------------------------------------- target ---

Operating_System_Tag :: enum u32 {
    NONE      :: 0;
    KRAMPOS   :: 1;
    WINDOWS   :: 2;
    LINUX     :: 3;
    ANDROID   :: 4;
    IOS       :: 5;
    MACOS     :: 6;
    NN_SWITCH :: 7;
    PS4       :: 8;
    PS5       :: 9;
    XBOX      :: 10;
    WASM      :: 11;
}

CPU_Tag :: enum u32 {
    UNINITIALIZED :: 0;
    KRAMPU        :: 1;
    CUSTOM        :: 2;
    X64           :: 3;
    ARM64         :: 4;
}

// `OS`, `CPU`, `IS_CROSS_COMPILING` and `MACHINE_OPTIONS_SIZE` are injected by
// the compiler rather than declared here: they are what the workspace is being
// built for, which this file cannot know (**L§4.3**).

// ------------------------------------------------------------- type info ---

// @Volatile: the compiler writes the type table image against these layouts,
// finding each field by name. Tag values are the ones **L§17** fixes; 17 is
// deliberately unused.
Type_Info_Tag :: enum u32 {
    INTEGER              :: 0;
    FLOAT                :: 1;
    BOOL                 :: 2;
    STRING               :: 3;
    POINTER              :: 4;
    PROCEDURE            :: 5;
    VOID                 :: 6;
    STRUCT               :: 7;
    ARRAY                :: 8;
    OVERLOAD_SET         :: 9;
    ANY                  :: 10;
    ENUM                 :: 11;
    POLYMORPHIC_VARIABLE :: 12;
    TYPE                 :: 13;
    CODE                 :: 14;
    UNTYPED_LITERAL      :: 15;
    UNTYPED_ENUM         :: 16;
    VARIANT              :: 18;
}

// `runtime_size` is -1 for a type whose size is not known: one still being
// built, or a polymorphic one that never will be.
Type_Info :: struct {
    type:         Type_Info_Tag;
    runtime_size: s64;
}

Type_Info_Integer :: struct {
    using #as info: Type_Info;
    signed: bool;
}

Type_Info_Float :: struct {
    using #as info: Type_Info;
}

Type_Info_String :: struct {
    using #as info: Type_Info;
}

Type_Info_Pointer :: struct {
    using #as info: Type_Info;
    pointer_to: *Type_Info;
}

Type_Info_Procedure :: struct {
    using #as info: Type_Info;
    argument_types:  [] *Type_Info;
    return_types:    [] *Type_Info;
    procedure_flags: Flags;

    Flags :: enum_flags u32 {
        IS_ELSEWHERE                 :: 0x1;
        IS_COMPILE_TIME_ONLY         :: 0x2;
        IS_POLYMORPHIC               :: 0x4;
        HAS_NO_CONTEXT               :: 0x8;
        IS_C_CALL                    :: 0x20;
        IS_INTRINSIC                 :: 0x80;
        IS_SYMMETRIC                 :: 0x100;
        IS_CPP_METHOD                :: 0x1000_0000;
        HAS_CPP_NON_POD_RETURN_TYPE  :: 0x2000_0000;
    }
}

Type_Info_Struct_Member :: struct {
    name:            string;
    type:            *Type_Info;
    offset_in_bytes: s64;
    flags:           Flags;
    notes:           [] string;
    // Where a constant member's bytes sit inside the struct's
    // `constant_storage`; -1 when the member is not a constant.
    offset_into_constant_storage: s64 = -1;

    Flags :: enum_flags u32 {
        CONSTANT                              :: 0x1;
        IMPORTED                              :: 0x2;
        USING                                 :: 0x4;
        PROCEDURE_WITH_VOID_POINTER_TYPE_INFO :: 0x8;
        AS                                    :: 0x10;
    }
}

Type_Info_Struct :: struct {
    using #as info: Type_Info;
    name: string;
    // The arguments a polymorphic struct was baked with, when it is one.
    specified_parameters: [] Type_Info_Struct_Member;
    members:              [] Type_Info_Struct_Member;
    status_flags:         Struct_Status_Flags;
    nontextual_flags:     Struct_Nontextual_Flags;
    textual_flags:        Struct_Textual_Flags;
    // The family this one was baked from, when it was baked from one.
    polymorph_source_struct: *Type_Info_Struct;
    initializer:             (*void) #no_context;
    constant_storage:        [] u8;
    notes:                   [] string;

    Struct_Status_Flags :: enum_flags u32 {
        INCOMPLETE :: 0x1;
    }

    Struct_Nontextual_Flags :: enum_flags u32 {
        POLYMORPHIC :: 0x1;
    }

    Struct_Textual_Flags :: enum_flags u32 {
        UNION      :: 0x1;
        NO_PADDING :: 0x2;
        FOREIGN    :: 0x4;
        TYPE_INFO_NONE :: 0x8;
        TYPE_INFO_NO_SIZE_COMPLAINT :: 0x10;
    }
}

Type_Info_Array :: struct {
    using #as info: Type_Info;
    element_type: *Type_Info;
    array_type:   Array_Type;
    // The length, when the array has one written into its type.
    array_count:  s64 = -1;

    Array_Type :: enum u16 {
        FIXED     :: 0;
        VIEW      :: 1;
        RESIZABLE :: 2;
    }
}

Type_Info_Enum :: struct {
    using #as info: Type_Info;
    name:            string;
    internal_type:   *Type_Info_Integer;
    names:           [] string;
    values:          [] s64;
    status_flags:    Enum_Status_Flags;
    enum_type_flags: Enum_Type_Flags;

    Enum_Status_Flags :: enum_flags u32 {
        INCOMPLETE :: 0x1;
    }

    Enum_Type_Flags :: enum_flags u32 {
        FLAGS     :: 0x1;
        COMPLETE  :: 0x2;
        SPECIFIED :: 0x4;
    }
}

Type_Info_Variant :: struct {
    using #as info: Type_Info;
    name:         string;
    variant_of:   *Type_Info;
    variant_flags: Variant_Flags;

    Variant_Flags :: enum_flags u32 {
        DISTINCT :: 0x1;
        ISA      :: 0x2;
    }
}

// ------------------------------------------------- the shapes of a value ---

// @Volatile: the layout the back end gives an `Any`.
Any_Struct :: struct {
    type:          *Type_Info;
    value_pointer: *void;
}

// @Volatile: `string`.
Newstring :: struct {
    count: s64;
    data:  *u8;
}

// @Volatile: `[] T`.
Array_View_64 :: struct {
    count: s64;
    data:  *u8;
}

// @Volatile: `[..] T`. A resizable array remembers what allocated it.
Resizable_Array :: struct {
    count:     s64;
    data:      *void;
    allocated: s64;
    allocator: Allocator;
}

Procedure_With_Data :: struct {
    proc: *void;
    data: *void;
}

Name_Mapper :: #type ([] string);

// A machine register an `#asm` block names (**L§15**).
__reg :: #type,distinct u16;

Workspace :: s64;

// ------------------------------------------------------------ allocators ---

Allocator_Proc :: #type (mode: Allocator_Mode, requested_size: s64, old_size: s64, old_memory: *void, allocator_data: *void) -> *void;

Allocator :: struct {
    proc: Allocator_Proc;
    data: *void;
}

Allocator_Mode :: enum {
    ALLOCATE      :: 0;
    RESIZE        :: 1;
    FREE          :: 2;
    STARTUP       :: 3;
    SHUTDOWN      :: 4;
    THREAD_START  :: 5;
    THREAD_STOP   :: 6;
    CREATE_HEAP   :: 7;
    DESTROY_HEAP  :: 8;
    IS_THIS_YOURS :: 9;
    CAPS          :: 10;
}

Allocator_Caps :: enum_flags {
    MULTIPLE_THREADS;
    CREATE_HEAP;
    FREE;
    ACTUALLY_RESIZE;
    IS_THIS_YOURS;

    HINT_I_AM_A_FAST_BUMP_ALLOCATOR       :: 0x0100_0000;
    HINT_I_AM_A_GENERAL_HEAP_ALLOCATOR    :: 0x0200_0000;
    HINT_I_AM_PER_FRAME_TEMPORARY_STORAGE :: 0x0400_0000;
    HINT_I_AM_A_DEBUG_ALLOCATOR           :: 0x0800_0000;
}

// --------------------------------------------------------------- logging ---

Logger :: #type (message: string, data: *void, info: Log_Info);

Log_Section :: struct {
    name: string;
}

Log_Flags :: enum_flags u32 {
    NONE              :: 0;
    ERROR             :: 0x1;
    WARNING           :: 0x2;
    CONTENT           :: 0x4;
    TO_FILE_ONLY      :: 0x8;
    VERBOSE_ONLY      :: 0x10;
    VERY_VERBOSE_ONLY :: 0x20;
    TOPIC_ONLY        :: 0x40;
}

Log_Level :: enum u8 {
    NORMAL       :: 0;
    VERBOSE      :: 1;
    VERY_VERBOSE :: 2;
}

Log_Info :: struct {
    source_identifier: u64;
    location:          Source_Code_Location;
    common_flags:      Log_Flags;
    user_flags:        u32;
    section:           *Log_Section;
}

// ---------------------------------------------------------- stack traces ---

Stack_Trace_Procedure_Info :: struct {
    name:              string;
    location:          Source_Code_Location;
    procedure_address: *void;
}

// One frame of the list rooted at `context.stack_trace`. A call site updates
// `line_number` before it calls, so a trace names the line rather than the
// procedure only (**L§10.6**).
Stack_Trace_Node :: struct {
    next:        *Stack_Trace_Node;
    info:        *Stack_Trace_Procedure_Info;
    hash:        u64;
    call_depth:  u32;
    line_number: u32;
}

// ------------------------------------------------------------- locations ---

// @Volatile: what `#caller_location` and `#location` produce.
Source_Code_Location :: struct {
    fully_pathed_filename: string;
    line_number:           s64;
    character_number:      s64;
}

Source_Code_Range :: struct {
    fully_pathed_filename:  string;
    line_number_start:      s64;
    line_number_end:        s64;
    character_number_start: s64;
    character_number_end:   s64;
}

// What a `for` was written with (**L§6.5**).
For_Flags :: enum_flags u8 {
    POINTER      :: 0x1;
    REVERSE      :: 0x2;
    TEMPORARY_V2 :: 0x4;
}

// ----------------------------------------------------------- the program ---

__command_line_arguments: [] *u8;

// Set once an assertion has already failed, so that reporting a failure
// cannot recurse through the machinery that reports it.
__runtime_support_disable_stack_trace := false;

// The workspace being compiled, which is 0 in a program that is running
// rather than being compiled (**C§3.1**).
get_current_workspace :: () -> Workspace #compiler;

// ------------------------------------------------------------ intrinsics ---

memcpy :: (dest: *void, source: *void, count: s64) #intrinsic;
memset :: (dest: *void, value: u8, count: s64) #intrinsic;
memcmp :: (a: *void, b: *void, count: s64) -> s16 #must #intrinsic;

// An atomic compare-exchange over anything eight bytes or smaller, floats
// included (**L§17**).
compare_and_swap :: (pointer: *$T, old: T, new: T) -> (success: bool, old_value: T) #intrinsic;

// The Context is seeded by inserting this: `Context_Base` is Runtime_Support's,
// so Preload names it without importing it (**L§10.3**).
FIRST_ADD_CONTEXT :: #code #add_context #as using base: Context_Base;
