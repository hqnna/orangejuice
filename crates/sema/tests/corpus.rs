use std::path::{Path, PathBuf};

use oj_diag::{Severity, SourceMap};
use oj_lexer::Interner;
use oj_scope::{Options, Program};
use oj_sema::Checker;
use walkdir::WalkDir;

fn jai_files(root: &Path) -> Vec<PathBuf> {
  let mut files: Vec<PathBuf> = WalkDir::new(root)
    .into_iter()
    .filter_map(Result::ok)
    .filter(|entry| entry.file_type().is_file())
    .map(|entry| entry.into_path())
    .filter(|path| path.extension().is_some_and(|extension| extension == "jai"))
    // A `.build` directory holds what a compiler left beside a program,
    // not part of the distribution: whoever has built one of its programs in
    // place has one.
    .filter(|path| {
      !path
        .components()
        .any(|component| component.as_os_str() == ".build")
    })
    .collect();
  files.sort();
  files
}

fn rendered_errors(checker: &Checker<'_>, sources: &SourceMap) -> Vec<String> {
  checker
    .diagnostics()
    .iter()
    .filter(|diagnostic| diagnostic.severity == Severity::Error)
    .map(|diagnostic| {
      oj_diag::render(diagnostic, &sources.file(diagnostic.source))
        .trim_end()
        .to_string()
    })
    .collect()
}

/// Every file of the distribution types on its own
/// (`docs/spec.md` §8). Resolved without its imports, most of a file's names
/// are unknown, so what this checks is that the typechecker itself never
/// contradicts the file — a circular dependency, a struct that cannot be laid
/// out, a runaway resolution.
#[test]
fn every_file_of_the_distribution_typechecks_on_its_own() {
  let mut files = jai_files(&oj_testsupport::modules());
  files.extend(jai_files(&oj_testsupport::examples()));
  files.sort();
  assert!(
    files.len() > 70,
    "expected the distribution's own .jai corpus, found {} files",
    files.len()
  );

  let mut failures = Vec::new();
  let mut types = 0;

  for path in &files {
    let sources = SourceMap::new();
    let interner = Interner::new();
    let program = Program::build(&sources, &interner, path, Options::single_file());
    let mut checker = Checker::new(&program);
    checker.check();

    let errors = rendered_errors(&checker, &sources);
    if !errors.is_empty() {
      failures.push(format!("{}:\n{}", path.display(), errors.join("\n")));
      continue;
    }
    types += checker.types().len();
  }

  assert!(
    failures.is_empty(),
    "{} of {} corpus files failed to typecheck:\n{}",
    failures.len(),
    files.len(),
    failures.join("\n\n")
  );
  assert!(types > files.len() * 25, "{types} types looks too small");
}

/// The `examples` suite is the acceptance suite (`docs/spec.md` §8): each of
/// its programs is typed as a whole — its `#load`s, its modules and Preload —
/// and the typechecker has nothing to complain about.
#[test]
fn every_example_program_typechecks() {
  let examples = oj_testsupport::examples();
  let mut roots: Vec<PathBuf> = jai_files(&examples)
    .into_iter()
    .filter(|path| path.parent() == Some(examples.as_path()))
    .collect();
  roots.sort();
  assert!(roots.len() > 10, "expected the examples suite");

  let mut failures = Vec::new();
  for path in &roots {
    let sources = SourceMap::new();
    let interner = Interner::new();
    let program = Program::build(
      &sources,
      &interner,
      path,
      Options {
        distribution: Some(oj_testsupport::distribution().to_path_buf()),
        ..Options::default()
      },
    );
    let mut checker = Checker::new(&program);
    checker.check();

    let errors = rendered_errors(&checker, &sources);
    if !errors.is_empty() {
      failures.push(format!("{}:\n{}", path.display(), errors.join("\n")));
    }
  }

  assert!(
    failures.is_empty(),
    "{} of {} example programs did not typecheck:\n{}",
    failures.len(),
    roots.len(),
    failures.join("\n\n")
  );
}

/// The struct layouts the runtime ABI is built on, each one measured against
/// the reference compiler (`size_of(T)` in a program that imports the module)
/// while one was still on hand. They are pinned here rather than left to the
/// acceptance suite to notice, because a program whose `Context` is the wrong
/// size fails somewhere else entirely.
#[test]
fn the_standard_module_layouts_are_the_sizes_they_were_measured_at() {
  let root = oj_testsupport::examples().join("010_hello.jai");
  assert!(root.is_file(), "{} should be there", root.display());

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
  checker.check();

  const EXPECTED: [(&str, u64); 17] = [
    ("Type_Info", 16),
    ("Type_Info_Integer", 24),
    ("Type_Info_Struct", 128),
    ("Type_Info_Struct_Member", 64),
    ("Type_Info_Enum", 80),
    ("Type_Info_Procedure", 56),
    ("Type_Info_Array", 40),
    ("Type_Info_Variant", 48),
    ("Any_Struct", 16),
    ("Source_Code_Location", 32),
    ("Stack_Trace_Node", 32),
    ("Resizable_Array", 40),
    ("Allocator", 16),
    ("Context_Base", 96),
    ("Temporary_Storage", 112),
    ("Log_Info", 56),
    ("String_Builder", 4104),
  ];

  let tree = program.tree();
  let mut wrong = Vec::new();
  for (name, expected) in EXPECTED {
    let symbol = interner.intern(name.as_bytes());
    let Some(id) = (0..tree.declaration_count() as u32)
      .map(oj_scope::DeclId)
      .find(|id| tree.decl(*id).name == symbol && checker.resolved(*id).is_some())
    else {
      wrong.push(format!("{name}: not declared"));
      continue;
    };
    let Some(denoted) = checker.decl_type(id).denoted else {
      wrong.push(format!("{name}: not a type"));
      continue;
    };
    match checker.types().size_of(denoted) {
      Some(size) if size == expected => {}
      other => wrong.push(format!("{name}: wanted {expected}, got {other:?}")),
    }
  }

  assert!(wrong.is_empty(), "layouts differ:\n{}", wrong.join("\n"));
}
