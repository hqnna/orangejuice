//! `oj-meta` is an ABI, not an API: a metaprogram is handed pointers to these
//! structs and reads them with the distribution's own declarations, so every
//! mirror has to match `Compiler.jai` byte for byte (`docs/spec.md` §6).
//!
//! The measurement is the compiler's own front end reading the vendored
//! module, which is the only source that cannot drift from what a metaprogram
//! will actually see.

use std::mem::{offset_of, size_of};

use oj_diag::SourceMap;
use oj_lexer::Interner;
use oj_meta::{
  Message, MessageComplete, MessageFile, MessageImport, MessagePhase, SourceCodeLocation, Str,
  VersionInfo,
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

fn measure(names: &[&str], check: impl FnOnce(&[Layout])) {
  let Some(jai_dir) = oj_testsupport::jai_dir() else {
    eprintln!("{}", oj_testsupport::MISSING_JAI_DIR_MESSAGE);
    return;
  };
  let directory = tempfile::tempdir().expect("a temporary directory");
  let root = directory.path().join("main.jai");
  std::fs::write(&root, "#import \"Compiler\";\nmain :: () {}\n").expect("the fixture is writable");

  let sources = SourceMap::new();
  let interner = Interner::new();
  let program = Program::build(
    &sources,
    &interner,
    &root,
    Options {
      jai_dir: Some(jai_dir),
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
    ],
    |layouts| {
      let [message, file, import, phase, complete] = layouts else {
        unreachable!("five layouts were asked for");
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
