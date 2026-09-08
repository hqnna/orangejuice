use std::fmt::Write;

use oj_scope::{DeclKind, ScopeId, ScopeKind, Visibility};
use oj_types::{MemberFlags, TypeId, TypeKind};

use crate::checker::Checker;

/// Prints the types a program resolved: the scope tree of `oj dump scopes`
/// with each declaration's type beside it, and the layout of every struct and
/// enum it declares. This is what `oj dump types` writes.
pub fn print_types(checker: &Checker<'_>) -> String {
  let mut out = String::new();
  print_scope(checker, checker.program().preload_scope(), 0, &mut out);
  out
}

fn print_scope(checker: &Checker<'_>, scope: ScopeId, depth: usize, out: &mut String) {
  let tree = checker.program().tree();
  let indent = "  ".repeat(depth);
  let (entry_kind, entry_path, entry_declarations, entry_children) =
    tree.with_scope(scope, |entry| {
      (
        entry.kind,
        entry.path.clone(),
        entry.declarations.clone(),
        entry.children.clone(),
      )
    });

  let _ = write!(out, "{indent}{}", entry_kind.name());
  if let Some(path) = &entry_path
    && entry_kind == ScopeKind::File
  {
    let _ = write!(out, " {}", path.display());
  }
  let _ = writeln!(out, " [{}]", scope.0);

  for id in &entry_declarations {
    let declaration = tree.decl(*id);
    let name = checker.interner().resolve_lossy(declaration.name);
    let Some(resolved) = checker.resolved(*id) else {
      let _ = writeln!(out, "{indent}  {name}: <not typed>");
      continue;
    };

    match resolved.denoted {
      Some(denoted) => {
        let _ = write!(out, "{indent}  {name} :: {}", describe(checker, denoted));
        if declaration.visibility != Visibility::Export && entry_kind.is_program_scope() {
          let _ = write!(out, " #scope_{}", declaration.visibility.name());
        }
        let _ = writeln!(out);
        // Only the declaration that names a type spells its members out.
        if checker.types().declared_name(denoted) == Some(declaration.name) {
          print_definition(checker, denoted, depth + 2, out);
        }
      }
      None => {
        let separator =
          if declaration.kind == DeclKind::Constant || declaration.kind == DeclKind::Procedure {
            "::"
          } else {
            ":"
          };
        let _ = writeln!(
          out,
          "{indent}  {name} {separator} {}",
          checker.type_name(resolved.value)
        );
      }
    }
  }

  for child in &entry_children {
    print_scope(checker, *child, depth + 1, out);
  }
}

/// A one-line description of a type declaration: what it is and how big.
fn describe(checker: &Checker<'_>, type_id: TypeId) -> String {
  let types = checker.types();
  let layout = types.layout(type_id);
  let head = match types.kind(type_id) {
    TypeKind::Struct(definition) => {
      if types.struct_info(*definition).is_union() {
        "union".to_string()
      } else {
        "struct".to_string()
      }
    }
    TypeKind::Enum(definition) => {
      let info = types.enum_info(*definition);
      let keyword = if info.is_flags() {
        "enum_flags"
      } else {
        "enum"
      };
      format!("{keyword} {}", checker.type_name(info.base))
    }
    TypeKind::Variant(definition) => {
      let info = types.variant_info(*definition);
      let keyword = if info.is_isa() {
        "#type,isa"
      } else {
        "#type,distinct"
      };
      format!("{keyword} {}", checker.type_name(info.base))
    }
    _ => checker.type_name(type_id),
  };
  match layout {
    Some(layout) => format!("{head} (size {}, align {})", layout.size, layout.alignment),
    None => format!("{head} (incomplete)"),
  }
}

fn print_definition(checker: &Checker<'_>, type_id: TypeId, depth: usize, out: &mut String) {
  let indent = "  ".repeat(depth);
  let types = checker.types();
  match types.kind(type_id) {
    TypeKind::Struct(definition) => {
      for member in &types.struct_info(*definition).members {
        let name = checker.interner().resolve_lossy(member.name);
        let _ = write!(out, "{indent}{name}: {}", checker.type_name(member.type_id));
        if member.flags.contains(MemberFlags::CONSTANT) {
          let _ = write!(out, " (constant)");
        } else {
          let _ = write!(out, " @{}", member.offset);
        }
        for (flag, text) in [
          (MemberFlags::USING, " using"),
          (MemberFlags::AS, " #as"),
          (MemberFlags::IMPORTED, " imported"),
        ] {
          if member.flags.contains(flag) {
            let _ = write!(out, "{text}");
          }
        }
        let _ = writeln!(out);
      }
    }
    TypeKind::Enum(definition) => {
      for member in &types.enum_info(*definition).members {
        let name = checker.interner().resolve_lossy(member.name);
        let _ = writeln!(out, "{indent}{name} = {}", member.value);
      }
    }
    _ => {}
  }
}

/// A one-line summary for `oj dump types` to close with, and for tests to
/// assert on.
pub fn summary(checker: &Checker<'_>) -> String {
  let typed = checker.finished().len();
  format!(
    "{typed} declarations typed, {} types",
    checker.types().len()
  )
}
