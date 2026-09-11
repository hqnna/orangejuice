//! `oj-meta` is an ABI, not an API: a metaprogram is handed pointers to these
//! structs and reads them with the distribution's own declarations, so every
//! mirror has to match `Compiler.jai` byte for byte (`docs/spec.md` §6).
//!
//! The measurement is the compiler's own front end reading the distribution's
//! own `Compiler` module, which is the only source that cannot drift from what
//! a metaprogram will actually see.

use std::mem::{offset_of, size_of};

use oj_diag::SourceMap;
use oj_lexer::Interner;
use oj_meta::{
  CodeArgument, CodeArrayLiteralInfo, CodeAsm, CodeBinaryOperator, CodeBlock, CodeCase, CodeCast,
  CodeCommaSeparatedArgument, CodeCommaSeparatedArguments, CodeCompoundDeclaration,
  CodeDeclaration, CodeDefer, CodeDirectiveAddContext, CodeDirectiveBake, CodeDirectiveBytes,
  CodeDirectiveCode, CodeDirectiveExists, CodeDirectiveImport, CodeDirectiveInsert,
  CodeDirectiveLibrary, CodeDirectiveLoad, CodeDirectiveLocation, CodeDirectiveModify,
  CodeDirectiveModuleParameters, CodeDirectivePlace, CodeDirectivePokeName,
  CodeDirectiveProcedureName, CodeDirectiveRun, CodeDirectiveScope, CodeDirectiveWildcard,
  CodeEnum, CodeExpressionQuery, CodeExtract, CodeFor, CodeIdent, CodeIf, CodeLiteral,
  CodeLoopControl, CodeMakeVarargs, CodeNode, CodeNote, CodePointerLiteralInfo, CodeProcedureBody,
  CodeProcedureCall, CodeProcedureHeader, CodePushContext, CodeResolvedOverload, CodeReturn,
  CodeScopeEntry, CodeStruct, CodeStructLiteralInfo, CodeTypeDefinition, CodeTypeInstantiation,
  CodeTypeQuery, CodeUnaryOperator, CodeUsing, CodeWhile, Message, MessageComplete,
  MessageFailedImport, MessageFile, MessageImport, MessagePhase, MessageTypechecked,
  SourceCodeLocation, Str, Typechecked, VersionInfo,
};
use oj_scope::{Options, Program};
use oj_sema::Checker;

/// One member of a Jai struct, as the front end laid it out.
struct Layout {
  name: String,
  size: u64,
  members: Vec<(String, u64)>,
}

impl Layout {
  fn offset(&self, member: &str) -> u64 {
    self
      .members
      .iter()
      .find(|(name, _)| name == member)
      .unwrap_or_else(|| panic!("'{}' has no member '{member}'", self.name))
      .1
  }

  fn assert_offset(&self, member: &str, expected: usize) {
    assert_eq!(
      self.offset(member),
      expected as u64,
      "{}.{member} is at a different offset than the mirror",
      self.name
    );
  }

  fn assert_size(&self, expected: usize) {
    assert_eq!(
      self.size, expected as u64,
      "{} is a different size than the mirror",
      self.name
    );
  }
}

/// A `Typechecked(T)` is polymorphic, so the fixture bakes one down to a name
/// the checker can be asked for.
const FIXTURE: &str = "\
#import \"Compiler\";
Typechecked_Probe :: struct { using entry: Typechecked(Code_Node); }
Enum_Probe :: struct {
    kind:        Message_Kind;
    module_type: Module_Type;
    status:      Import_Status;
    phase:       Phase;
    error_code:  Error_Code;
}
main :: () {}
";

/// The Rust mirror of `Enum_Probe`. A field's *offset* is what the width of the
/// field before it comes to, so this is how an enum's width is measured: an
/// enum's own size is invisible to a struct whose next member is wider, which
/// is exactly how a `#[repr(u8)]` mirror of an `enum u32` went unnoticed —
/// `Message.workspace` sat at the right offset either way, and a metaprogram
/// read three bytes of padding as part of `kind`.
#[repr(C)]
struct EnumProbe {
  kind: oj_meta::Kind,
  module_type: oj_meta::ModuleType,
  status: oj_meta::ImportStatus,
  phase: oj_meta::Phase,
  error_code: oj_meta::ErrorCode,
}

fn measure(names: &[&str], check: impl FnOnce(&[Layout])) {
  let directory = tempfile::tempdir().expect("a temporary directory");
  let root = directory.path().join("main.jai");
  std::fs::write(&root, FIXTURE).expect("the fixture is writable");

  let sources = SourceMap::new();
  let interner = Interner::new();
  let program = Program::build(
    &sources,
    &interner,
    &root,
    Options {
      distribution: Some(oj_testsupport::distribution().to_path_buf()),
      ..Options::default()
    },
  );
  let mut checker = Checker::new(&program);

  let layouts: Vec<Layout> = names
    .iter()
    .map(|name| {
      let type_id = checker
        .type_named(name)
        .unwrap_or_else(|| panic!("the Compiler module should declare '{name}'"));
      let size = checker
        .layout(type_id)
        .unwrap_or_else(|| panic!("'{name}' should have a layout"))
        .size;
      let definition = checker
        .types()
        .struct_of(type_id)
        .unwrap_or_else(|| panic!("'{name}' should be a struct"));
      let members = checker
        .types()
        .struct_info(definition)
        .members
        .iter()
        .filter(|member| !member.is_constant())
        .map(|member| {
          (
            interner.resolve_lossy(member.name).into_owned(),
            member.offset,
          )
        })
        .collect();
      Layout {
        name: (*name).to_string(),
        size,
        members,
      }
    })
    .collect();
  check(&layouts);
}

#[test]
fn the_message_mirrors_match_the_module() {
  measure(
    &[
      "Message",
      "Message_File",
      "Message_Import",
      "Message_Phase",
      "Message_Complete",
      "Message_Failed_Import",
    ],
    |layouts| {
      let [message, file, import, phase, complete, failed] = layouts else {
        unreachable!("six layouts were asked for");
      };

      message.assert_size(size_of::<Message>());
      message.assert_offset("kind", offset_of!(Message, kind));
      message.assert_offset("workspace", offset_of!(Message, workspace));

      file.assert_size(size_of::<MessageFile>());
      file.assert_offset("kind", offset_of!(MessageFile, message));
      file.assert_offset(
        "fully_pathed_filename",
        offset_of!(MessageFile, fully_pathed_filename),
      );
      file.assert_offset(
        "enclosing_import",
        offset_of!(MessageFile, enclosing_import),
      );
      file.assert_offset("from_a_string", offset_of!(MessageFile, from_a_string));

      import.assert_size(size_of::<MessageImport>());
      import.assert_offset("module_type", offset_of!(MessageImport, module_type));
      import.assert_offset("module_name", offset_of!(MessageImport, module_name));
      import.assert_offset(
        "fully_pathed_filename",
        offset_of!(MessageImport, fully_pathed_filename),
      );

      phase.assert_size(size_of::<MessagePhase>());
      phase.assert_offset("phase", offset_of!(MessagePhase, phase));
      phase.assert_offset("executable_name", offset_of!(MessagePhase, executable_name));
      phase.assert_offset(
        "executable_write_failed",
        offset_of!(MessagePhase, executable_write_failed),
      );
      phase.assert_offset(
        "linker_exit_code",
        offset_of!(MessagePhase, linker_exit_code),
      );
      phase.assert_offset(
        "num_items_waiting_to_typecheck",
        offset_of!(MessagePhase, num_items_waiting_to_typecheck),
      );
      phase.assert_offset(
        "compiler_generated_object_files",
        offset_of!(MessagePhase, compiler_generated_object_files),
      );
      phase.assert_offset(
        "support_object_files",
        offset_of!(MessagePhase, support_object_files),
      );
      phase.assert_offset(
        "system_libraries",
        offset_of!(MessagePhase, system_libraries),
      );
      phase.assert_offset("user_libraries", offset_of!(MessagePhase, user_libraries));

      complete.assert_size(size_of::<MessageComplete>());
      complete.assert_offset("error_code", offset_of!(MessageComplete, error_code));

      failed.assert_size(size_of::<MessageFailedImport>());
      failed.assert_offset("status", offset_of!(MessageFailedImport, status));
      failed.assert_offset(
        "host_module_name",
        offset_of!(MessageFailedImport, host_module_name),
      );
      failed.assert_offset(
        "target_module_name",
        offset_of!(MessageFailedImport, target_module_name),
      );
      failed.assert_offset("import_code", offset_of!(MessageFailedImport, import_code));
    },
  );
}

#[test]
fn the_preload_mirrors_match_the_module() {
  measure(&["Source_Code_Location", "Version_Info"], |layouts| {
    let [location, version] = layouts else {
      unreachable!("two layouts were asked for");
    };

    location.assert_size(size_of::<SourceCodeLocation>());
    location.assert_offset(
      "fully_pathed_filename",
      offset_of!(SourceCodeLocation, fully_pathed_filename),
    );
    location.assert_offset("line_number", offset_of!(SourceCodeLocation, line_number));
    location.assert_offset(
      "character_number",
      offset_of!(SourceCodeLocation, character_number),
    );

    version.assert_size(size_of::<VersionInfo>());
    version.assert_offset("major", offset_of!(VersionInfo, major));
    version.assert_offset("minor", offset_of!(VersionInfo, minor));
    version.assert_offset("micro", offset_of!(VersionInfo, micro));
  });
}

#[test]
fn a_jai_string_is_a_count_and_a_pointer() {
  assert_eq!(size_of::<Str>(), 16);
  assert_eq!(offset_of!(Str, count), 0);
  assert_eq!(offset_of!(Str, data), 8);
}

/// Checks one mirror against the layout the front end measured: the size, and
/// every member the reference declares, by the name a metaprogram writes.
macro_rules! check {
  ($layouts:expr, $index:expr, $mirror:ty $(, $member:literal => $($field:ident).+)* $(,)?) => {{
    let layout = &$layouts[$index];
    layout.assert_size(size_of::<$mirror>());
    $(layout.assert_offset($member, offset_of!($mirror, $($field).+));)*
  }};
}

/// The names measured, in the order `the_node_mirrors_match_the_module` reads
/// them back.
const NODE_STRUCTS: &[&str] = &[
  "Code_Node",
  "Code_Scope_Entry",
  "Code_Declaration",
  "Code_Block",
  "Code_Ident",
  "Code_Literal",
  "Code_Struct_Literal_Info",
  "Code_Array_Literal_Info",
  "Code_Pointer_Literal_Info",
  "Code_Type_Instantiation",
  "Code_Type_Definition",
  "Code_Enum",
  "Code_Argument",
  "Code_Procedure_Call",
  "Code_Procedure_Header",
  "Code_Procedure_Body",
  "Code_Resolved_Overload",
  "Code_Struct",
  "Code_Cast",
  "Code_Type_Query",
  "Code_Expression_Query",
  "Code_If",
  "Code_Case",
  "Code_While",
  "Code_For",
  "Code_Loop_Control",
  "Code_Return",
  "Code_Defer",
  "Code_Using",
  "Code_Push_Context",
  "Code_Unary_Operator",
  "Code_Binary_Operator",
  "Code_Comma_Separated_Argument",
  "Code_Comma_Separated_Arguments",
  "Code_Compound_Declaration",
  "Code_Extract",
  "Code_Make_Varargs",
  "Code_Note",
  "Code_Asm",
  "Code_Directive_Run",
  "Code_Directive_Code",
  "Code_Directive_Insert",
  "Code_Directive_Import",
  "Code_Directive_Load",
  "Code_Directive_Library",
  "Code_Directive_Bake",
  "Code_Directive_Modify",
  "Code_Directive_Scope",
  "Code_Directive_Module_Parameters",
  "Code_Directive_Location",
  "Code_Directive_Place",
  "Code_Directive_Poke_Name",
  "Code_Directive_Add_Context",
  "Code_Directive_Procedure_Name",
  "Code_Directive_Exists",
  "Code_Directive_Wildcard",
  "Code_Directive_Bytes",
  "Code_Context",
  "Code_Placeholder",
  "Code_Directive_Through",
  "Code_Directive_Context_Type",
  "Message_Typechecked",
];

#[test]
fn the_node_mirrors_match_the_module() {
  measure(NODE_STRUCTS, |l| {
    check!(l, 0, CodeNode,
      "kind" => kind,
      "node_flags" => node_flags,
      "type" => type_info,
      "enclosing_load" => location.enclosing_load,
      "l0" => location.l0,
      "c0" => location.c0,
      "l1" => location.l1,
      "c1" => location.c1,
      "serial" => serial);
    check!(l, 1, CodeScopeEntry, "name" => name, "import_target" => import_target);
    check!(l, 2, CodeDeclaration,
      "type_inst" => type_inst,
      "expression" => expression,
      "flags" => flags,
      "alignment_expression" => alignment_expression,
      "notes" => notes,
      "program_export_name" => program_export_name);
    check!(l, 3, CodeBlock,
      "parent" => parent,
      "block_type" => block_type,
      "block_flags" => block_flags,
      "belongs_to_struct" => belongs_to_struct,
      "members" => members,
      "statements" => statements,
      "owning_statement" => owning_statement);
    check!(l, 4, CodeIdent,
      "name" => name,
      "resolved_declaration" => resolved_declaration,
      "flags" => flags);
    check!(l, 5, CodeLiteral,
      "value_type" => value_type,
      "values" => values,
      "value_flags" => value_flags);
    check!(l, 6, CodeStructLiteralInfo,
      "type_expression" => type_expression,
      "arguments" => arguments);
    check!(l, 7, CodeArrayLiteralInfo,
      "element_type" => element_type,
      "alignment" => alignment,
      "array_members" => array_members,
      "array_literal_flags" => array_literal_flags);
    check!(l, 8, CodePointerLiteralInfo,
      "global_symbol" => global_symbol,
      "data_pointer" => data_pointer,
      "pointer_literal_type" => pointer_literal_type,
      "offset_from_symbol" => offset_from_symbol);
    check!(l, 9, CodeTypeInstantiation,
      "result" => result,
      "type_valued_expression" => type_valued_expression,
      "must_implement" => must_implement,
      "pointer_to" => pointer_to,
      "type_directive_target" => type_directive_target,
      "array_element_type" => array_element_type,
      "array_dimension" => array_dimension,
      "inst_flags" => inst_flags);
    check!(l, 10, CodeTypeDefinition, "info" => info);
    check!(l, 11, CodeEnum,
      "internal_type_inst" => internal_type_inst,
      "internal_type" => internal_type,
      "external_type" => external_type,
      "block" => block,
      "notes" => notes,
      "marked_as_complete" => marked_as_complete,
      "marked_as_specified" => marked_as_specified,
      "is_flags" => is_flags);
    check!(l, 12, CodeArgument, "expression" => expression, "name" => name);
    check!(l, 13, CodeProcedureCall,
      "procedure_expression" => procedure_expression,
      "resolved_procedure_expression" => resolved_procedure_expression,
      "overloads" => overloads,
      "arguments_unsorted" => arguments_unsorted,
      "arguments_sorted" => arguments_sorted,
      "num_return_values_received" => num_return_values_received,
      "macro_expansion_block" => macro_expansion_block,
      "context_modification" => context_modification,
      "flags" => flags);
    check!(l, 14, CodeProcedureHeader,
      "constants_block" => constants_block,
      "arguments" => arguments,
      "returns" => returns,
      "parameter_usings" => parameter_usings,
      "name" => name,
      "foreign_function_name" => foreign_function_name,
      "library_identifier" => library_identifier,
      "deprecation_string" => deprecation_string,
      "polymorph_source_header" => polymorph_source_header,
      "modify_directives" => modify_directives,
      "body_or_null" => body_or_null,
      "procedure_flags" => procedure_flags,
      "notes" => notes);
    check!(l, 15, CodeProcedureBody,
      "block" => block,
      "header" => header,
      "body_flags" => body_flags);
    check!(l, 16, CodeResolvedOverload,
      "result" => result,
      "source_expression" => source_expression);
    check!(l, 17, CodeStruct,
      "modify_directives" => modify_directives,
      "block" => block,
      "arguments_block" => arguments_block,
      "constants_block" => constants_block,
      "notes" => notes,
      "textual_flags" => textual_flags,
      "alignment" => alignment,
      "defined_type" => defined_type);
    check!(l, 18, CodeCast,
      "target_type" => target_type,
      "expression" => expression,
      "cast_flags" => cast_flags);
    check!(l, 19, CodeTypeQuery, "query_kind" => query_kind, "type_to_query" => type_to_query);
    check!(l, 20, CodeExpressionQuery,
      "query_kind" => query_kind,
      "expression_to_query" => expression_to_query);
    check!(l, 21, CodeIf,
      "condition" => condition,
      "then_block" => then_block,
      "else_block" => else_block,
      "if_flags" => if_flags,
      "static_if_flags" => static_if_flags,
      "static_if_accepted_case" => static_if_accepted_case);
    check!(l, 22, CodeCase,
      "condition" => condition,
      "then_block" => then_block,
      "owning_if" => owning_if,
      "marked_as_fallthrough" => marked_as_fallthrough);
    check!(l, 23, CodeWhile, "condition" => condition, "block" => block);
    check!(l, 24, CodeFor,
      "iteration_expression" => iteration_expression,
      "iteration_expression_right" => iteration_expression_right,
      "block" => block,
      "ident_it" => ident_it,
      "ident_it_index" => ident_it_index,
      "ident_decl" => ident_decl,
      "index_decl" => index_decl,
      "want_replacement_for_expansion" => want_replacement_for_expansion,
      "want_pointer_expression" => want_pointer_expression,
      "want_reverse_expression" => want_reverse_expression,
      "macro_expansion_procedure_call" => macro_expansion_procedure_call,
      "for_flags" => for_flags);
    check!(l, 25, CodeLoopControl, "control_type" => control_type, "target_ident" => target_ident);
    check!(l, 26, CodeReturn,
      "arguments_unsorted" => arguments_unsorted,
      "arguments_sorted" => arguments_sorted,
      "return_flags" => return_flags);
    check!(l, 27, CodeDefer, "block" => block, "is_backticked" => is_backticked);
    check!(l, 28, CodeUsing,
      "expression" => expression,
      "filter_type" => filter_type,
      "filter_expression" => filter_expression,
      "no_parameters" => no_parameters);
    check!(l, 29, CodePushContext,
      "to_push" => to_push,
      "block" => block,
      "push_context_flags" => push_context_flags);
    check!(l, 30, CodeUnaryOperator,
      "operator_type" => operator_type,
      "subexpression" => subexpression);
    check!(l, 31, CodeBinaryOperator,
      "operator_type" => operator_type,
      "flags" => flags,
      "left" => left,
      "right" => right);
    check!(l, 32, CodeCommaSeparatedArgument, "node" => node, "modifier" => modifier);
    check!(l, 33, CodeCommaSeparatedArguments, "arguments" => arguments);
    check!(l, 34, CodeCompoundDeclaration,
      "comma_separated_assignment" => comma_separated_assignment,
      "declaration_properties" => declaration_properties,
      "alignment_expression" => alignment_expression,
      "notes" => notes,
      "operator_type" => operator_type);
    check!(l, 35, CodeExtract, "from" => from, "index" => index);
    check!(l, 36, CodeMakeVarargs,
      "element_type" => element_type,
      "expressions" => expressions,
      "is_for_non_native_calling_convention" => is_for_non_native_calling_convention);
    check!(l, 37, CodeNote, "text" => text, "note_flags" => note_flags);
    check!(l, 38, CodeAsm, "b1" => b1, "b2" => b2, "b3" => b3);
    check!(l, 39, CodeDirectiveRun,
      "procedure" => procedure,
      "flags" => flags,
      "assertion_string" => assertion_string);
    check!(l, 40, CodeDirectiveCode, "expression" => expression, "code_flags" => code_flags);
    check!(l, 41, CodeDirectiveInsert,
      "expression" => expression,
      "scope_redirection" => scope_redirection,
      "break_replacement" => break_replacement,
      "continue_replacement" => continue_replacement,
      "remove_replacement" => remove_replacement,
      "expansion" => expansion,
      "is_internal" => is_internal);
    check!(l, 42, CodeDirectiveImport,
      "name" => name,
      "flags" => flags,
      "import_type" => import_type,
      "module_parameters_call" => module_parameters_call,
      "program_parameters_call" => program_parameters_call);
    check!(l, 43, CodeDirectiveLoad,
      "short_name" => short_name,
      "fully_pathed_filename" => fully_pathed_filename,
      "loaded_string" => loaded_string,
      "load_flags" => load_flags);
    check!(l, 44, CodeDirectiveLibrary, "name" => name, "library_flags" => library_flags);
    check!(l, 45, CodeDirectiveBake, "procedure_call" => procedure_call, "bake_type" => bake_type);
    check!(l, 46, CodeDirectiveModify, "block" => block);
    check!(l, 47, CodeDirectiveScope, "scope_type" => scope_type);
    check!(l, 48, CodeDirectiveModuleParameters,
      "module_parameters" => module_parameters,
      "program_parameters" => program_parameters,
      "common_code" => common_code);
    check!(l, 49, CodeDirectiveLocation,
      "expression" => expression,
      "is_caller_location" => is_caller_location);
    check!(l, 50, CodeDirectivePlace, "ident" => ident);
    check!(l, 51, CodeDirectivePokeName, "module_struct" => module_struct, "name" => name);
    check!(l, 52, CodeDirectiveAddContext, "expression" => expression);
    check!(l, 53, CodeDirectiveProcedureName, "argument" => argument);
    check!(l, 54, CodeDirectiveExists,
      "query_expression" => query_expression,
      "sync_expression" => sync_expression);
    check!(l, 55, CodeDirectiveWildcard, "index" => index);
    check!(l, 56, CodeDirectiveBytes, "expression" => expression);
    // The nodes with nothing of their own are still a `Code_Node`'s worth of
    // storage, which is what the exporter allocates for them.
    check!(l, 57, CodeNode);
    check!(l, 58, CodeNode);
    check!(l, 59, CodeNode);
    check!(l, 60, CodeNode);
    check!(l, 61, MessageTypechecked,
      "declarations" => declarations,
      "procedure_headers" => procedure_headers,
      "procedure_bodies" => procedure_bodies,
      "structs" => structs,
      "others" => others,
      "all" => all);
  });
}

#[test]
fn a_typechecked_entry_is_an_expression_and_its_subexpressions() {
  measure(&["Typechecked_Probe"], |layouts| {
    layouts[0].assert_size(size_of::<Typechecked>());
    layouts[0].assert_offset("expression", offset_of!(Typechecked, expression));
    layouts[0].assert_offset("subexpressions", offset_of!(Typechecked, subexpressions));
  });
}

#[test]
fn every_enum_a_message_carries_is_as_wide_as_the_module_declares() {
  measure(&["Enum_Probe"], |layouts| {
    let probe = &layouts[0];
    probe.assert_size(size_of::<EnumProbe>());
    probe.assert_offset("kind", offset_of!(EnumProbe, kind));
    probe.assert_offset("module_type", offset_of!(EnumProbe, module_type));
    probe.assert_offset("status", offset_of!(EnumProbe, status));
    probe.assert_offset("phase", offset_of!(EnumProbe, phase));
    probe.assert_offset("error_code", offset_of!(EnumProbe, error_code));
  });
}
