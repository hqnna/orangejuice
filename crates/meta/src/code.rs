//! The AST as a metaprogram sees it (**C§5.3**).
//!
//! `oj-syntax`'s node kinds already carry the reference's numbering, so
//! handing a metaprogram a tree is a projection rather than a translation:
//! every struct here mirrors the one `Compiler.jai` declares, member for
//! member, and `crates/meta/tests/layout.rs` measures the distribution's
//! against these.
//!
//! Nothing here walks an AST — `oj-meta` is below the front end. What it owns
//! is the storage: [`Nodes`] hands out stable addresses that live as long as
//! the compilation, since a metaprogram keeps every pointer it was given.

use std::collections::HashMap;
use std::ffi::c_void;

use crate::abi::{Slice, Str};
use crate::message::{Message, MessageFile};

/// `Code_Node.Location` (**C§5.3**).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Location {
  pub enclosing_load: *const MessageFile,
  pub l0: i32,
  pub c0: i32,
  pub l1: i32,
  pub c1: i32,
}

/// `Code_Node` (**C§5.3**), the head every node struct begins with.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeNode {
  pub kind: u8,
  pub node_flags: u32,
  pub type_info: *const c_void,
  pub location: Location,
  pub serial: i64,
}

/// `Code_Scope_Entry`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeScopeEntry {
  pub base: CodeNode,
  pub name: Str,
  pub import_target: *const CodeDeclaration,
}

/// `Code_Declaration`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDeclaration {
  pub entry: CodeScopeEntry,
  pub type_inst: *const CodeTypeInstantiation,
  pub expression: *const CodeNode,
  pub flags: u32,
  pub alignment_expression: *const CodeNode,
  pub notes: Slice,
  pub program_export_name: Str,
}

/// `Code_Block`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeBlock {
  pub base: CodeNode,
  pub parent: *const CodeBlock,
  pub block_type: i32,
  pub block_flags: u32,
  pub belongs_to_struct: *const CodeStruct,
  pub members: Slice,
  pub statements: Slice,
  pub owning_statement: *const CodeNode,
}

/// `Code_Ident`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeIdent {
  pub base: CodeNode,
  pub name: Str,
  pub resolved_declaration: *const CodeDeclaration,
  pub flags: u32,
}

/// `Code_Literal.values`, the union the `value_type` selects.
#[repr(C)]
#[derive(Clone, Copy)]
pub union CodeLiteralValues {
  pub text: Str,
  pub float64: f64,
  pub signed: i64,
  pub unsigned: u64,
  pub struct_literal_info: *const CodeStructLiteralInfo,
  pub array_literal_info: *const CodeArrayLiteralInfo,
  pub pointer_literal_info: *const CodePointerLiteralInfo,
  pub type_info_literal_defn: *const CodeTypeDefinition,
}

/// `Code_Literal`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CodeLiteral {
  pub base: CodeNode,
  pub value_type: i16,
  pub values: CodeLiteralValues,
  pub value_flags: u32,
}

/// `Code_Literal.value_type` (**C§5.3**).
pub mod literal_type {
  pub const UNINITIALIZED: i16 = 0;
  pub const NUMBER: i16 = 1;
  pub const STRING: i16 = 2;
  pub const BOOLEAN: i16 = 3;
  pub const ARRAY: i16 = 6;
  pub const STRUCT: i16 = 7;
  pub const POINTER: i16 = 8;
  pub const TYPE_INFO: i16 = 9;
}

/// `Code_Struct_Literal_Info`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeStructLiteralInfo {
  pub type_expression: *const CodeTypeInstantiation,
  pub arguments: Slice,
}

/// `Code_Array_Literal_Info`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeArrayLiteralInfo {
  pub element_type: *const CodeTypeInstantiation,
  pub alignment: *const CodeNode,
  pub array_members: Slice,
  pub array_literal_flags: u8,
}

/// `Code_Pointer_Literal_Info`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodePointerLiteralInfo {
  pub global_symbol: *const c_void,
  pub data_pointer: *const u8,
  pub pointer_literal_type: u8,
  pub offset_from_symbol: i64,
}

/// `Code_Type_Instantiation`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeTypeInstantiation {
  pub base: CodeNode,
  pub result: *const c_void,
  pub type_valued_expression: *const CodeNode,
  pub must_implement: *const CodeNode,
  pub pointer_to: *const CodeTypeInstantiation,
  pub type_directive_target: *const CodeTypeInstantiation,
  pub array_element_type: *const CodeTypeInstantiation,
  pub array_dimension: *const CodeNode,
  pub inst_flags: u32,
}

/// `Code_Type_Definition`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeTypeDefinition {
  pub base: CodeNode,
  pub info: *const c_void,
}

/// `Code_Enum`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeEnum {
  pub base: CodeNode,
  pub internal_type_inst: *const CodeTypeInstantiation,
  pub internal_type: *const c_void,
  pub external_type: *const c_void,
  pub block: *const CodeBlock,
  pub notes: Slice,
  pub marked_as_complete: bool,
  pub marked_as_specified: bool,
  pub is_flags: bool,
}

/// `Code_Argument`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeArgument {
  pub expression: *const CodeNode,
  pub name: *const CodeIdent,
}

/// `Code_Procedure_Call.Context_Modification`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ContextModification {
  pub modification_expressions: Slice,
}

/// `Code_Procedure_Call`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeProcedureCall {
  pub base: CodeNode,
  pub procedure_expression: *const CodeNode,
  pub resolved_procedure_expression: *const CodeNode,
  pub overloads: *const Slice,
  pub arguments_unsorted: Slice,
  pub arguments_sorted: Slice,
  pub num_return_values_received: i64,
  pub macro_expansion_block: *const CodeBlock,
  pub context_modification: *const ContextModification,
  pub flags: u32,
}

/// `Code_Procedure_Header`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeProcedureHeader {
  pub base: CodeNode,
  pub constants_block: *const CodeBlock,
  pub arguments: Slice,
  pub returns: Slice,
  pub parameter_usings: Slice,
  pub name: Str,
  pub foreign_function_name: Str,
  pub library_identifier: *const CodeIdent,
  pub deprecation_string: Str,
  pub polymorph_source_header: *const CodeProcedureHeader,
  pub modify_directives: Slice,
  pub body_or_null: *const CodeProcedureBody,
  pub procedure_flags: u32,
  pub notes: Slice,
}

/// `Code_Procedure_Body`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeProcedureBody {
  pub base: CodeNode,
  pub block: *const CodeBlock,
  pub header: *const CodeProcedureHeader,
  pub body_flags: u32,
}

/// `Code_Resolved_Overload`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeResolvedOverload {
  pub base: CodeNode,
  pub result: *const CodeNode,
  pub source_expression: *const CodeNode,
}

/// `Code_Struct`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeStruct {
  pub base: CodeNode,
  pub modify_directives: Slice,
  pub block: *const CodeBlock,
  pub arguments_block: *const CodeBlock,
  pub constants_block: *const CodeBlock,
  pub notes: Slice,
  pub textual_flags: u32,
  pub alignment: i32,
  pub defined_type: *const c_void,
}

/// `Code_Cast`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeCast {
  pub base: CodeNode,
  pub target_type: *const CodeTypeInstantiation,
  pub expression: *const CodeNode,
  pub cast_flags: u32,
}

/// `Code_Type_Query`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeTypeQuery {
  pub base: CodeNode,
  pub query_kind: u32,
  pub type_to_query: *const CodeTypeInstantiation,
}

/// `Code_Expression_Query`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeExpressionQuery {
  pub base: CodeNode,
  pub query_kind: u32,
  pub expression_to_query: *const CodeNode,
}

/// `Code_If`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeIf {
  pub base: CodeNode,
  pub condition: *const CodeNode,
  pub then_block: *const CodeBlock,
  pub else_block: *const CodeBlock,
  pub if_flags: u16,
  pub static_if_flags: u16,
  pub static_if_accepted_case: *const CodeCase,
}

/// `Code_Case`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeCase {
  pub base: CodeNode,
  pub condition: *const CodeNode,
  pub then_block: *const CodeBlock,
  pub owning_if: *const CodeIf,
  pub marked_as_fallthrough: bool,
}

/// `Code_While`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeWhile {
  pub base: CodeNode,
  pub condition: *const CodeNode,
  pub block: *const CodeBlock,
}

/// `Code_For`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeFor {
  pub base: CodeNode,
  pub iteration_expression: *const CodeNode,
  pub iteration_expression_right: *const CodeNode,
  pub block: *const CodeBlock,
  pub ident_it: *const CodeIdent,
  pub ident_it_index: *const CodeIdent,
  pub ident_decl: *const CodeDeclaration,
  pub index_decl: *const CodeDeclaration,
  pub want_replacement_for_expansion: *const CodeNode,
  pub want_pointer_expression: *const CodeNode,
  pub want_reverse_expression: *const CodeNode,
  pub macro_expansion_procedure_call: *const CodeProcedureCall,
  pub for_flags: u8,
}

/// `Code_Loop_Control`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeLoopControl {
  pub base: CodeNode,
  pub control_type: u8,
  pub target_ident: *const CodeIdent,
}

/// `Code_Return`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeReturn {
  pub base: CodeNode,
  pub arguments_unsorted: Slice,
  pub arguments_sorted: Slice,
  pub return_flags: u32,
}

/// `Code_Defer`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDefer {
  pub base: CodeNode,
  pub block: *const CodeBlock,
  pub is_backticked: bool,
}

/// `Code_Using`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeUsing {
  pub base: CodeNode,
  pub expression: *const CodeNode,
  pub filter_type: u8,
  pub filter_expression: *const CodeNode,
  pub no_parameters: bool,
}

/// `Code_Push_Context`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodePushContext {
  pub base: CodeNode,
  pub to_push: *const CodeNode,
  pub block: *const CodeBlock,
  pub push_context_flags: u32,
}

/// `Code_Unary_Operator`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeUnaryOperator {
  pub base: CodeNode,
  pub operator_type: i32,
  pub subexpression: *const CodeNode,
}

/// `Code_Binary_Operator`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeBinaryOperator {
  pub base: CodeNode,
  pub operator_type: i32,
  pub flags: u16,
  pub left: *const CodeNode,
  pub right: *const CodeNode,
}

/// `Code_Comma_Separated_Argument`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeCommaSeparatedArgument {
  pub node: *const CodeNode,
  pub modifier: u8,
}

/// `Code_Comma_Separated_Arguments`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeCommaSeparatedArguments {
  pub base: CodeNode,
  pub arguments: Slice,
}

/// `Code_Compound_Declaration`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeCompoundDeclaration {
  pub entry: CodeNode,
  pub comma_separated_assignment: *const CodeCommaSeparatedArguments,
  pub declaration_properties: *const CodeDeclaration,
  pub alignment_expression: *const CodeNode,
  pub notes: Slice,
  pub operator_type: i32,
}

/// `Code_Extract`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeExtract {
  pub base: CodeNode,
  pub from: *const CodeNode,
  pub index: i64,
}

/// `Code_Make_Varargs`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeMakeVarargs {
  pub base: CodeNode,
  pub element_type: *const c_void,
  pub expressions: Slice,
  pub is_for_non_native_calling_convention: bool,
}

/// `Code_Note`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeNote {
  pub base: CodeNode,
  pub text: Str,
  pub note_flags: u32,
}

/// `Code_Asm`, whose contents the reference does not expose (**L§15**).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeAsm {
  pub base: CodeNode,
  pub b1: *const c_void,
  pub b2: Slice,
  pub b3: Slice,
}

/// `Code_Directive_Run`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveRun {
  pub base: CodeNode,
  pub procedure: *const CodeProcedureHeader,
  pub flags: u32,
  pub assertion_string: Str,
}

/// `Code_Directive_Code`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveCode {
  pub base: CodeNode,
  pub expression: *const CodeNode,
  pub code_flags: u32,
}

/// `Code_Directive_Insert`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveInsert {
  pub base: CodeNode,
  pub expression: *const CodeNode,
  pub scope_redirection: *const CodeNode,
  pub break_replacement: *const CodeNode,
  pub continue_replacement: *const CodeNode,
  pub remove_replacement: *const CodeNode,
  pub expansion: *const CodeNode,
  pub is_internal: bool,
}

/// `Code_Directive_Import`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveImport {
  pub base: CodeNode,
  pub name: Str,
  pub flags: u32,
  pub import_type: u8,
  pub module_parameters_call: *const CodeProcedureCall,
  pub program_parameters_call: *const CodeProcedureCall,
}

/// `Code_Directive_Load`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveLoad {
  pub base: CodeNode,
  pub short_name: Str,
  pub fully_pathed_filename: Str,
  pub loaded_string: Str,
  pub load_flags: i32,
}

/// `Code_Directive_Library`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveLibrary {
  pub base: CodeNode,
  pub name: Str,
  pub library_flags: u32,
}

/// `Code_Directive_Bake`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveBake {
  pub base: CodeNode,
  pub procedure_call: *const CodeProcedureCall,
  pub bake_type: u8,
}

/// `Code_Directive_Modify`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveModify {
  pub base: CodeNode,
  pub block: *const CodeBlock,
}

/// `Code_Directive_Scope`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveScope {
  pub base: CodeNode,
  pub scope_type: u32,
}

/// `Code_Directive_Module_Parameters`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveModuleParameters {
  pub base: CodeNode,
  pub module_parameters: *const CodeProcedureHeader,
  pub program_parameters: *const CodeProcedureHeader,
  pub common_code: *const CodeBlock,
}

/// `Code_Directive_Location`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveLocation {
  pub base: CodeNode,
  pub expression: *const CodeNode,
  pub is_caller_location: bool,
}

/// `Code_Directive_Place`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectivePlace {
  pub base: CodeNode,
  pub ident: *const CodeIdent,
}

/// `Code_Directive_Poke_Name`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectivePokeName {
  pub base: CodeNode,
  pub module_struct: *const CodeStruct,
  pub name: Str,
}

/// `Code_Directive_Add_Context`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveAddContext {
  pub base: CodeNode,
  pub expression: *const CodeNode,
}

/// `Code_Directive_Procedure_Name`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveProcedureName {
  pub base: CodeNode,
  pub argument: *const CodeNode,
}

/// `Code_Directive_Exists`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveExists {
  pub base: CodeNode,
  pub query_expression: *const CodeNode,
  pub sync_expression: *const CodeNode,
}

/// `Code_Directive_Wildcard`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveWildcard {
  pub base: CodeNode,
  pub index: i32,
}

/// `Code_Directive_Bytes`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CodeDirectiveBytes {
  pub base: CodeNode,
  pub expression: *const CodeNode,
}

/// `Typechecked(T)` (**C§3.2**), whose `expression` a metaprogram casts by the
/// array it came out of.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Typechecked {
  pub expression: *const CodeNode,
  pub subexpressions: Slice,
}

/// `Message_Typechecked` (**C§3.2**).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MessageTypechecked {
  pub message: Message,
  pub declarations: Slice,
  pub procedure_headers: Slice,
  pub procedure_bodies: Slice,
  pub structs: Slice,
  pub others: Slice,
  pub all: Slice,
}

/// Storage whose addresses do not move for as long as the compilation lives.
///
/// A metaprogram keeps every `*Code_Node` it was handed, so nothing exported
/// is ever freed before the compilation ends; each allocation is a block of
/// its own, which is what makes an address stable while the arena grows.
#[derive(Debug, Default)]
pub struct Arena {
  blocks: Vec<Box<[u64]>>,
}

impl Arena {
  /// Copies `value` into the arena and hands back where it landed. `T` must be
  /// plain data — nothing here is ever dropped.
  pub fn alloc<T: Copy>(&mut self, value: T) -> *mut T {
    assert!(align_of::<T>() <= align_of::<u64>());
    let words = size_of::<T>().div_ceil(size_of::<u64>()).max(1);
    let mut block = vec![0u64; words].into_boxed_slice();
    let address = block.as_mut_ptr().cast::<T>();
    unsafe { address.write(value) };
    self.blocks.push(block);
    address
  }

  /// Reserves storage for a `T` whose members are filled in afterwards, which
  /// is what a tree with a cycle in it needs: the address has to exist before
  /// the members that point back at it can be written.
  ///
  /// Every mirror in this module is plain data whose all-zero bit pattern is a
  /// value — a null pointer, a `false`, an empty `string` — so the reserved
  /// storage is readable before anything fills it.
  pub fn alloc_zeroed<T: Copy>(&mut self) -> *mut T {
    assert!(align_of::<T>() <= align_of::<u64>());
    let words = size_of::<T>().div_ceil(size_of::<u64>()).max(1);
    let mut block = vec![0u64; words].into_boxed_slice();
    let address = block.as_mut_ptr().cast::<T>();
    self.blocks.push(block);
    address
  }

  /// Copies `items` into the arena and describes them the way a `[] T` does.
  pub fn alloc_slice<T: Copy>(&mut self, items: &[T]) -> Slice {
    if items.is_empty() {
      return Slice::EMPTY;
    }
    assert!(align_of::<T>() <= align_of::<u64>());
    let words = std::mem::size_of_val(items)
      .div_ceil(size_of::<u64>())
      .max(1);
    let mut block = vec![0u64; words].into_boxed_slice();
    let address = block.as_mut_ptr().cast::<T>();
    unsafe { std::ptr::copy_nonoverlapping(items.as_ptr(), address, items.len()) };
    self.blocks.push(block);
    Slice {
      count: items.len() as i64,
      data: address.cast(),
    }
  }

  /// Copies `text` into the arena and describes it the way a Jai `string`
  /// does.
  pub fn alloc_str(&mut self, text: &[u8]) -> Str {
    let slice = self.alloc_slice(text);
    Str {
      count: slice.count,
      data: slice.data,
    }
  }
}

/// What `compiler_get_nodes` answers for one `Code` (**C§3.3**): the node the
/// `Code` was written at, and every node under it in the order they were
/// exported.
#[derive(Clone, Copy, Debug)]
pub struct Tree {
  pub root: *const CodeNode,
  pub expressions: Slice,
}

/// The trees this compilation has exported, addressed the way a metaprogram
/// addresses them: by the node pointer it holds.
#[derive(Debug, Default)]
pub struct Nodes {
  pub arena: Arena,
  trees: HashMap<usize, Tree>,
  placed: HashMap<Key, *mut CodeNode>,
  paths: HashMap<(u32, u32), std::path::PathBuf>,
  /// Which node an exported address came from, and how many bytes of it there
  /// are, so that what a metaprogram hands back can be recognised.
  origin: HashMap<usize, (Key, usize)>,
  /// The byte range each exported node was written at, and the text of the
  /// file it was written in. A node a metaprogram handed back unchanged prints
  /// as the text it was written as rather than as a reconstruction of it.
  spans: HashMap<Key, (u32, u32)>,
  texts: HashMap<(u32, u32), std::sync::Arc<[u8]>>,
  /// The bytes each exported node had when the compilation handed it over,
  /// which is what says whether the metaprogram has since written to it.
  shadow: HashMap<usize, Vec<u8>>,
  generation: u32,
  serial: i64,
}

/// What identifies an exported node: the compilation it belongs to, and the
/// `(source, node)` it was written at. The compilation has to be part of it
/// because one arena outlives many of them — a workspace compiled again after
/// a `provide_import` numbers its sources from zero just as the first attempt
/// did (**C§3.2**).
pub type Key = (u32, u32, u32);

impl Nodes {
  /// Records what `compiler_get_nodes` should answer for a root.
  pub fn record(&mut self, tree: Tree) {
    self.trees.insert(tree.root as usize, tree);
  }

  /// The tree a `Code` names, or `None` when the pointer is not one this
  /// compilation exported.
  pub fn tree(&self, root: *const CodeNode) -> Option<Tree> {
    self.trees.get(&(root as usize)).copied()
  }

  /// Where a node of the program was exported to, if it was. One AST node has
  /// one exported address for the whole compilation, so a metaprogram that
  /// meets the same declaration twice is handed the same pointer.
  pub fn placed(&self, key: Key) -> Option<*mut CodeNode> {
    self.placed.get(&key).copied()
  }

  /// Starts a compilation: everything it exports is keyed under a number of
  /// its own, so that two compilations sharing this arena cannot be mistaken
  /// for each other.
  pub fn begin_compilation(&mut self) -> u32 {
    self.generation += 1;
    self.generation
  }

  /// Reserves storage for the node `key` names and records where it went. The
  /// members are filled in afterwards, so that a tree pointing back at itself
  /// has an address to point at.
  pub fn place<T: Copy>(&mut self, key: Key) -> *mut T {
    let address = self.arena.alloc_zeroed::<T>();
    self.placed.insert(key, address.cast());
    self.origin.insert(address as usize, (key, size_of::<T>()));
    address
  }

  /// Records where a node was written and the text of the file it was written
  /// in, which together are what it prints as.
  pub fn record_span(&mut self, key: Key, span: (u32, u32), text: &[u8]) {
    self.spans.insert(key, span);
    self
      .texts
      .entry((key.0, key.1))
      .or_insert_with(|| std::sync::Arc::from(text));
  }

  /// Takes the picture of every exported node that later tells whether a
  /// metaprogram has written to it. Nothing the compiler fills in itself may
  /// happen after this.
  pub fn freeze(&mut self) {
    for (address, (_, size)) in &self.origin {
      let bytes = unsafe { std::slice::from_raw_parts(*address as *const u8, *size) };
      self.shadow.insert(*address, bytes.to_vec());
    }
  }

  /// Whether `node` is one this compiler exported and still holds the bytes it
  /// was exported with. A node the metaprogram made itself, and one it has
  /// written to, are both `false`.
  pub fn unchanged(&self, node: *const CodeNode) -> bool {
    let Some((_, size)) = self.origin.get(&(node as usize)) else {
      return false;
    };
    let Some(shadow) = self.shadow.get(&(node as usize)) else {
      return false;
    };
    let bytes = unsafe { std::slice::from_raw_parts(node.cast::<u8>(), *size) };
    bytes == shadow.as_slice()
  }

  /// Where every node a metaprogram has written to was written. A node whose
  /// own text encloses one of these has to be printed rather than quoted, since
  /// the change is somewhere inside it.
  pub fn changed_spans(&self) -> Vec<(Key, (u32, u32))> {
    let mut changed = Vec::new();
    for (address, (key, _)) in &self.origin {
      if self.unchanged(*address as *const CodeNode) {
        continue;
      }
      if let Some(span) = self.spans.get(key) {
        changed.push((*key, *span));
      }
    }
    changed
  }

  /// Which node `node` was exported from and where it was written.
  pub fn span_of(&self, node: *const CodeNode) -> Option<(Key, (u32, u32))> {
    let (key, _) = self.origin.get(&(node as usize))?;
    Some((*key, *self.spans.get(key)?))
  }

  /// The text `node` was written as, when it is one of the program's own.
  pub fn source_text(&self, node: *const CodeNode) -> Option<&[u8]> {
    let (key, _) = self.origin.get(&(node as usize))?;
    let (start, end) = *self.spans.get(key)?;
    let text = self.texts.get(&(key.0, key.1))?;
    text.get(start as usize..end as usize)
  }

  /// `Code_Node.serial`, which numbers the nodes in the order the compiler
  /// made them (**C§5.3**).
  pub fn next_serial(&mut self) -> i64 {
    self.serial += 1;
    self.serial
  }

  /// Every node exported so far, by the compilation, source and node it came from. A pass
  /// that fills in what a first walk could not yet know — a name whose
  /// declaration had not been exported when the name was — reads this.
  pub fn placed_entries(&self) -> Vec<(Key, *mut CodeNode)> {
    self
      .placed
      .iter()
      .map(|(key, address)| (*key, *address))
      .collect()
  }

  /// Which file a source came from, so that the `Message_File` the metaprogram
  /// is handed can be found again once it exists.
  pub fn record_path(&mut self, key: (u32, u32), path: std::path::PathBuf) {
    self.paths.insert(key, path);
  }

  /// Points every exported node at the `Message_File` of the file it was
  /// written in (**C§3.2**). The messages are made after the compilation they
  /// describe, so `enclosing_load` is filled in here rather than at export.
  pub fn attach_files(&mut self, by_path: &HashMap<std::path::PathBuf, *const MessageFile>) {
    for ((generation, source, _), address) in &self.placed {
      let Some(path) = self.paths.get(&(*generation, *source)) else {
        continue;
      };
      let Some(message) = by_path.get(path) else {
        continue;
      };
      unsafe { (**address).location.enclosing_load = *message };
    }
  }
}
