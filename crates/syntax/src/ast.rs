use bitflags::bitflags;
use oj_diag::Span;
use oj_lexer::Symbol;

/// A node kind, numbered as `Code_Node.Kind` (**C§5.3**) so that exporting a
/// tree to metaprogram-visible `Code_*` structs is a projection rather than a
/// translation. Kinds at [`NodeKind::INTERNAL_BASE`] and above are orangejuice's
/// own: the reference folds those constructs into literals during parsing, and
/// so do we, but only once a scope knows the file and line they stand for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum NodeKind {
  Block = 1,
  Literal = 2,
  Ident = 3,
  UnaryOperator = 4,
  BinaryOperator = 5,
  ProcedureBody = 6,
  ProcedureCall = 7,
  Context = 8,
  While = 9,
  If = 10,
  LoopControl = 11,
  Case = 12,
  Return = 14,
  For = 15,
  TypeInstantiation = 17,
  Enum = 18,
  ProcedureHeader = 19,
  Struct = 20,
  CommaSeparatedArguments = 21,
  DirectiveBytes = 23,
  Declaration = 25,
  Cast = 26,
  DirectiveImport = 27,
  DirectiveThis = 28,
  DirectiveThrough = 29,
  DirectiveLoad = 30,
  DirectiveRun = 31,
  DirectiveCode = 32,
  DirectivePokeName = 33,
  Asm = 34,
  DirectiveBake = 35,
  DirectiveModify = 36,
  DirectiveLibrary = 37,
  ExpressionQuery = 38,
  PushContext = 39,
  Note = 40,
  DirectivePlace = 41,
  DirectiveScope = 42,
  TypeQuery = 43,
  DirectiveLocation = 44,
  DirectiveModuleParameters = 45,
  DirectiveAddContext = 46,
  DirectiveCompileTime = 47,
  CompoundDeclaration = 48,
  Defer = 49,
  Using = 50,
  Placeholder = 51,
  DirectiveInsert = 52,
  DirectiveProcedureName = 53,
  DirectiveWildcard = 54,
  DirectiveExists = 55,
  DirectiveContextType = 56,

  DirectiveFileInfo = 200,
  DirectiveCallerCode = 201,
}

impl NodeKind {
  pub const INTERNAL_BASE: u16 = 200;

  pub fn is_internal(self) -> bool {
    self as u16 >= Self::INTERNAL_BASE
  }

  pub fn name(self) -> &'static str {
    match self {
      Self::Block => "BLOCK",
      Self::Literal => "LITERAL",
      Self::Ident => "IDENT",
      Self::UnaryOperator => "UNARY_OPERATOR",
      Self::BinaryOperator => "BINARY_OPERATOR",
      Self::ProcedureBody => "PROCEDURE_BODY",
      Self::ProcedureCall => "PROCEDURE_CALL",
      Self::Context => "CONTEXT",
      Self::While => "WHILE",
      Self::If => "IF",
      Self::LoopControl => "LOOP_CONTROL",
      Self::Case => "CASE",
      Self::Return => "RETURN",
      Self::For => "FOR",
      Self::TypeInstantiation => "TYPE_INSTANTIATION",
      Self::Enum => "ENUM",
      Self::ProcedureHeader => "PROCEDURE_HEADER",
      Self::Struct => "STRUCT",
      Self::CommaSeparatedArguments => "COMMA_SEPARATED_ARGUMENTS",
      Self::DirectiveBytes => "DIRECTIVE_BYTES",
      Self::Declaration => "DECLARATION",
      Self::Cast => "CAST",
      Self::DirectiveImport => "DIRECTIVE_IMPORT",
      Self::DirectiveThis => "DIRECTIVE_THIS",
      Self::DirectiveThrough => "DIRECTIVE_THROUGH",
      Self::DirectiveLoad => "DIRECTIVE_LOAD",
      Self::DirectiveRun => "DIRECTIVE_RUN",
      Self::DirectiveCode => "DIRECTIVE_CODE",
      Self::DirectivePokeName => "DIRECTIVE_POKE_NAME",
      Self::Asm => "ASM",
      Self::DirectiveBake => "DIRECTIVE_BAKE",
      Self::DirectiveModify => "DIRECTIVE_MODIFY",
      Self::DirectiveLibrary => "DIRECTIVE_LIBRARY",
      Self::ExpressionQuery => "EXPRESSION_QUERY",
      Self::PushContext => "PUSH_CONTEXT",
      Self::Note => "NOTE",
      Self::DirectivePlace => "DIRECTIVE_PLACE",
      Self::DirectiveScope => "DIRECTIVE_SCOPE",
      Self::TypeQuery => "TYPE_QUERY",
      Self::DirectiveLocation => "DIRECTIVE_LOCATION",
      Self::DirectiveModuleParameters => "DIRECTIVE_MODULE_PARAMETERS",
      Self::DirectiveAddContext => "DIRECTIVE_ADD_CONTEXT",
      Self::DirectiveCompileTime => "DIRECTIVE_COMPILE_TIME",
      Self::CompoundDeclaration => "COMPOUND_DECLARATION",
      Self::Defer => "DEFER",
      Self::Using => "USING",
      Self::Placeholder => "PLACEHOLDER",
      Self::DirectiveInsert => "DIRECTIVE_INSERT",
      Self::DirectiveProcedureName => "DIRECTIVE_PROCEDURE_NAME",
      Self::DirectiveWildcard => "DIRECTIVE_WILDCARD",
      Self::DirectiveExists => "DIRECTIVE_EXISTS",
      Self::DirectiveContextType => "DIRECTIVE_CONTEXT_TYPE",
      Self::DirectiveFileInfo => "DIRECTIVE_FILE_INFO",
      Self::DirectiveCallerCode => "DIRECTIVE_CALLER_CODE",
    }
  }
}

bitflags! {
  /// `Code_Node.node_flags` (**C§5.3**). `IS_PARENTHESIZED` doubles as "this
  /// block was written with braces", the way `Program_Print` reads it.
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct NodeFlags: u32 {
    const EXPRESSION_IS_SPREAD = 0x1;
    const NO_ARRAY_BOUNDS_CHECK = 0x2;
    const ALLOWED_BY_CONTEXT = 0x4;
    const STATEMENT_IS_DEFERRED = 0x8;
    const IS_PARENTHESIZED = 0x10;
    const NO_ARITHMETIC_OVERFLOW_CHECK = 0x80;
    const CREATED_BY_DESUGARING = 0x100;
  }
}

/// `Operator_Type` (**C§5.3**): single-character operators are their own ASCII
/// value, everything else has the reference's number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OperatorType(pub i32);

impl OperatorType {
  pub const PLUS: Self = Self(b'+' as i32);
  pub const MINUS: Self = Self(b'-' as i32);
  pub const TIMES: Self = Self(b'*' as i32);
  pub const DIVIDE: Self = Self(b'/' as i32);
  pub const MODULUS: Self = Self(b'%' as i32);
  pub const BITWISE_AND: Self = Self(b'&' as i32);
  pub const BITWISE_OR: Self = Self(b'|' as i32);
  pub const BITWISE_XOR: Self = Self(b'^' as i32);
  pub const BITWISE_NOT: Self = Self(b'~' as i32);
  pub const NOT: Self = Self(b'!' as i32);
  pub const LESS: Self = Self(b'<' as i32);
  pub const GREATER: Self = Self(b'>' as i32);
  pub const ASSIGN: Self = Self(b'=' as i32);
  pub const DOT: Self = Self(b'.' as i32);

  pub const IS_EQUAL: Self = Self(131);
  pub const IS_NOT_EQUAL: Self = Self(132);
  pub const LOGICAL_AND: Self = Self(133);
  pub const LOGICAL_OR: Self = Self(134);
  pub const LESS_OR_EQUAL: Self = Self(135);
  pub const GREATER_OR_EQUAL: Self = Self(136);
  pub const SHIFT_LEFT: Self = Self(137);
  pub const SHIFT_RIGHT: Self = Self(138);
  pub const ROTATE_LEFT: Self = Self(139);
  pub const ROTATE_RIGHT: Self = Self(140);
  pub const PLUS_ASSIGN: Self = Self(145);
  pub const MINUS_ASSIGN: Self = Self(146);
  pub const TIMES_ASSIGN: Self = Self(147);
  pub const DIV_ASSIGN: Self = Self(148);
  pub const MOD_ASSIGN: Self = Self(149);
  pub const SHIFT_LEFT_ASSIGN: Self = Self(150);
  pub const SHIFT_RIGHT_ASSIGN: Self = Self(151);
  pub const ROTATE_LEFT_ASSIGN: Self = Self(152);
  pub const ROTATE_RIGHT_ASSIGN: Self = Self(153);
  pub const BITWISE_AND_ASSIGN: Self = Self(154);
  pub const BITWISE_OR_ASSIGN: Self = Self(155);
  pub const BITWISE_XOR_ASSIGN: Self = Self(156);
  pub const LOGICAL_AND_ASSIGN: Self = Self(157);
  pub const LOGICAL_OR_ASSIGN: Self = Self(158);
  pub const POINTER_DEREFERENCE: Self = Self(168);
  pub const POSTFIX_DEREFERENCE: Self = Self(169);
  pub const ARRAY_SUBSCRIPT: Self = Self(500);

  pub const MAX_ASCII: i32 = 127;

  /// The source text of the operator, as `Program_Print` writes it.
  pub fn text(self) -> &'static str {
    match self {
      Self::IS_EQUAL => "==",
      Self::IS_NOT_EQUAL => "!=",
      Self::LOGICAL_AND => "&&",
      Self::LOGICAL_OR => "||",
      Self::LESS_OR_EQUAL => "<=",
      Self::GREATER_OR_EQUAL => ">=",
      Self::SHIFT_LEFT => "<<",
      Self::SHIFT_RIGHT => ">>",
      Self::ROTATE_LEFT => "<<<",
      Self::ROTATE_RIGHT => ">>>",
      Self::PLUS_ASSIGN => "+=",
      Self::MINUS_ASSIGN => "-=",
      Self::TIMES_ASSIGN => "*=",
      Self::DIV_ASSIGN => "/=",
      Self::MOD_ASSIGN => "%=",
      Self::SHIFT_LEFT_ASSIGN => "<<=",
      Self::SHIFT_RIGHT_ASSIGN => ">>=",
      Self::ROTATE_LEFT_ASSIGN => "<<<=",
      Self::ROTATE_RIGHT_ASSIGN => ">>>=",
      Self::BITWISE_AND_ASSIGN => "&=",
      Self::BITWISE_OR_ASSIGN => "|=",
      Self::BITWISE_XOR_ASSIGN => "^=",
      Self::LOGICAL_AND_ASSIGN => "&&=",
      Self::LOGICAL_OR_ASSIGN => "||=",
      Self::POINTER_DEREFERENCE => "<<",
      Self::POSTFIX_DEREFERENCE => ".*",
      Self::ARRAY_SUBSCRIPT => "[]",
      Self(value) if (0..=Self::MAX_ASCII).contains(&value) => ascii_operator_text(value as u8),
      _ => "?",
    }
  }

  pub fn is_assignment(self) -> bool {
    self == Self::ASSIGN || (Self::PLUS_ASSIGN.0..=Self::LOGICAL_OR_ASSIGN.0).contains(&self.0)
  }
}

fn ascii_operator_text(byte: u8) -> &'static str {
  const TEXTS: [&str; 15] = [
    "+", "-", "*", "/", "%", "&", "|", "^", "~", "!", "<", ">", "=", ".", "?",
  ];
  const BYTES: [u8; 15] = *b"+-*/%&|^~!<>=.?";
  match BYTES.iter().position(|candidate| *candidate == byte) {
    Some(index) => TEXTS[index],
    None => "?",
  }
}

/// A node's identity inside one [`Ast`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
  pub span: Span,
  pub flags: NodeFlags,
  pub data: NodeData,
}

impl Node {
  pub fn kind(&self) -> NodeKind {
    self.data.kind()
  }
}

/// The nodes of one parsed file, in an arena addressed by [`NodeId`].
#[derive(Clone, Debug, Default)]
pub struct Ast {
  nodes: Vec<Node>,
}

impl Ast {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn push(&mut self, span: Span, data: NodeData) -> NodeId {
    let id = NodeId(self.nodes.len() as u32);
    self.nodes.push(Node {
      span,
      flags: NodeFlags::empty(),
      data,
    });
    id
  }

  pub fn node(&self, id: NodeId) -> &Node {
    &self.nodes[id.0 as usize]
  }

  pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
    &mut self.nodes[id.0 as usize]
  }

  pub fn kind(&self, id: NodeId) -> NodeKind {
    self.node(id).kind()
  }

  pub fn data(&self, id: NodeId) -> &NodeData {
    &self.node(id).data
  }

  pub fn flags(&self, id: NodeId) -> NodeFlags {
    self.node(id).flags
  }

  pub fn add_flags(&mut self, id: NodeId, flags: NodeFlags) {
    self.node_mut(id).flags |= flags;
  }

  pub fn len(&self) -> usize {
    self.nodes.len()
  }

  pub fn is_empty(&self) -> bool {
    self.nodes.is_empty()
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockType {
  Imperative,
  DataDeclarations,
  Arguments,
  Returns,
  StructArguments,
  Constants,
}

bitflags! {
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct BlockFlags: u32 {
    const NO_ARRAY_BOUNDS_CHECK = 0x1;
    const NO_ARITHMETIC_OVERFLOW_CHECK = 0x2;
  }
}

bitflags! {
  /// `Code_Ident.flags` (**C§5.3**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct IdentFlags: u32 {
    const DEFINES_POLYMORPH_VARIABLE = 0x1;
    const IS_RHS_OF_DOT_DEREFERENCE = 0x2;
    const RESOLVES_ONLY_SHALLOWLY = 0x4;
    const GENERATED_BY_OPERATOR_OVERLOAD = 0x8;
    const DO_NOT_RESOLVE_DUE_TO_PARSER = 0x10;
    const CAN_RESOLVE_TO_REG = 0x20;
    const HAS_SCOPE_MODIFIER = 0x40;
  }
}

bitflags! {
  /// `Code_Literal.value_flags` (**C§5.3**). These are *not* the lexer's
  /// `Value_Flags`: the AST keeps the reference's smaller literal set.
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct LiteralFlags: u32 {
    const IS_A_NUMBER = 0x1;
    const HEX = 0x2;
    const FLOAT = 0x4;
    const BINARY = 0x20;
    const MINUS_SIGN = 0x40;
    const HERE_STRING = 0x80;
    const DEFAULTS_TO_FLOAT64 = 0x100;
    const REQUIRES_FLOAT64 = 0x200;
  }
}

bitflags! {
  /// `Code_Declaration.flags` (**C§5.3**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct DeclarationFlags: u32 {
    const IS_CONSTANT = 0x1;
    const IS_MARKED_AS_AS = 0x2;
    const IS_MARKED_AS_DISCARD = 0x4;
    const IS_ITERATOR = 0x10;
    const NO_RESET = 0x40;
    const IS_UNINITIALIZED = 0x80;
    const COMES_FROM_ASM = 0x100;
    const IS_IMPORTED = 0x200;
    const AUTO_VALUE_BAKE = 0x4000;
    const AUTO_VALUE_BAKE_IS_REQUIRED = 0x8000;
    const MUST_BE_RECEIVED = 0x1_0000;
    const PROGRAM_EXPORT = 0x2_0000;
    const ELSEWHERE = 0x4_0000;
    const SCOPE_FILE = 0x8_0000;
    const IS_GLOBAL = 0x10_0000;
    const HAS_SCOPE_MODIFIER = 0x20_0000;
  }
}

bitflags! {
  /// `Code_Procedure_Header.procedure_flags` (**C§5.3**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct ProcedureFlags: u32 {
    const ELSEWHERE = 0x1;
    const COMPILE_TIME_ONLY = 0x2;
    const POLYMORPHIC = 0x4;
    const COMPILER_GENERATED = 0x8;
    const DEBUG_DUMP = 0x10;
    const C_CALL = 0x20;
    const TYPE_ONLY = 0x40;
    const INTRINSIC = 0x80;
    const DEPRECATED = 0x100;
    const SYNTACTICALLY_MARKED_AS_NO_CONTEXT = 0x200;
    const QUICK = 0x400;
    const QUICK_IN_BLOCK_FORM = 0x800;
    const CPP_METHOD = 0x1000;
    const NO_CALL = 0x2000;
    const MACRO = 0x4000;
    const NO_DEBUG = 0x8000;
    const ENTRY_POINT = 0x1_0000;
    const HAS_IMPLICIT_RETURN_VALUE = 0x2_0000;
    const SYMMETRIC = 0x4_0000;
    const CPP_RETURN_TYPE_IS_NON_POD = 0x8_0000;
    const ENTRY_POINT_HOOK = 0x10_0000;
    const SYNTACTICALLY_MARKED_AS_COMPILE_TIME = 0x20_0000;
    const SYNTACTICALLY_MARKED_AS_COMPILER = 0x40_0000;
    const RUNTIME_SUPPORT = 0x80_0000;
    const SYNTACTICALLY_MARKED_AS_INLINE_YES = 0x100_0000;
    const SYNTACTICALLY_MARKED_AS_INLINE_NO = 0x200_0000;
    /// `#no_alias`, which the reference accepts on a header but does not expose
    /// in `Code_Procedure_Header.procedure_flags`; this bit is orangejuice's.
    const NO_ALIAS = 0x400_0000;
  }
}

bitflags! {
  /// `Code_Procedure_Call.flags` (**C§5.3**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct CallFlags: u32 {
    const INLINE_YES = 0x1;
    const INLINE_NO = 0x2;
    const RETURNS_PROCEDURE_POINTER_ONLY = 0x10;
    const NO_DEBUG = 0x100;
    const IS_MODULE_PARAMETERS = 0x200;
  }
}

bitflags! {
  /// `Code_Cast.cast_flags` (**C§5.3**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct CastFlags: u32 {
    const IS_IMPLICIT = 0x1;
    const IS_POINTER_DEREFERENCE = 0x2;
    const IS_AUTO = 0x4;
    const NO_BOUNDS_CHECK = 0x8;
    const TRUNCATE = 0x10;
    const AS = 0x20;
    const ISA = 0x40;
    const FORCE = 0x80;
    const VERY_FORCE = 0x100;
    const HAS_DEREFERENCE = 0x400;
    const HAS_FUNCTION_SYNTAX = 0x800;
    const HAS_POSTFIX_SYNTAX = 0x1000;
  }
}

bitflags! {
  /// `Code_If.if_flags` (**C§5.3**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct IfFlags: u32 {
    const IS_SWITCH_STATEMENT = 0x1;
    const IS_IFX = 0x2;
    const IS_STATIC = 0x4;
    const MARKED_AS_COMPLETE = 0x8;
  }
}

bitflags! {
  /// `For_Flags` (Preload, **L§17**), plus the parser's record of which
  /// modifiers were written as expressions.
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct ForFlags: u32 {
    const POINTER = 0x1;
    const REVERSE = 0x2;
    const TEMPORARY_V2 = 0x4;
  }
}

bitflags! {
  /// `Code_Directive_Run.flags` (**C§5.3**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct RunFlags: u32 {
    const ASSERTION = 0x1;
    const STALLABLE = 0x2;
    const SYNTACTICALLY_IMPLICIT = 0x4;
    const HAS_IMPLICIT_RETURN_TYPES = 0x8;
    /// `#run,host`, which runs on the host machine when cross-compiling; the
    /// reference does not export a flag for it, so this bit is orangejuice's.
    const HOST = 0x100;
  }
}

bitflags! {
  /// `Code_Directive_Code.code_flags` (**C§5.3**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct CodeFlags: u32 {
    const IMPLICITLY_GENERATED = 0x1;
    const NULL = 0x2;
    const TYPED = 0x8;
  }
}

bitflags! {
  /// `Code_Directive_Import.flags` (**C§5.3**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct ImportFlags: u32 {
    const UNSHARED = 0x1;
  }
}

bitflags! {
  /// `Code_Directive_Library.library_flags` (**C§5.3**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct LibraryFlags: u32 {
    const IS_SYSTEM_LIBRARY = 0x1;
    const DYNAMIC_LIBRARY_UNAVAILABLE = 0x2;
    const STATIC_LIBRARY_UNAVAILABLE = 0x4;
    const LINK_ALWAYS = 0x8;
  }
}

bitflags! {
  /// `Type_Info_Struct.textual_flags` (**L§8.7**), as written on the
  /// declaration.
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct StructFlags: u32 {
    const FOREIGN = 0x1;
    const UNION = 0x2;
    const NO_PADDING = 0x4;
    const TYPE_INFO_NONE = 0x8;
    const TYPE_INFO_NO_SIZE_COMPLAINT = 0x10;
    const TYPE_INFO_PROCEDURES_ARE_VOID_POINTERS = 0x20;
  }
}

bitflags! {
  /// `Code_Type_Instantiation.inst_flags` (**C§5.3**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct InstFlags: u32 {
    const VARARGS = 0x1;
    const RESIZABLE = 0x2;
    const TYPE_OF_PROCEDURE = 0x4;
    const TYPE_DIRECTIVE = 0x8;
    const TYPE_DIRECTIVE_DISTINCT = 0x10;
    const TYPE_DIRECTIVE_ISA = 0x20;
    const ARRAY_VIEW = 0x40;
    const INTERFACE = 0x80;
  }
}

bitflags! {
  /// `Code_Binary_Operator.flags` (**C§5.3**): the `,logical` / `,small` shift
  /// modifiers of **L§5.2**.
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct BinaryFlags: u32 {
    const SHIFT_MARKED_AS_LOGICAL = 0x1;
    const SHIFT_MARKED_AS_SMALL = 0x2;
  }
}

bitflags! {
  /// `Code_Return.return_flags` (**C§5.3**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct ReturnFlags: u32 {
    const IS_BACKTICKED = 0x1;
    const AUTO_INSERTED_FOR_QUICK_LAMBDA = 0x2;
  }
}

bitflags! {
  /// `Code_Push_Context.push_context_flags` (**C§5.3**).
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
  pub struct PushContextFlags: u32 {
    const IS_BACKTICKED = 0x1;
    const DEFER_POP = 0x2;
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopControlType {
  Break,
  Continue,
  Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterType {
  None,
  Only,
  Except,
  Map,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeType {
  Export,
  File,
  Internal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BakeType {
  Constants,
  ParameterValue,
  DynamicSpecialize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeQueryKind {
  SizeOf,
  TypeInfo,
  InitializerOf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpressionQueryKind {
  TypeOf,
  IsConstant,
  CodeOf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportType {
  ShortName,
  PathToFile,
  PathToDirectory,
  FullText,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommaModifier {
  None,
  Declare,
  Assign,
}

/// Which of the three source-position directives a [`NodeKind::DirectiveFileInfo`]
/// stands for (**L§5.14**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileInfoKind {
  File,
  Filepath,
  Line,
}

impl FileInfoKind {
  pub fn text(self) -> &'static str {
    match self {
      Self::File => "#file",
      Self::Filepath => "#filepath",
      Self::Line => "#line",
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Argument {
  pub name: Option<NodeId>,
  pub expression: NodeId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommaArgument {
  pub node: NodeId,
  pub modifier: CommaModifier,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
  pub block_type: BlockType,
  pub block_flags: BlockFlags,
  pub statements: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LiteralValue {
  Integer(u64),
  Float(f64),
  Text(Box<[u8]>),
  Bool(bool),
  Null,
  Array(ArrayLiteral),
  Struct(StructLiteral),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArrayLiteral {
  pub element_type: Option<NodeId>,
  pub members: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructLiteral {
  pub type_expression: Option<NodeId>,
  pub arguments: Vec<Argument>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Literal {
  pub value: LiteralValue,
  pub flags: LiteralFlags,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ident {
  pub name: Symbol,
  pub flags: IdentFlags,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeInstantiation {
  pub type_valued_expression: Option<NodeId>,
  pub must_implement: Option<NodeId>,
  pub pointer_to: Option<NodeId>,
  pub type_directive_target: Option<NodeId>,
  pub array_element_type: Option<NodeId>,
  pub array_dimension: Option<NodeId>,
  pub inst_flags: InstFlags,
}

impl TypeInstantiation {
  pub fn empty() -> Self {
    Self {
      type_valued_expression: None,
      must_implement: None,
      pointer_to: None,
      type_directive_target: None,
      array_element_type: None,
      array_dimension: None,
      inst_flags: InstFlags::empty(),
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
  pub name: Option<NodeId>,
  pub type_inst: Option<NodeId>,
  pub expression: Option<NodeId>,
  pub flags: DeclarationFlags,
  pub alignment_expression: Option<NodeId>,
  pub notes: Vec<NodeId>,
  pub program_export_name: Option<Box<[u8]>>,
  /// `name: T #elsewhere lib "symbol";` (**L§4.8**): extern data. The reference
  /// keeps this on the declaration's symbol record rather than in
  /// `Code_Declaration`, so these two fields are orangejuice's.
  pub elsewhere_library: Option<NodeId>,
  pub elsewhere_symbol: Option<Box<[u8]>>,
}

impl Declaration {
  pub fn empty() -> Self {
    Self {
      name: None,
      type_inst: None,
      expression: None,
      flags: DeclarationFlags::empty(),
      alignment_expression: None,
      notes: Vec::new(),
      program_export_name: None,
      elsewhere_library: None,
      elsewhere_symbol: None,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompoundDeclaration {
  pub comma_separated_assignment: NodeId,
  pub declaration_properties: NodeId,
  pub operator_type: Option<OperatorType>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcedureHeader {
  pub name: Option<Symbol>,
  pub arguments: Vec<NodeId>,
  pub returns: Vec<NodeId>,
  pub procedure_flags: ProcedureFlags,
  pub modify_directives: Vec<NodeId>,
  pub foreign_function_name: Option<Box<[u8]>>,
  pub library_identifier: Option<NodeId>,
  pub intrinsic_name: Option<Box<[u8]>>,
  pub deprecation_string: Option<Box<[u8]>>,
  pub body_or_null: Option<NodeId>,
  pub notes: Vec<NodeId>,
  pub parenthesized_returns: bool,
}

impl ProcedureHeader {
  pub fn empty() -> Self {
    Self {
      name: None,
      arguments: Vec::new(),
      returns: Vec::new(),
      procedure_flags: ProcedureFlags::empty(),
      modify_directives: Vec::new(),
      foreign_function_name: None,
      library_identifier: None,
      intrinsic_name: None,
      deprecation_string: None,
      body_or_null: None,
      notes: Vec::new(),
      parenthesized_returns: false,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructNode {
  pub arguments: Vec<NodeId>,
  pub has_argument_list: bool,
  pub block: Option<NodeId>,
  pub modify_directives: Vec<NodeId>,
  pub notes: Vec<NodeId>,
  pub textual_flags: StructFlags,
  pub alignment_expression: Option<NodeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumNode {
  pub internal_type_inst: Option<NodeId>,
  pub block: Option<NodeId>,
  pub notes: Vec<NodeId>,
  pub is_flags: bool,
  pub marked_as_complete: bool,
  pub marked_as_specified: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ForNode {
  pub iteration_expression: NodeId,
  pub iteration_expression_right: Option<NodeId>,
  pub block: NodeId,
  pub ident_it: Option<NodeId>,
  pub ident_it_index: Option<NodeId>,
  pub for_flags: ForFlags,
  pub want_replacement_for_expansion: Option<NodeId>,
  pub want_pointer_expression: Option<NodeId>,
  pub want_reverse_expression: Option<NodeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfNode {
  pub condition: NodeId,
  pub then_block: Option<NodeId>,
  pub else_block: Option<NodeId>,
  pub if_flags: IfFlags,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CaseNode {
  pub condition: Option<NodeId>,
  pub then_block: NodeId,
  pub marked_as_fallthrough: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcedureCall {
  pub procedure_expression: NodeId,
  pub arguments: Vec<Argument>,
  pub context_modification: Option<Vec<NodeId>>,
  pub flags: CallFlags,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cast {
  pub target_type: Option<NodeId>,
  pub expression: NodeId,
  pub cast_flags: CastFlags,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirectiveImport {
  pub name: Box<[u8]>,
  pub flags: ImportFlags,
  pub import_type: ImportType,
  pub module_parameters: Option<Vec<Argument>>,
  pub program_parameters: Option<Vec<Argument>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirectiveRun {
  pub procedure: NodeId,
  pub flags: RunFlags,
  pub assertion_string: Option<NodeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirectiveInsert {
  pub expression: NodeId,
  pub scope_redirection: Option<NodeId>,
  pub has_scope_redirection: bool,
  pub break_replacement: Option<NodeId>,
  pub continue_replacement: Option<NodeId>,
  pub remove_replacement: Option<NodeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirectiveModuleParameters {
  pub module_parameters: NodeId,
  pub program_parameters: Option<NodeId>,
  pub common_code: Option<NodeId>,
}

/// One `#asm` block (**L§15**). Everything an operand can name — a high-level
/// variable, a constant, the type behind a `?T` size — is an ordinary
/// expression node, so the scope tree and the checker see those names the way
/// they see any other.
#[derive(Clone, Debug, PartialEq)]
pub struct AsmNode {
  /// The `x86_Feature_Flag` names the block was tagged with.
  pub features: Vec<Symbol>,
  pub instructions: Vec<AsmInstruction>,
}

/// One statement of an `#asm` block. A statement with no mnemonic is a
/// declaration or a pinning on its own (`t: gpr === a;`, `x === a;`).
#[derive(Clone, Debug, PartialEq)]
pub struct AsmInstruction {
  pub span: Span,
  pub mnemonic: Option<Symbol>,
  pub size: AsmSize,
  pub operands: Vec<AsmOperand>,
}

/// The operand size a mnemonic was tagged with (**L§15**).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AsmSize {
  /// No tag: the block's feature set and the operands decide.
  Inferred,
  /// `.8` … `.512`, and the letter spellings `.b .w .d .q .x .y .z`.
  Bits(u32),
  /// `?T`, where a type means its size in bits and an integer means itself.
  Of(NodeId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AsmOperand {
  pub span: Span,
  pub kind: AsmOperandKind,
  /// `&mask` merges into the destination, `&* mask` zeroes what the mask
  /// leaves out (**L§15**).
  pub mask: Option<AsmMask>,
  /// The `!` suffix: broadcast on a memory operand, suppress-all-exceptions
  /// with an optional rounding mode on a register one.
  pub flag: Option<AsmFlag>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AsmOperandKind {
  /// `name:`, `name: gpr`, `name: gpr === a` — a register the block declares.
  /// The declaration is visible in the scope the block stands in, not in a
  /// scope of the block's own.
  Declaration(AsmDeclaration),
  /// `x === a` — pins a name the block already knows to a register.
  Pin { name: NodeId, register: AsmRegister },
  /// A register the block declared earlier, a high-level variable, or a
  /// constant.
  Expression(NodeId),
  /// `[base + index*scale + displacement]`.
  Memory(Box<AsmMemory>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AsmDeclaration {
  /// The `Ident` node the name was written as, so that the declaration has a
  /// node of its own the way every other declaration does.
  pub name: NodeId,
  pub class: Option<AsmClass>,
  pub register: Option<AsmRegister>,
}

/// A register pool (**L§15**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsmClass {
  Gpr,
  Str,
  Vec,
  Omr,
}

/// Where a `===` pinned an operand: `a b c d si di sp bp` name one of the
/// first eight general-purpose registers, a number names any of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsmRegister {
  Named(Symbol),
  Numbered(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AsmMask {
  pub register: NodeId,
  /// `&*` rather than `&`.
  pub zeroing: bool,
}

/// The `!` suffix of an operand (**L§15**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsmFlag {
  /// `!` on a memory operand broadcasts it, on a register one suppresses all
  /// exceptions.
  Plain,
  /// `!n`, `!d`, `!u`, `!z`.
  Rounding(RoundingMode),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundingMode {
  Nearest,
  Down,
  Up,
  Zero,
}

/// A memory operand, whose components come in the rigid order the reference
/// requires (**L§15**).
#[derive(Clone, Debug, PartialEq)]
pub struct AsmMemory {
  /// `[*p + 8]`: the base names a value by reference rather than by value.
  pub by_reference: bool,
  pub base: NodeId,
  pub index: Option<NodeId>,
  pub scale: Option<NodeId>,
  pub displacement: Option<NodeId>,
  /// Whether the displacement was written after a `-`.
  pub displacement_is_negative: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Using {
  pub expression: NodeId,
  pub filter_type: FilterType,
  pub filter_expression: Option<NodeId>,
  pub no_parameters: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirectiveExists {
  pub query_expression: NodeId,
  pub sync_expression: Option<NodeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirectiveLocation {
  pub expression: Option<NodeId>,
  pub is_caller_location: bool,
  pub has_parentheses: bool,
}

/// The payload of a node. The variant determines [`NodeKind`], so the numbering
/// of **C§5.3** has exactly one source of truth.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeData {
  Block(Block),
  Literal(Literal),
  Ident(Ident),
  UnaryOperator {
    operator: OperatorType,
    operand: NodeId,
  },
  BinaryOperator {
    operator: OperatorType,
    flags: BinaryFlags,
    left: NodeId,
    right: NodeId,
  },
  ProcedureBody {
    header: NodeId,
    block: NodeId,
  },
  ProcedureCall(Box<ProcedureCall>),
  Context,
  While {
    condition: NodeId,
    block: NodeId,
  },
  If(Box<IfNode>),
  LoopControl {
    control_type: LoopControlType,
    target_ident: Option<NodeId>,
  },
  Case(Box<CaseNode>),
  Return {
    arguments: Vec<Argument>,
    flags: ReturnFlags,
  },
  For(Box<ForNode>),
  TypeInstantiation(Box<TypeInstantiation>),
  Enum(Box<EnumNode>),
  ProcedureHeader(Box<ProcedureHeader>),
  Struct(Box<StructNode>),
  CommaSeparatedArguments {
    arguments: Vec<CommaArgument>,
  },
  DirectiveBytes {
    expression: NodeId,
  },
  Declaration(Box<Declaration>),
  Cast(Box<Cast>),
  DirectiveImport(Box<DirectiveImport>),
  DirectiveThis,
  DirectiveThrough,
  DirectiveLoad {
    name: Box<[u8]>,
  },
  DirectiveRun(Box<DirectiveRun>),
  DirectiveCode {
    expression: Option<NodeId>,
    flags: CodeFlags,
  },
  DirectivePokeName {
    module: NodeId,
    name: NodeId,
  },
  Asm(Box<AsmNode>),
  DirectiveBake {
    procedure_call: NodeId,
    bake_type: BakeType,
  },
  DirectiveModify {
    block: NodeId,
  },
  DirectiveLibrary {
    name: Box<[u8]>,
    library_flags: LibraryFlags,
  },
  ExpressionQuery {
    query_kind: ExpressionQueryKind,
    expression_to_query: NodeId,
  },
  PushContext {
    to_push: Option<NodeId>,
    block: Option<NodeId>,
    flags: PushContextFlags,
  },
  Note {
    text: Symbol,
  },
  DirectivePlace {
    ident: NodeId,
  },
  DirectiveScope {
    scope_type: ScopeType,
  },
  TypeQuery {
    query_kind: TypeQueryKind,
    type_to_query: NodeId,
  },
  DirectiveLocation(Box<DirectiveLocation>),
  DirectiveModuleParameters(Box<DirectiveModuleParameters>),
  DirectiveAddContext {
    expression: NodeId,
  },
  DirectiveCompileTime,
  CompoundDeclaration(Box<CompoundDeclaration>),
  Defer {
    block: NodeId,
    is_backticked: bool,
  },
  Using(Box<Using>),
  Placeholder,
  DirectiveInsert(Box<DirectiveInsert>),
  DirectiveProcedureName {
    argument: Option<NodeId>,
  },
  DirectiveWildcard {
    index: i32,
  },
  DirectiveExists(Box<DirectiveExists>),
  DirectiveContextType,
  DirectiveFileInfo {
    which: FileInfoKind,
  },
  DirectiveCallerCode,
}

impl NodeData {
  pub fn kind(&self) -> NodeKind {
    match self {
      Self::Block(_) => NodeKind::Block,
      Self::Literal(_) => NodeKind::Literal,
      Self::Ident(_) => NodeKind::Ident,
      Self::UnaryOperator { .. } => NodeKind::UnaryOperator,
      Self::BinaryOperator { .. } => NodeKind::BinaryOperator,
      Self::ProcedureBody { .. } => NodeKind::ProcedureBody,
      Self::ProcedureCall(_) => NodeKind::ProcedureCall,
      Self::Context => NodeKind::Context,
      Self::While { .. } => NodeKind::While,
      Self::If(_) => NodeKind::If,
      Self::LoopControl { .. } => NodeKind::LoopControl,
      Self::Case(_) => NodeKind::Case,
      Self::Return { .. } => NodeKind::Return,
      Self::For(_) => NodeKind::For,
      Self::TypeInstantiation(_) => NodeKind::TypeInstantiation,
      Self::Enum(_) => NodeKind::Enum,
      Self::ProcedureHeader(_) => NodeKind::ProcedureHeader,
      Self::Struct(_) => NodeKind::Struct,
      Self::CommaSeparatedArguments { .. } => NodeKind::CommaSeparatedArguments,
      Self::DirectiveBytes { .. } => NodeKind::DirectiveBytes,
      Self::Declaration(_) => NodeKind::Declaration,
      Self::Cast(_) => NodeKind::Cast,
      Self::DirectiveImport(_) => NodeKind::DirectiveImport,
      Self::DirectiveThis => NodeKind::DirectiveThis,
      Self::DirectiveThrough => NodeKind::DirectiveThrough,
      Self::DirectiveLoad { .. } => NodeKind::DirectiveLoad,
      Self::DirectiveRun(_) => NodeKind::DirectiveRun,
      Self::DirectiveCode { .. } => NodeKind::DirectiveCode,
      Self::DirectivePokeName { .. } => NodeKind::DirectivePokeName,
      Self::Asm(_) => NodeKind::Asm,
      Self::DirectiveBake { .. } => NodeKind::DirectiveBake,
      Self::DirectiveModify { .. } => NodeKind::DirectiveModify,
      Self::DirectiveLibrary { .. } => NodeKind::DirectiveLibrary,
      Self::ExpressionQuery { .. } => NodeKind::ExpressionQuery,
      Self::PushContext { .. } => NodeKind::PushContext,
      Self::Note { .. } => NodeKind::Note,
      Self::DirectivePlace { .. } => NodeKind::DirectivePlace,
      Self::DirectiveScope { .. } => NodeKind::DirectiveScope,
      Self::TypeQuery { .. } => NodeKind::TypeQuery,
      Self::DirectiveLocation(_) => NodeKind::DirectiveLocation,
      Self::DirectiveModuleParameters(_) => NodeKind::DirectiveModuleParameters,
      Self::DirectiveAddContext { .. } => NodeKind::DirectiveAddContext,
      Self::DirectiveCompileTime => NodeKind::DirectiveCompileTime,
      Self::CompoundDeclaration(_) => NodeKind::CompoundDeclaration,
      Self::Defer { .. } => NodeKind::Defer,
      Self::Using(_) => NodeKind::Using,
      Self::Placeholder => NodeKind::Placeholder,
      Self::DirectiveInsert(_) => NodeKind::DirectiveInsert,
      Self::DirectiveProcedureName { .. } => NodeKind::DirectiveProcedureName,
      Self::DirectiveWildcard { .. } => NodeKind::DirectiveWildcard,
      Self::DirectiveExists(_) => NodeKind::DirectiveExists,
      Self::DirectiveContextType => NodeKind::DirectiveContextType,
      Self::DirectiveFileInfo { .. } => NodeKind::DirectiveFileInfo,
      Self::DirectiveCallerCode => NodeKind::DirectiveCallerCode,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn kinds_carry_the_reference_numbers() {
    assert_eq!(NodeKind::Block as u16, 1);
    assert_eq!(NodeKind::Return as u16, 14);
    assert_eq!(NodeKind::Declaration as u16, 25);
    assert_eq!(NodeKind::DirectiveContextType as u16, 56);
    assert!(NodeKind::DirectiveFileInfo.is_internal());
    assert!(!NodeKind::Declaration.is_internal());
  }

  #[test]
  fn operators_carry_the_reference_numbers_and_text() {
    assert_eq!(OperatorType::PLUS.0, 43);
    assert_eq!(OperatorType::IS_EQUAL.0, 131);
    assert_eq!(OperatorType::ARRAY_SUBSCRIPT.0, 500);
    assert_eq!(OperatorType::SHIFT_LEFT.text(), "<<");
    assert_eq!(OperatorType::PLUS.text(), "+");
    assert_eq!(OperatorType::DOT.text(), ".");
    assert!(OperatorType::PLUS_ASSIGN.is_assignment());
    assert!(OperatorType::ASSIGN.is_assignment());
    assert!(!OperatorType::PLUS.is_assignment());
  }

  #[test]
  fn nodes_are_addressed_in_insertion_order() {
    let mut ast = Ast::new();
    let first = ast.push(Span::at(0), NodeData::Context);
    let second = ast.push(Span::at(1), NodeData::DirectiveThis);

    assert_eq!(first, NodeId(0));
    assert_eq!(second, NodeId(1));
    assert_eq!(ast.kind(first), NodeKind::Context);
    assert_eq!(ast.kind(second), NodeKind::DirectiveThis);
    assert_eq!(ast.len(), 2);
  }
}
