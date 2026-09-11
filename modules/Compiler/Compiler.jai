//
// Compiler: what a metaprogram sees and says (**C§3**, **C§5.3**).
//
// This module is an ABI, not a library. Every struct here is mirrored in
// orangejuice's own `oj-meta` as a `#[repr(C)]` type, and
// `crates/meta/tests/layout.rs` measures the two against each other member by
// member — so a field reordered here is a field the compiler reads at the
// wrong offset. Nothing in this file has a body: the compiler answers the
// `#compiler` procedures itself.
//
// Written against `docs/compiler.md` §3 and §5.3, which are the normative
// description of the message protocol and the exported AST.
//

Workspace :: s64;

// ------------------------------------------------------------- messages ---

// @Volatile: `Message` is the head every message begins with, so a metaprogram
// reads `kind` and casts to the specific one.
Message :: struct {
    kind:      Message_Kind;
    workspace: Workspace;
}

Message_Kind :: enum u32 {
    UNINITIALIZED      :: 0;
    FILE               :: 1;
    IMPORT             :: 2;
    FAILED_IMPORT      :: 3;
    PHASE              :: 4;
    TYPECHECKED        :: 5;
    COMPLETE           :: 6;
    DEBUG_DUMP         :: 7;
    ERROR              :: 8;
    PERFORMANCE_REPORT :: 9;
}

Module_Type :: enum u32 {
    UNKNOWN   :: 0;
    MAIN      :: 1;
    FILE      :: 2;
    DIRECTORY :: 3;
    STRING    :: 4;
}

// A file the compilation reached, and what brought it in.
Message_File :: struct {
    #as using message: Message;
    fully_pathed_filename: string;
    enclosing_import:      *Message_Import;
    from_a_string:         bool;
}

Message_Import :: struct {
    #as using message: Message;
    module_type:           Module_Type;
    module_name:           string;
    fully_pathed_filename: string;
}

Import_Status :: enum u32 {
    UNKNOWN   :: 0;
    NOT_FOUND :: 1;
    BLOCKED   :: 2;
}

// An import that could not be satisfied. A metaprogram may answer this with
// `provide_import`, and the compilation runs again with the replacement in
// place (**C§3.3**).
Message_Failed_Import :: struct {
    #as using message: Message;
    status:             Import_Status;
    host_module_name:   string;
    target_module_name: string;
    import_code:        *Code_Directive_Import;
}

Phase :: enum u32 {
    ALL_SOURCE_CODE_PARSED       :: 0;
    TYPECHECKED_ALL_WE_CAN       :: 1;
    ALL_TARGET_CODE_BUILT        :: 2;
    PRE_WRITE_EXECUTABLE         :: 3;
    POST_WRITE_EXECUTABLE        :: 4;
    READY_FOR_CUSTOM_LINK_COMMAND :: 5;
}

Message_Phase :: struct {
    #as using message: Message;
    phase:                          Phase;
    executable_name:                string;
    executable_write_failed:        bool;
    linker_exit_code:               s32;
    num_items_waiting_to_typecheck: s32;
    compiler_generated_object_files: [] string;
    support_object_files:            [] string;
    system_libraries:                [] string;
    user_libraries:                  [] string;
}

Error_Code :: enum u8 {
    NONE                  :: 0;
    COMPILE_TIME_ERRORS   :: 1;
    ARGUMENT_ERRORS       :: 2;
    SOURCE_FILE_NOT_FOUND :: 3;
    UNDEFINED             :: 4;
}

Message_Complete :: struct {
    #as using message: Message;
    error_code: Error_Code;
}

// One typechecked tree: the node itself and everything under it, flattened in
// dependency order (**C§3.2**). Parameterised so that a batch says what kind
// of node it is carrying.
Typechecked :: struct (T: Type) {
    expression:     *T;
    subexpressions: [] *Code_Node;
}

// A batch of trees the compiler has finished typechecking. `compiler_get_nodes`
// turns one of these entries into the nodes it names.
Message_Typechecked :: struct {
    #as using message: Message;
    declarations:      [] Typechecked(Code_Declaration);
    procedure_headers: [] Typechecked(Code_Procedure_Header);
    procedure_bodies:  [] Typechecked(Code_Procedure_Body);
    structs:           [] Typechecked(Code_Struct);
    others:            [] Typechecked(Code_Node);
    // Everything above, in one list, for a plugin that does not care which
    // kind a tree is.
    all:               [] Typechecked(Code_Node);
}

// ------------------------------------------------------------ the nodes ---

// @Volatile: **C§5.3** fixes these numbers; 13 is deliberately unused.
Code_Node_Kind :: enum u8 {
    UNINITIALIZED             :: 0;
    BLOCK                     :: 1;
    LITERAL                   :: 2;
    IDENT                     :: 3;
    UNARY_OPERATOR            :: 4;
    BINARY_OPERATOR           :: 5;
    PROCEDURE_BODY            :: 6;
    PROCEDURE_CALL            :: 7;
    CONTEXT                   :: 8;
    WHILE                     :: 9;
    IF                        :: 10;
    LOOP_CONTROL              :: 11;
    CASE                      :: 12;
    RETURN                    :: 14;
    FOR                       :: 15;
    TYPE_DEFINITION           :: 16;
    TYPE_INSTANTIATION        :: 17;
    ENUM                      :: 18;
    PROCEDURE_HEADER          :: 19;
    STRUCT                    :: 20;
    COMMA_SEPARATED_ARGUMENTS :: 21;
    EXTRACT                   :: 22;
    DIRECTIVE_BYTES           :: 23;
    MAKE_VARARGS              :: 24;
    DECLARATION               :: 25;
    CAST                      :: 26;
    DIRECTIVE_IMPORT          :: 27;
    DIRECTIVE_THIS            :: 28;
    DIRECTIVE_THROUGH         :: 29;
    DIRECTIVE_LOAD            :: 30;
    DIRECTIVE_RUN             :: 31;
    DIRECTIVE_CODE            :: 32;
    DIRECTIVE_POKE_NAME       :: 33;
    ASM                       :: 34;
    DIRECTIVE_BAKE            :: 35;
    DIRECTIVE_MODIFY          :: 36;
    DIRECTIVE_LIBRARY         :: 37;
    EXPRESSION_QUERY          :: 38;
    PUSH_CONTEXT              :: 39;
    NOTE                      :: 40;
    DIRECTIVE_PLACE           :: 41;
    DIRECTIVE_SCOPE           :: 42;
    TYPE_QUERY                :: 43;
    DIRECTIVE_LOCATION        :: 44;
    DIRECTIVE_MODULE_PARAMETERS :: 45;
    DIRECTIVE_ADD_CONTEXT     :: 46;
    DIRECTIVE_COMPILE_TIME    :: 47;
    COMPOUND_DECLARATION      :: 48;
    DEFER                     :: 49;
    USING                     :: 50;
    PLACEHOLDER               :: 51;
    DIRECTIVE_INSERT          :: 52;
    DIRECTIVE_PROCEDURE_NAME  :: 53;
    DIRECTIVE_WILDCARD        :: 54;
    DIRECTIVE_EXISTS          :: 55;
    DIRECTIVE_CONTEXT_TYPE    :: 56;
    RESOLVED_OVERLOAD         :: 57;
}

Node_Flags :: enum_flags u32 {
    EXPRESSION_IS_SPREAD         :: 0x1;
    NO_ARRAY_BOUNDS_CHECK        :: 0x2;
    ALLOWED_BY_CONTEXT           :: 0x4;
    STATEMENT_IS_DEFERRED        :: 0x8;
    IS_PARENTHESIZED             :: 0x10;
    NO_ARITHMETIC_OVERFLOW_CHECK :: 0x80;
    CREATED_BY_DESUGARING        :: 0x100;
}

// @Volatile: every node struct begins with this.
Code_Node :: struct {
    kind:       Code_Node_Kind;
    node_flags: Node_Flags;
    type:       *Type_Info;
    #as using location: Location;
    serial:     s64;

    Location :: struct {
        enclosing_load: *Message_File;
        l0: s32;
        c0: s32;
        l1: s32;
        c1: s32;
    }
}

Code_Scope_Entry :: struct {
    #as using base: Code_Node;
    name:          string;
    import_target: *Code_Declaration;
}

Declaration_Flags :: enum_flags u32 {
    IS_CONSTANT                 :: 0x1;
    IS_MARKED_AS_AS             :: 0x2;
    IS_MARKED_AS_DISCARD        :: 0x4;
    IS_ITERATOR                 :: 0x10;
    NO_RESET                    :: 0x40;
    IS_UNINITIALIZED            :: 0x80;
    COMES_FROM_ASM              :: 0x100;
    IS_IMPORTED                 :: 0x200;
    AUTO_VALUE_BAKE             :: 0x4000;
    AUTO_VALUE_BAKE_IS_REQUIRED :: 0x8000;
    MUST_BE_RECEIVED            :: 0x10000;
    PROGRAM_EXPORT              :: 0x20000;
    ELSEWHERE                   :: 0x40000;
    SCOPE_FILE                  :: 0x80000;
    IS_GLOBAL                   :: 0x100000;
    HAS_SCOPE_MODIFIER          :: 0x200000;
}

Code_Declaration :: struct {
    #as using entry: Code_Scope_Entry;
    type_inst:            *Code_Type_Instantiation;
    expression:           *Code_Node;
    flags:                Declaration_Flags;
    alignment_expression: *Code_Node;
    notes:                [] *Code_Note;
    program_export_name:  string;
}

Block_Type :: enum s32 {
    UNINITIALIZED     :: 0;
    IMPERATIVE        :: 1;
    DATA_DECLARATIONS :: 2;
    ARGUMENTS         :: 3;
    RETURNS           :: 4;
    STRUCT_ARGUMENTS  :: 5;
    CONSTANTS         :: 6;
}

Block_Flags :: enum_flags u32 {
    NO_ARRAY_BOUNDS_CHECK        :: 0x1;
    NO_ARITHMETIC_OVERFLOW_CHECK :: 0x2;
}

Code_Block :: struct {
    #as using base: Code_Node;
    parent:            *Code_Block;
    block_type:        Block_Type;
    block_flags:       Block_Flags;
    belongs_to_struct: *Code_Struct;
    members:           [] *Code_Scope_Entry;
    statements:        [] *Code_Node;
    owning_statement:  *Code_Node;
}

Ident_Flags :: enum_flags u32 {
    DEFINES_POLYMORPH_VARIABLE     :: 0x1;
    IS_RHS_OF_DOT_DEREFERENCE      :: 0x2;
    RESOLVES_ONLY_SHALLOWLY        :: 0x4;
    GENERATED_BY_OPERATOR_OVERLOAD :: 0x8;
    DO_NOT_RESOLVE_DUE_TO_PARSER   :: 0x10;
    CAN_RESOLVE_TO_REG             :: 0x20;
    HAS_SCOPE_MODIFIER             :: 0x40;
}

Code_Ident :: struct {
    #as using base: Code_Node;
    name:                 string;
    resolved_declaration: *Code_Declaration;
    flags:                Ident_Flags;
}

Literal_Type :: enum s16 {
    UNINITIALIZED :: 0;
    NUMBER        :: 1;
    STRING        :: 2;
    BOOLEAN       :: 3;
    ARRAY         :: 6;
    STRUCT        :: 7;
    POINTER       :: 8;
    TYPE_INFO     :: 9;
}

Value_Flags :: enum_flags u32 {
    IS_A_NUMBER          :: 0x1;
    HEX                  :: 0x2;
    FLOAT                :: 0x4;
    BINARY               :: 0x20;
    MINUS_SIGN           :: 0x40;
    HERE_STRING          :: 0x80;
    DEFAULTS_TO_FLOAT64  :: 0x100;
    REQUIRES_FLOAT64     :: 0x200;
}

Code_Struct_Literal_Info :: struct {
    type_expression: *Code_Type_Instantiation;
    arguments:       [] *Code_Node;
}

Array_Literal_Flags :: enum_flags u8 {
    WAS_UNTYPED :: 0x1;
}

Code_Array_Literal_Info :: struct {
    element_type:        *Code_Type_Instantiation;
    alignment:           *Code_Node;
    array_members:       [] *Code_Node;
    array_literal_flags: Array_Literal_Flags;
}

Pointer_Literal_Type :: enum u8 {
    GLOBAL_SYMBOL                          :: 0;
    STRING_OR_ARRAY_LITERAL_DATA_POINTER   :: 1;
    CONSTANT_VALUE                         :: 2;
}

Code_Pointer_Literal_Info :: struct {
    global_symbol:        *Code_Declaration;
    data_pointer:         *u8;
    pointer_literal_type: Pointer_Literal_Type;
    offset_from_symbol:   s64;
}

// Floats are stored as float64 whatever they were written as (**C§5.3**).
Code_Literal_Values :: union {
    _string:                string;
    _float64:               float64;
    _s64:                   s64;
    _u64:                   u64;
    struct_literal_info:    *Code_Struct_Literal_Info;
    array_literal_info:     *Code_Array_Literal_Info;
    pointer_literal_info:   *Code_Pointer_Literal_Info;
    type_info_literal_defn: *Code_Type_Definition;
}

Code_Literal :: struct {
    #as using base: Code_Node;
    value_type:  Literal_Type;
    using values: Code_Literal_Values;
    value_flags: Value_Flags;
}

// ASCII for the single-character operators; the rest are the numbers
// **C§5.3** fixes.
Operator_Type :: enum s32 {
    IS_EQUAL              :: 131;
    IS_NOT_EQUAL          :: 132;
    LOGICAL_AND           :: 133;
    LOGICAL_OR            :: 134;
    LESS_OR_EQUAL         :: 135;
    GREATER_OR_EQUAL      :: 136;
    SHIFT_LEFT            :: 137;
    SHIFT_RIGHT           :: 138;
    ROTATE_LEFT           :: 139;
    ROTATE_RIGHT          :: 140;
    PLUS_ASSIGN           :: 145;
    MINUS_ASSIGN          :: 146;
    TIMES_ASSIGN          :: 147;
    DIV_ASSIGN            :: 148;
    MOD_ASSIGN            :: 149;
    SHIFT_LEFT_ASSIGN     :: 150;
    SHIFT_RIGHT_ASSIGN    :: 151;
    ROTATE_LEFT_ASSIGN    :: 152;
    ROTATE_RIGHT_ASSIGN   :: 153;
    BITWISE_AND_ASSIGN    :: 154;
    BITWISE_OR_ASSIGN     :: 155;
    BITWISE_XOR_ASSIGN    :: 156;
    LOGICAL_AND_ASSIGN    :: 157;
    LOGICAL_OR_ASSIGN     :: 158;
    POINTER_DEREFERENCE   :: 168;
    POSTFIX_DEREFERENCE   :: 169;
    ARRAY_SUBSCRIPT       :: 500;
}

Code_Unary_Operator :: struct {
    #as using base: Code_Node;
    operator_type: Operator_Type;
    subexpression: *Code_Node;
}

Binary_Operator_Flags :: enum_flags u32 {
    SHIFT_MARKED_AS_LOGICAL :: 0x1;
    SHIFT_MARKED_AS_SMALL   :: 0x2;
}

// Member access is a binary `.`, with the right side an IDENT flagged
// `IS_RHS_OF_DOT_DEREFERENCE`; a subscript is `ARRAY_SUBSCRIPT` (**C§5.3**).
Code_Binary_Operator :: struct {
    #as using base: Code_Node;
    operator_type: Operator_Type;
    flags:         Binary_Operator_Flags;
    left:          *Code_Node;
    right:         *Code_Node;
}

Code_Argument :: struct {
    expression: *Code_Node;
    name:       *Code_Ident;
}

Context_Modification :: struct {
    modification_expressions: [] *Code_Node;
}

Procedure_Call_Flags :: enum_flags u32 {
    INLINE_YES                     :: 0x1;
    INLINE_NO                      :: 0x2;
    RETURNS_PROCEDURE_POINTER_ONLY :: 0x10;
    NO_DEBUG                       :: 0x100;
    IS_MODULE_PARAMETERS           :: 0x200;
}

// Also used for struct instantiations and macro invocations (**C§5.3**).
Code_Procedure_Call :: struct {
    #as using base: Code_Node;
    procedure_expression:          *Code_Node;
    resolved_procedure_expression: *Code_Node;
    // Slot 0 is the overload resolution chose.
    overloads:                     *[] *Code_Declaration;
    arguments_unsorted:            [] Code_Argument;
    arguments_sorted:              [] *Code_Node;
    num_return_values_received:    s64;
    macro_expansion_block:         *Code_Block;
    context_modification:          *Context_Modification;
    flags:                         Procedure_Call_Flags;
}

Procedure_Flags :: enum_flags u32 {
    ELSEWHERE                             :: 0x1;
    COMPILE_TIME_ONLY                     :: 0x2;
    POLYMORPHIC                           :: 0x4;
    COMPILER_GENERATED                    :: 0x8;
    DEBUG_DUMP                            :: 0x10;
    C_CALL                                :: 0x20;
    TYPE_ONLY                             :: 0x40;
    INTRINSIC                             :: 0x80;
    DEPRECATED                            :: 0x100;
    SYNTACTICALLY_MARKED_AS_NO_CONTEXT    :: 0x200;
    QUICK                                 :: 0x400;
    QUICK_IN_BLOCK_FORM                   :: 0x800;
    CPP_METHOD                            :: 0x1000;
    NO_CALL                               :: 0x2000;
    MACRO                                 :: 0x4000;
    NO_DEBUG                              :: 0x8000;
    ENTRY_POINT                           :: 0x1_0000;
    HAS_IMPLICIT_RETURN_VALUE             :: 0x2_0000;
    SYMMETRIC                             :: 0x4_0000;
    CPP_RETURN_TYPE_IS_NON_POD            :: 0x8_0000;
    ENTRY_POINT_HOOK                      :: 0x10_0000;
    SYNTACTICALLY_MARKED_AS_COMPILE_TIME  :: 0x20_0000;
    SYNTACTICALLY_MARKED_AS_COMPILER      :: 0x40_0000;
    SYNTACTICALLY_MARKED_AS_INLINE_YES    :: 0x100_0000;
    SYNTACTICALLY_MARKED_AS_INLINE_NO     :: 0x200_0000;
}

// `body_or_null` is filled asynchronously: a header may be typechecked before
// its body is (**C§5.3**).
Code_Procedure_Header :: struct {
    #as using base: Code_Node;
    constants_block:         *Code_Block;
    arguments:               [] *Code_Declaration;
    returns:                 [] *Code_Declaration;
    parameter_usings:        [] *Code_Using;
    name:                    string;
    foreign_function_name:   string;
    library_identifier:      *Code_Ident;
    deprecation_string:      string;
    polymorph_source_header: *Code_Procedure_Header;
    modify_directives:       [] *Code_Directive_Modify;
    body_or_null:            *Code_Procedure_Body;
    procedure_flags:         Procedure_Flags;
    notes:                   [] *Code_Note;
}

Body_Flags :: enum_flags u32 {
    ALREADY_MODIFIED :: 0x1;
}

Code_Procedure_Body :: struct {
    #as using base: Code_Node;
    block:      *Code_Block;
    header:     *Code_Procedure_Header;
    body_flags: Body_Flags;
}

Code_Resolved_Overload :: struct {
    #as using base: Code_Node;
    result:            *Code_Node;
    source_expression: *Code_Node;
}

Struct_Textual_Flags :: enum_flags u32 {
    UNION      :: 0x1;
    NO_PADDING :: 0x2;
}

Code_Struct :: struct {
    #as using base: Code_Node;
    modify_directives: [] *Code_Directive_Modify;
    block:             *Code_Block;
    // The arguments a specialization was not given.
    arguments_block:   *Code_Block;
    constants_block:   *Code_Block;
    notes:             [] *Code_Note;
    textual_flags:     Struct_Textual_Flags;
    alignment:         s32;
    defined_type:      *Type_Info_Struct;
}

Code_Enum :: struct {
    #as using base: Code_Node;
    internal_type_inst:  *Code_Type_Instantiation;
    internal_type:       *Type_Info_Integer;
    external_type:       *Type_Info_Enum;
    block:               *Code_Block;
    notes:               [] *Code_Note;
    marked_as_complete:  bool;
    marked_as_specified: bool;
    is_flags:            bool;
}

Inst_Flags :: enum_flags u32 {
    VARARGS                 :: 0x1;
    RESIZABLE               :: 0x2;
    TYPE_OF_PROCEDURE       :: 0x4;
    TYPE_DIRECTIVE          :: 0x8;
    TYPE_DIRECTIVE_DISTINCT :: 0x10;
    TYPE_DIRECTIVE_ISA      :: 0x20;
    ARRAY_VIEW              :: 0x40;
    INTERFACE               :: 0x80;
}

Code_Type_Instantiation :: struct {
    #as using base: Code_Node;
    result:                  *Type_Info;
    type_valued_expression:  *Code_Node;
    // The `/R` restriction on a polymorph variable.
    must_implement:          *Code_Node;
    pointer_to:              *Code_Type_Instantiation;
    type_directive_target:   *Code_Type_Instantiation;
    array_element_type:      *Code_Type_Instantiation;
    array_dimension:         *Code_Node;
    inst_flags:              Inst_Flags;
}

Code_Type_Definition :: struct {
    #as using base: Code_Node;
    info: *Type_Info;
}

Type_Query_Kind :: enum u32 {
    SIZE_OF        :: 0;
    TYPE_INFO      :: 1;
    INITIALIZER_OF :: 2;
}

Code_Type_Query :: struct {
    #as using base: Code_Node;
    query_kind:    Type_Query_Kind;
    type_to_query: *Code_Type_Instantiation;
}

Expression_Query_Kind :: enum u32 {
    TYPE_OF     :: 0;
    IS_CONSTANT :: 1;
    CODE_OF     :: 2;
}

Code_Expression_Query :: struct {
    #as using base: Code_Node;
    query_kind:          Expression_Query_Kind;
    expression_to_query: *Code_Node;
}

Cast_Flags :: enum_flags u32 {
    IS_IMPLICIT           :: 0x1;
    IS_POINTER_DEREFERENCE :: 0x2;
    IS_AUTO               :: 0x4;
    NO_BOUNDS_CHECK       :: 0x8;
    TRUNCATE              :: 0x10;
    AS                    :: 0x20;
    ISA                   :: 0x40;
    FORCE                 :: 0x80;
    VERY_FORCE            :: 0x100;
    HAS_DEREFERENCE       :: 0x400;
    HAS_FUNCTION_SYNTAX   :: 0x800;
    HAS_POSTFIX_SYNTAX    :: 0x1000;
}

Code_Cast :: struct {
    #as using base: Code_Node;
    target_type: *Code_Type_Instantiation;
    expression:  *Code_Node;
    cast_flags:  Cast_Flags;
}

If_Flags :: enum_flags u16 {
    IS_SWITCH_STATEMENT :: 0x1;
    IS_IFX              :: 0x2;
    IS_STATIC           :: 0x4;
    MARKED_AS_COMPLETE  :: 0x8;
}

Static_If_Flags :: enum_flags u16 {
    EVALUATED_AS_TRUE :: 0x1;
}

Code_If :: struct {
    #as using base: Code_Node;
    condition:               *Code_Node;
    then_block:              *Code_Node;
    else_block:              *Code_Node;
    if_flags:                If_Flags;
    static_if_flags:         Static_If_Flags;
    static_if_accepted_case: *Code_Case;
}

Code_Case :: struct {
    #as using base: Code_Node;
    condition:             *Code_Node;
    then_block:            *Code_Block;
    owning_if:             *Code_If;
    marked_as_fallthrough: bool;
}

Code_While :: struct {
    #as using base: Code_Node;
    condition: *Code_Node;
    block:     *Code_Node;
}

Code_For :: struct {
    #as using base: Code_Node;
    iteration_expression:            *Code_Node;
    iteration_expression_right:      *Code_Node;
    block:                           *Code_Node;
    ident_it:                        *Code_Ident;
    ident_it_index:                  *Code_Ident;
    ident_decl:                      *Code_Declaration;
    index_decl:                      *Code_Declaration;
    // Expressions, not flags: `for <` and `for *` are written as modifiers on
    // the iteration and the compiler keeps what was written.
    want_replacement_for_expansion:  *Code_Node;
    want_pointer_expression:         *Code_Node;
    want_reverse_expression:         *Code_Node;
    macro_expansion_procedure_call:  *Code_Procedure_Call;
    for_flags:                       For_Flags;
}

Loop_Control_Type :: enum u32 {
    BREAK    :: 0;
    CONTINUE :: 1;
    REMOVE   :: 2;
}

Code_Loop_Control :: struct {
    #as using base: Code_Node;
    control_type: Loop_Control_Type;
    target_ident: *Code_Ident;
}

Return_Flags :: enum_flags u32 {
    IS_BACKTICKED                  :: 0x1;
    AUTO_INSERTED_FOR_QUICK_LAMBDA :: 0x2;
}

Code_Return :: struct {
    #as using base: Code_Node;
    arguments_unsorted: [] Code_Argument;
    arguments_sorted:   [] *Code_Node;
    return_flags:       Return_Flags;
}

Code_Defer :: struct {
    #as using base: Code_Node;
    block:         *Code_Node;
    is_backticked: bool;
}

Filter_Type :: enum u32 {
    NONE   :: 0;
    ONLY   :: 1;
    EXCEPT :: 2;
    MAP    :: 3;
}

Code_Using :: struct {
    #as using base: Code_Node;
    expression:        *Code_Node;
    filter_type:       Filter_Type;
    filter_expression: *Code_Node;
    no_parameters:     bool;
}

Push_Context_Flags :: enum_flags u32 {
    IS_BACKTICKED :: 0x1;
    DEFER_POP     :: 0x2;
}

Code_Push_Context :: struct {
    #as using base: Code_Node;
    to_push:             *Code_Node;
    block:               *Code_Block;
    push_context_flags:  Push_Context_Flags;
}

Code_Context :: struct {
    #as using base: Code_Node;
}

Comma_Modifier :: enum u32 {
    NONE    :: 0;
    DECLARE :: 1;
    ASSIGN  :: 2;
}

Code_Comma_Separated_Argument :: struct {
    node:     *Code_Node;
    modifier: Comma_Modifier;
}

Code_Comma_Separated_Arguments :: struct {
    #as using base: Code_Node;
    arguments: [] Code_Comma_Separated_Argument;
}

Code_Compound_Declaration :: struct {
    #as using base: Code_Node;
    comma_separated_assignment: *Code_Comma_Separated_Arguments;
    declaration_properties:     *Code_Declaration;
    alignment_expression:       *Code_Node;
    notes:                      [] *Code_Note;
    operator_type:              Operator_Type;
}

Code_Extract :: struct {
    #as using base: Code_Node;
    from:  *Code_Node;
    index: s64;
}

// What the compiler puts in a `..T` slot to stand for the arguments it
// gathered (**C§5.3**).
Code_Make_Varargs :: struct {
    #as using base: Code_Node;
    element_type:                          *Code_Type_Instantiation;
    expressions:                           [] *Code_Node;
    is_for_non_native_calling_convention:  bool;
}

Code_Note :: struct {
    #as using base: Code_Node;
    text:       string;
    note_flags: u32;
}

// The block itself is opaque here: a metaprogram reads where it was written
// rather than its instructions (**L§15**). The three members are what the
// compiler keeps; nothing outside it reads them, so they are named for their
// shape rather than pretending to a meaning.
Code_Asm :: struct {
    #as using base: Code_Node;
    b1: *void;
    b2: [] *void;
    b3: [] *void;
}

// ---------------------------------------------------------- directives ---

Directive_Run_Flags :: enum_flags u32 {
    ASSERTION                 :: 0x1;
    STALLABLE                 :: 0x2;
    SYNTACTICALLY_IMPLICIT    :: 0x4;
    HAS_IMPLICIT_RETURN_TYPES :: 0x8;
}

Code_Directive_Run :: struct {
    #as using base: Code_Node;
    procedure:        *Code_Procedure_Header;
    flags:            Directive_Run_Flags;
    assertion_string: string;
}

Code_Flags :: enum_flags u32 {
    IMPLICITLY_GENERATED :: 0x1;
    NULL                 :: 0x2;
    TYPED                :: 0x8;
}

Code_Directive_Code :: struct {
    #as using base: Code_Node;
    expression: *Code_Node;
    code_flags: Code_Flags;
}

Code_Directive_Insert :: struct {
    #as using base: Code_Node;
    expression:          *Code_Node;
    scope_redirection:   *Code_Node;
    break_replacement:   *Code_Node;
    continue_replacement: *Code_Node;
    remove_replacement:  *Code_Node;
    expansion:           *Code_Node;
    is_internal:         bool;
}

Import_Flags :: enum_flags u32 {
    UNSHARED :: 0x1;
}

Provided_Import_Type :: enum u32 {
    MODULE :: 0;
    FILE   :: 1;
    DIR    :: 2;
    STRING :: 3;
}

Code_Directive_Import :: struct {
    #as using base: Code_Node;
    name:                     string;
    flags:                    Import_Flags;
    import_type:              Provided_Import_Type;
    module_parameters_call:   *Code_Procedure_Call;
    program_parameters_call:  *Code_Procedure_Call;
}

Code_Directive_Load :: struct {
    #as using base: Code_Node;
    short_name:            string;
    fully_pathed_filename: string;
    loaded_string:         string;
    load_flags:            u32;
}

Library_Flags :: enum_flags u32 {
    IS_SYSTEM_LIBRARY           :: 0x1;
    DYNAMIC_LIBRARY_UNAVAILABLE :: 0x2;
    STATIC_LIBRARY_UNAVAILABLE  :: 0x4;
    LINK_ALWAYS                 :: 0x8;
}

Code_Directive_Library :: struct {
    #as using base: Code_Node;
    name:          string;
    library_flags: Library_Flags;
}

Bake_Type :: enum u32 {
    CONSTANTS_BAKE       :: 0;
    PARAMETER_VALUE_BAKE :: 1;
    DYNAMIC_SPECIALIZE   :: 2;
}

Code_Directive_Bake :: struct {
    #as using base: Code_Node;
    procedure_call: *Code_Procedure_Call;
    bake_type:      Bake_Type;
}

Code_Directive_Modify :: struct {
    #as using base: Code_Node;
    block: *Code_Block;
}

Scope_Type :: enum u32 {
    EXPORT   :: 0;
    FILE     :: 1;
    INTERNAL :: 2;
}

Code_Directive_Scope :: struct {
    #as using base: Code_Node;
    scope_type: Scope_Type;
}

Code_Directive_Module_Parameters :: struct {
    #as using base: Code_Node;
    module_parameters:  *Code_Procedure_Header;
    program_parameters: *Code_Procedure_Header;
    common_code:        *Code_Block;
}

Code_Directive_Location :: struct {
    #as using base: Code_Node;
    expression:         *Code_Node;
    is_caller_location: bool;
}

Code_Directive_Place :: struct {
    #as using base: Code_Node;
    ident: *Code_Ident;
}

Code_Directive_Poke_Name :: struct {
    #as using base: Code_Node;
    module_struct: *Code_Struct;
    name:          string;
}

Code_Directive_Add_Context :: struct {
    #as using base: Code_Node;
    expression: *Code_Node;
}

Code_Directive_Context_Type :: struct {
    #as using base: Code_Node;
}

Code_Directive_Procedure_Name :: struct {
    #as using base: Code_Node;
    argument: *Code_Node;
}

Code_Directive_Exists :: struct {
    #as using base: Code_Node;
    query_expression: *Code_Node;
    sync_expression:  *Code_Node;
}

Code_Directive_Wildcard :: struct {
    #as using base: Code_Node;
    index: s32 = -1;
}

Code_Directive_Bytes :: struct {
    #as using base: Code_Node;
    expression: *Code_Node;
}

Code_Directive_Through :: struct {
    #as using base: Code_Node;
}

Code_Directive_This :: struct {
    #as using base: Code_Node;
}

Code_Directive_Compile_Time :: struct {
    #as using base: Code_Node;
}

Code_Placeholder :: struct {
    #as using base: Code_Node;
}

// --------------------------------------------------------- build options ---

Os_Tag  :: Operating_System_Tag;

Output_Type :: enum u8 {
    NO_OUTPUT       :: 0;
    EXECUTABLE      :: 1;
    DYNAMIC_LIBRARY :: 2;
    STATIC_LIBRARY  :: 3;
    OBJECT_FILE     :: 4;
}

Runtime_Support_Definitions :: enum u8 {
    ENTRY_POINT_AND_INIT :: 0;
    ONLY_INIT            :: 1;
    OMIT                 :: 2;
}

Backend :: enum u8 {
    X64  :: 0;
    LLVM :: 1;
}

Llvm_Options :: struct {
    enable_split_modules      := true;
    target_system_triple:     string;
    target_system_features:   string;
    code_gen_optimization:    s32;
    function_sections         := false;
    disable_inlining          := false;
    enable_bitcode            := false;
}

Import_Path_Entry :: struct {
    path: string;
}

// @Volatile: nothing about this is written down in Rust — orangejuice folds a
// fresh one out of these defaults and finds the members it acts on by name at
// the offsets the checker computed (**C§4**).
Build_Options :: struct {
    output_type                              := Output_Type.EXECUTABLE;
    output_executable_name:                   string;
    output_path:                              string;
    intermediate_path:                        string;
    append_executable_filename_extension      := true;

    // Default to what this compiler is building for; `OS` and `CPU` are
    // injected by the compiler (**L§4.3**).
    os_target                                 := OS;
    cpu_target                                := CPU;

    backend                                   := Backend.LLVM;
    llvm_options:                             Llvm_Options;

    optimization_level:                       s32;
    emit_debug_info                           := true;
    stack_trace                               := true;
    enable_bytecode_inliner                   := true;

    runtime_support_definitions               := Runtime_Support_Definitions.ENTRY_POINT_AND_INIT;
    // Only meaningful where Runtime_Support defines the entry point, since
    // that is what installs the handler.
    backtrace_on_crash:        enum u8 { OFF; ON; } = .ON;

    // Each safety check is off, on, or — for the two that can report without
    // dying — on but non-fatal. `ALWAYS` keeps the array check even where a
    // `#no_abc` asked for it to go.
    array_bounds_check:        enum u8 { OFF; ON; ALWAYS; }      = .ON;
    cast_bounds_check:         enum u8 { OFF; NONFATAL; FATAL; } = .FATAL;
    null_pointer_check:        enum u8 { OFF; ON; }              = .ON;
    arithmetic_overflow_check: enum u8 { OFF; NONFATAL; FATAL; } = .OFF;

    use_custom_link_command                   := false;
    dead_code_elimination:     enum u8 { NONE; ALL; MODULES_ONLY; } = .MODULES_ONLY;

    import_path:                              [] string;
    compile_time_command_line:                [] string;

    text_output_flags:                        u32;
    entry_point_name                          := "main";

    temporary_storage_size:                   s64 = 32768;
    context_size_max:                         s64 = 4096;

    machine_options:                          [MACHINE_OPTIONS_SIZE] u8;
}

Build_Options_During_Compile :: struct {
    do_output                        := true;
    write_added_strings              := true;
    runtime_storageless_type_info    := false;
    use_visual_studio_message_format := false;
    stack_trace_on_error             := true;
}

// Copies the fields a workspace should inherit from the compilation that
// created it (**C§4**). Where the output goes is deliberately *not* among
// them: a fresh workspace writes beside the first file it was given unless it
// says otherwise, which is what lets one metaprogram build a library and then
// a program that links it.
copy_commonly_propagated_fields :: (from: Build_Options, to: *Build_Options) {
    to.os_target                 = from.os_target;
    to.cpu_target                = from.cpu_target;
    to.backend                   = from.backend;
    to.llvm_options              = from.llvm_options;

    to.optimization_level        = from.optimization_level;
    to.emit_debug_info           = from.emit_debug_info;
    to.backtrace_on_crash        = from.backtrace_on_crash;
    to.stack_trace               = from.stack_trace;
    to.array_bounds_check        = from.array_bounds_check;
    to.cast_bounds_check         = from.cast_bounds_check;
    to.null_pointer_check        = from.null_pointer_check;
    to.arithmetic_overflow_check = from.arithmetic_overflow_check;
    to.dead_code_elimination     = from.dead_code_elimination;

    to.import_path               = from.import_path;
    to.temporary_storage_size    = from.temporary_storage_size;
    to.context_size_max          = from.context_size_max;
}

// A plugin the driving metaprogram runs alongside a compilation (**C§3**).
// Declared here rather than in `Metaprogram_Plugins` because a build file that
// imports only `Compiler` still names the type.
Metaprogram_Plugin :: struct {
    // Set by the metaprogram that created the plugin: the workspace whose
    // messages it sees, and the one it would add source to.
    workspace: Workspace;

    before_intercept:  (p: *Metaprogram_Plugin, flags: *Intercept_Flags);
    add_source:        (p: *Metaprogram_Plugin);
    // The workspace is not passed, because it is on the message.
    message:           (p: *Metaprogram_Plugin, message: *Message);
    finish:            (p: *Metaprogram_Plugin);
    shutdown:          (p: *Metaprogram_Plugin);

    // Returns where argument parsing carries on from. A plugin that does not
    // recognise the option leaves the cursor where it found it.
    handle_one_option: (p: *Metaprogram_Plugin, options: [] string, cursor: int) -> (new_cursor: int);
    log_help:          (p: *Metaprogram_Plugin);
}

Optimization_Type :: enum u8 {
    DEBUG                :: 0;
    VERY_DEBUG           :: 1;
    OPTIMIZED            :: 2;
    VERY_OPTIMIZED       :: 3;
    OPTIMIZED_SMALL      :: 4;
    OPTIMIZED_VERY_SMALL :: 5;
}

// Sets the several options a build type implies, so a caller need not know
// which they are. Anything it touches can still be changed afterwards.
//
// Debug info and optimization are kept orthogonal: an optimized build that is
// still debuggable is usually what people want, which is why
// `preserve_debug_info` defaults to keeping it.
set_optimization :: (options: *Build_Options, type: Optimization_Type, preserve_debug_info := true) {
    debug := (type == Optimization_Type.DEBUG) || (type == Optimization_Type.VERY_DEBUG);

    options.optimization_level      = ifx debug then 0 else 2;
    options.enable_bytecode_inliner = !debug;

    // The runtime checks are what a debug build is for, and what an optimized
    // one is trying not to pay for.
    if debug {
        options.array_bounds_check        = .ON;
        options.cast_bounds_check         = .FATAL;
        options.null_pointer_check        = .ON;
        options.arithmetic_overflow_check = .FATAL;
    } else {
        options.array_bounds_check        = .OFF;
        options.cast_bounds_check         = .OFF;
        options.null_pointer_check        = .OFF;
        options.arithmetic_overflow_check = .OFF;
    }

    options.stack_trace     = debug;
    options.emit_debug_info = debug || preserve_debug_info;

    if type == {
        case Optimization_Type.VERY_DEBUG;
            options.llvm_options.disable_inlining = true;
        case Optimization_Type.VERY_OPTIMIZED;
            options.llvm_options.code_gen_optimization = 3;
        case Optimization_Type.OPTIMIZED_SMALL;       #through;
        case Optimization_Type.OPTIMIZED_VERY_SMALL;
            options.llvm_options.code_gen_optimization = 1;
        case;
    }
}

Version_Info :: struct {
    major: s32;
    minor: s32;
    micro: s32;
}

Report :: enum u8 {
    ERROR             :: 0;
    ERROR_CONTINUABLE :: 1;
    WARNING           :: 2;
    INFO              :: 3;
}

Intercept_Flags :: enum_flags u32 {
    NONE     :: 0;
    SKIP_ALL :: 0x1;
}

Workspace_Status_Flags :: enum_flags u32 {
    NONE  :: 0;
    READY :: 0x1;
}

// ------------------------------------------------- the compiler answers ---

// Everything below is answered by the compiler itself: `oj-meta` binds each
// symbol to a Rust procedure built to `oj_ir::abi_of`, and
// `crates/meta/tests/intrinsics.rs` checks that every symbol answered is one
// this module declares (**C§3.3**).

compiler_create_workspace  :: (name := "") -> Workspace #compiler;
compiler_destroy_workspace :: (w: Workspace) -> bool #compiler;

// `get_current_workspace` is Preload's, since a program that never imports
// this module still asks it (**L§17**).

get_name              :: (w: Workspace = -1) -> string #compiler;

get_build_options    :: (w: Workspace = -1) -> Build_Options #compiler;
set_build_options    :: (options: Build_Options, w: Workspace = -1, loc := #caller_location) #compiler;
set_build_options_dc :: (options: Build_Options_During_Compile, w: Workspace = -1, loc := #caller_location) #compiler;

add_build_file   :: (filename: string, w: Workspace = -1, loc := #caller_location) #compiler;
add_build_string :: (data: string, w: Workspace = -1, code := #code,null, loc := #caller_location) #compiler;
add_build_string_scoped_by_message :: (data: string, message: *Message, loc := #caller_location) #compiler;

compiler_begin_intercept :: (w: Workspace, flags := Intercept_Flags.NONE) #compiler;
compiler_end_intercept   :: (w: Workspace) #compiler;
compiler_wait_for_message :: () -> *Message #compiler;

compiler_get_nodes :: (code: Code) -> (root: *Code_Node, expressions: [] *Code_Node) #compiler;
compiler_get_code  :: (node: *Code_Node) -> Code #compiler;

compiler_report :: (message: string, loc := #caller_location, mode := Report.ERROR) #compiler;
compiler_report_errors_for_unresolved_identifiers :: (w: Workspace) #compiler;
compiler_report_errors_for_untyped_declarations_with_these_notes :: (w: Workspace, notes: [] string) #compiler;

compiler_get_version_info :: (version_info_return: *Version_Info) -> string #compiler;
compiler_get_base_path    :: () -> string #compiler;

compiler_modify_procedure :: (w: Workspace, body: *Code_Procedure_Body) #compiler;
compiler_make_procedure_live :: (w: Workspace, header: *Code_Procedure_Header) #compiler;
compiler_get_struct_location :: (info: *Type_Info_Struct) -> Source_Code_Location #compiler;
compiler_set_type_info_flags :: (type: Type, flags: u32) #compiler;
compiler_set_workspace_status :: (w: Workspace, flags: Workspace_Status_Flags) #compiler;
compiler_set_memory_breakpoint :: (address: *void, size: s64) #compiler;

// An import a metaprogram redirects or blocks before the compilation reaches
// it: naming no replacement blocks it outright (**C§3.3**).
remap_import :: (w: Workspace, host_module_name: string, import_name: string, replacement_name: string) #compiler;

// The answer to a `FAILED_IMPORT` message. The reference is holding the import
// open waiting for this; orangejuice has already compiled the workspace, so
// the answer makes it compile again with the import in place (`docs/spec.md`
// §10).
provide_import :: (w: Workspace, message: *Message_Failed_Import, type: Provided_Import_Type, value: string) #compiler;

// A hook for setting a data breakpoint while debugging compile-time execution.
developer_debug :: (x: *void) #compiler;

compiler_add_library_search_directory :: (path: string) #compiler;
compiler_custom_link_command_is_complete :: (w: Workspace, success: bool) #compiler;

add_global_data  :: (data: [] u8, alignment: u32 = 8) -> [] u8 #compiler;
add_data_segment :: (w: Workspace, name: string, characteristics: u32, alignment: u32) -> *void #compiler;

get_type      :: (info: *Type_Info) -> Type #compiler;
get_root_type :: (info: *Type_Info) -> *Type_Info #compiler;

get_toplevel_command_line :: () -> [] string #compiler;


// -------------------------------------------------- the performance report ---
//
// What `compiler_begin_intercept` reports when asked for it. A metaprogram
// gets these to find out where a slow compilation went.

Time_Report :: struct {
    run_directives:      float64;
    bytecode_inlining:   float64;
    bytecode_generating: float64;
}

Run_Directive_Report :: struct {
    // Only true when the interception asked for the report; otherwise
    // everything below is blank.
    reported := false;

    Record :: struct {
        callee:       *Code_Procedure_Header;
        num_stalls:   s64;
        elapsed_time: float64;
    }

    records: [] Record;
}

Polymorph_Report :: struct {
    // How many times the solver ran, and how many of those produced something
    // that was used. A low ratio means overload resolution is doing a lot of
    // work that is thrown away, which is what makes a compilation slow.
    num_solves_invoked: s64;
    num_solves_used:    s64;

    reported := false;

    Record :: struct {
        source: *Code_Procedure_Header;
        num_call_sites: s64;
        num_deduplications_by_constants: s64;
        num_deduplications_by_bytecode:  s64;

        distinct_polymorphs: [] Distinct_Polymorph;
    }

    Distinct_Polymorph :: struct {
        polymorphed: *Code_Procedure_Header;
        num_call_sites: s64;
    }

    records: [] Record;
}

Bytecode_Report :: struct {
    // Bytecode inlining only: what an optimizing back end decides afterwards
    // is not counted, and neither are inlined initializers.
    num_calls_inlined:        s64;
    num_calls_total:          s64;
    num_instructions_inlined: s64;
    num_instructions_total:   s64;
}

Message_Debug_Dump :: struct {
    #as using m: Message;

    dump_text: string;
}

Message_Performance_Report :: struct {
    #as using m: Message;

    time_report:          Time_Report;
    run_directive_report: Run_Directive_Report;
    polymorph_report:     Polymorph_Report;
    bytecode_report:      Bytecode_Report;
}

// ------------------------------------------------------- back-end options ---

X64_Options :: struct {
    use_dlls := false;
    enable_register_allocation := false;
    // The stack frame information in `.eh_frame`, which is loaded at runtime,
    // rather than in `.debug_frame`, which a debugger has to be told to read.
    enable_unix_runtime_frame_information := true;
}

Llvm_Bitcode_Optimization_Setting :: enum u8 {
    UNSET :: 0;
    O0    :: 1;
    O1    :: 2;
    O2    :: 3;
    O3    :: 4;
    OS    :: 5;
    OZ    :: 6;
}

// Left UNSET it follows from the bitcode setting.
Llvm_Machine_Code_Optimization_Setting :: enum u8 {
    UNSET      :: 0;
    NONE       :: 1;
    LESS       :: 2;
    DEFAULT    :: 3;
    AGGRESSIVE :: 4;
}

// ------------------------------------------------------- data and segments ---

// Which of the program's segments a blob goes in. `WRITABLE_NO_RESET` is the
// one whose bytes survive from compile time into the executable unchanged.
Data_Segment_Index :: enum u16 {
    WRITABLE          :: 0x0;
    WRITABLE_NO_RESET :: 0x1;
    READ_ONLY         :: 0x2;
    BSS               :: 0x3;
    USER_SEGMENT      :: 0x10;
}

// Opaque: a segment a metaprogram made is named by its address and nothing
// else.
Data_Segment :: struct {}

Data_Segment_Characteristics :: enum_flags u32 {
    READ    :: 0x1;
    WRITE   :: 0x2;
    EXECUTE :: 0x4;
    ZEROED  :: 0x8;
}

// What a running program's `Runtime_Info` says about the segments it was built
// with. Null at compile time.
Global_Data_Segment_Info :: struct {
    segment_tag: enum u16 {
        BSS      :: 0;
        DATA     :: 1;
        RDATA    :: 2;
        NO_RESET :: 3;

        USER :: 5;
    }

    data: [] u8;
}

Global_Data_Info :: struct {
    version_stamp: u64;
    segment_info: [] Global_Data_Segment_Info;
}

// ------------------------------------------------------------ type queries ---

Type_Info_Flags :: enum u32 {
    NO_TYPE_INFO                 :: 0x1;
    PROCEDURES_ARE_VOID_POINTERS :: 0x2;
    NO_SIZE_COMPLAINT            :: 0x4;
}

Get_Root_Type_Status :: enum u8 {
    UNINITIALIZED      :: 0;
    SUCCESS            :: 1;
    INPUT_IS_CODE_NULL :: 2;
    // The `code` given is malformed.
    CORRUPTED          :: 3;
    // The `code` given is not typechecked, so its identifiers mean nothing
    // yet. A `#code,infer` produces one that is.
    NOT_TYPED          :: 4;
}

Workspace_Status :: enum u8 {
    OK     :: 0;
    FAILED :: 1;
}

// Whether a metaprogram was able to fix a value a register has to hold. Not
// answered by this compiler.
Compiler_Pin :: struct {
    name: string;
    pin:  string;
}

// ------------------------------------------------- what a running program has ---

Runtime_Info :: struct {
    type_table: [] *Type_Info;

    // Null at compile time; only a running program has one.
    global_data_info: *Global_Data_Info;
}

// The whole type table of the program that is running. `#elsewhere` because
// the symbol is the image the compiler laid down beside the program rather
// than anything a library exports.
get_runtime_info :: (w: Workspace = -1) -> Runtime_Info #compiler {
    __runtime_info: Runtime_Info #elsewhere;
    return __runtime_info;
}

get_type_table :: (w: Workspace = -1) -> [] *Type_Info {
    return get_runtime_info(w).type_table;
}

// --------------------------------------------------------- little utilities ---

// Anything at or below this is a single character, printed as its own ASCII
// value; `operator_to_string` handles the rest.
OPERATOR_TYPE_MAX_ASCII_VALUE :: 127;

operator_to_string :: (op: Operator_Type) -> string {
    if op == {
      case .PLUS_ASSIGN;         return "+=";
      case .MINUS_ASSIGN;        return "-=";
      case .TIMES_ASSIGN;        return "*=";
      case .DIV_ASSIGN;          return "/=";
      case .MOD_ASSIGN;          return "%=";
      case .IS_EQUAL;            return "==";
      case .IS_NOT_EQUAL;        return "!=";
      case .LOGICAL_AND;         return "&&";
      case .LOGICAL_OR;          return "||";
      case .LESS_OR_EQUAL;       return "<=";
      case .GREATER_OR_EQUAL;    return ">=";

      case .SHIFT_LEFT;          return "<<";
      case .SHIFT_RIGHT;         return ">>";

      case .ROTATE_LEFT;         return "<<<";
      case .ROTATE_RIGHT;        return ">>>";

      case .SHIFT_LEFT_ASSIGN;   return "<<=";
      case .SHIFT_RIGHT_ASSIGN;  return ">>=";

      case .ROTATE_LEFT_ASSIGN;  return "<<<=";
      case .ROTATE_RIGHT_ASSIGN; return ">>>=";

      case .BITWISE_AND_ASSIGN;  return "&=";
      case .BITWISE_OR_ASSIGN;   return "|=";
      case .BITWISE_XOR_ASSIGN;  return "^=";

      case .LOGICAL_AND_ASSIGN;  return "&&=";
      case .LOGICAL_OR_ASSIGN;   return "||=";

      case .POINTER_DEREFERENCE; return "(.*)";
      case .POSTFIX_DEREFERENCE; return ".*";

      case .ARRAY_SUBSCRIPT;     return "[]";

      case; return "";
    }
}

make_integer_literal :: (v: s64) -> *Code_Literal {
    Basic :: #import "Basic";

    literal := Basic.New(Code_Literal);

    literal.kind        = .LITERAL;
    literal.value_type  = .NUMBER;
    literal.value_flags = .IS_A_NUMBER;
    literal._s64        = v;

    return literal;
}

make_string_literal :: (s: string) -> *Code_Literal {
    Basic :: #import "Basic";

    literal := Basic.New(Code_Literal);
    literal.kind       = .LITERAL;
    literal.value_type = .STRING;

    // Copied and zero-terminated, since the node outlives whatever the caller
    // had.
    c_str := Basic.to_c_string(s);
    literal._string = to_string(c_str);

    return literal;
}

// Whether a struct reaches `base_name` through a chain of `#as using` members
// — an entity hierarchy written the way this language writes one. A struct is
// not a subclass of itself, which is what `first_call` is for.
is_subclass_of :: (info: *Type_Info, base_name: string, first_call := true) -> bool {
    if info.type != .STRUCT  return false;

    info_struct := cast(*Type_Info_Struct) info;

    while true {
        if info_struct.name == base_name  return !first_call;

        if info_struct.members.count == 0  break;

        member := info_struct.members[0];
        if !(member.flags & .AS)  break;

        type := member.type;
        if type.type == .STRUCT {
            info_struct = cast(*Type_Info_Struct) type;
            first_call = false;
            continue;
        } else if type.type == .POINTER {
            pointer := cast(*Type_Info_Pointer) type;
            return is_subclass_of(pointer.pointer_to, base_name, false);
        } else {
            break;
        }
    }

    return false;
}

// The file a node was written in, which is what its enclosing `#load` says.
get_filename :: (node: *Code_Node) -> string {
    if !node.enclosing_load  return "";
    return node.enclosing_load.fully_pathed_filename;
}

make_location :: (c: *Code_Node) -> Source_Code_Location {
    loc: Source_Code_Location = ---;
    loc.fully_pathed_filename = get_filename(c);
    loc.line_number           = c.l0;
    loc.character_number      = c.c0;
    return loc;
}

// `"a/b/c/d.foo.txt"` becomes `"d.foo"` and `"a/b/c/"` — the path keeps its
// trailing slash. A string operation, but it is here because almost every
// startup metaprogram wants it and importing `String` for it would cost more
// than it saves.
get_basename_and_path :: (input_name: string) -> (basename: string, path: string) {
    Basic :: #import "Basic";

    if !input_name  return "", "";

    dot_index   := -1;
    slash_index := -1;
    // From the right, so the *last* slash and the dot after it are the ones
    // that matter.
    for < c, i: input_name {
        if c == #char "." && dot_index == -1  dot_index = i;

        if c == #char "/" || c == #char "\\" {
            slash_index = i;
            break;
        }
    }

    basename    := input_name;
    output_path := "";
    if slash_index >= 0 {
        basename    = Basic.advance(basename, slash_index + 1);
        output_path = input_name;
        output_path.count = slash_index + 1;
    }

    // A dot before the last slash belongs to a directory name, not to this
    // file's extension.
    if dot_index >= 0 && dot_index > slash_index {
        basename.count = dot_index - slash_index - 1;
    }

    return basename, output_path;
}
