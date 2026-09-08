use std::fmt::Write;

use crate::program::Program;
use crate::tree::{DeclKind, ScopeId, ScopeKind, ScopeTree, Visibility};

/// Prints the scope tree of a resolved program: one line per scope, indented by
/// depth, then its declarations and the scopes it imports from. This is what
/// `oj dump scopes` writes.
pub fn print_scopes(program: &Program<'_>) -> String {
  let mut out = String::new();
  print_scope(program, program.preload_scope(), 0, &mut out);
  out
}

fn print_scope(program: &Program<'_>, scope: ScopeId, depth: usize, out: &mut String) {
  let tree = program.tree();
  let interner = program.interner();
  let indent = "  ".repeat(depth);
  let entry = tree.scope(scope);

  let _ = write!(out, "{indent}{}", entry.kind.name());
  if let Some(path) = &entry.path
    && entry.kind == ScopeKind::File
  {
    let _ = write!(out, " {}", path.display());
  }
  let _ = writeln!(out, " [{}]", scope.0);

  for id in &entry.declarations {
    let declaration = tree.decl(*id);
    let _ = write!(
      out,
      "{indent}  {} {}",
      declaration.kind.name(),
      interner.resolve_lossy(declaration.name)
    );
    // `#scope_*` only ever chose where a file's own declarations went, so it is
    // noise anywhere else.
    if declaration.visibility != Visibility::Export && entry.kind.is_program_scope() {
      let _ = write!(out, " #scope_{}", declaration.visibility.name());
    }
    if let DeclKind::Module(target) = declaration.kind {
      let _ = write!(out, " -> [{}]", target.0);
    }
    if declaration.conditional {
      let _ = write!(out, " (conditional)");
    }
    let _ = writeln!(out);
  }

  for edge in &entry.imports {
    let _ = writeln!(out, "{indent}  using [{}]", edge.target.0);
  }
  for pending in &entry.pending {
    let _ = writeln!(out, "{indent}  pending {pending:?}");
  }

  for child in &entry.children {
    print_scope(program, *child, depth + 1, out);
  }
}

/// A one-line summary of what a build resolved, for `oj dump scopes` to close
/// with and for tests to assert on.
pub fn summary(tree: &ScopeTree, files: usize) -> String {
  format!(
    "{files} {}, {} scopes, {} declarations",
    if files == 1 { "file" } else { "files" },
    tree.scope_count(),
    tree.declaration_count()
  )
}
