use std::fmt::Write;

use oj_lexer::Interner;

use crate::kind::{ArrayKind, ProcedureFlags, ProcedureType, TypeId, TypeKind};
use crate::table::Types;

impl Types {
  /// A type's name in source-like syntax, the way the reference prints types
  /// (`*[17] **void`) and reports them in diagnostics (**L§3.10**).
  pub fn name(&self, id: TypeId, interner: &Interner) -> String {
    let mut out = String::new();
    self.write_name(id, interner, &mut out);
    out
  }

  fn write_name(&self, id: TypeId, interner: &Interner, out: &mut String) {
    match self.kind(id) {
      TypeKind::Void => out.push_str("void"),
      TypeKind::Bool => out.push_str("bool"),
      TypeKind::Integer(kind) => out.push_str(kind.name()),
      TypeKind::Float(kind) => out.push_str(kind.name()),
      TypeKind::String => out.push_str("string"),
      TypeKind::Any => out.push_str("Any"),
      TypeKind::Code => out.push_str("Code"),
      TypeKind::Type => out.push_str("Type"),
      TypeKind::V128 => out.push_str("v128"),
      TypeKind::Pointer(pointee) => {
        out.push('*');
        self.write_name(*pointee, interner, out);
      }
      TypeKind::Array { element, kind } => {
        match kind {
          ArrayKind::Fixed(count) => {
            let _ = write!(out, "[{count}] ");
          }
          ArrayKind::View => out.push_str("[] "),
          ArrayKind::Resizable => out.push_str("[..] "),
        }
        self.write_name(*element, interner, out);
      }
      TypeKind::Procedure(signature) => self.write_procedure_name(signature, interner, out),
      TypeKind::Struct(definition) => {
        let info = self.struct_info(*definition);
        match info.name {
          Some(name) => out.push_str(&interner.resolve_lossy(name)),
          None if info.is_union() => out.push_str("union"),
          None => out.push_str("struct"),
        }
      }
      TypeKind::Enum(definition) => match self.enum_info(*definition).name {
        Some(name) => out.push_str(&interner.resolve_lossy(name)),
        None => out.push_str("enum"),
      },
      TypeKind::Variant(definition) => {
        let info = self.variant_info(*definition);
        match info.name {
          Some(name) => out.push_str(&interner.resolve_lossy(name)),
          None => {
            out.push_str(if info.is_isa() {
              "#type,isa "
            } else {
              "#type,distinct "
            });
            self.write_name(info.base, interner, out);
          }
        }
      }
      TypeKind::Polymorph(definition) => {
        out.push('$');
        out.push_str(&interner.resolve_lossy(self.polymorph_info(*definition).name));
      }
      // An untyped literal that reaches a diagnostic never took a type, so the
      // reference names the type it would have defaulted to (**L§5.10**).
      TypeKind::UntypedInt => out.push_str("s64"),
      TypeKind::UntypedFloat(kind) => out.push_str(kind.name()),
      TypeKind::UntypedEnum => out.push_str("enum"),
      TypeKind::UntypedLiteral => out.push_str("untyped literal"),
      TypeKind::OverloadSet => out.push_str("overload set"),
      TypeKind::Unknown => out.push_str("unknown"),
    }
  }

  fn write_procedure_name(&self, signature: &ProcedureType, interner: &Interner, out: &mut String) {
    out.push('(');
    for (index, argument) in signature.arguments.iter().enumerate() {
      if index > 0 {
        out.push_str(", ");
      }
      if signature.vararg_index == Some(index as u32) {
        out.push_str("..");
      }
      self.write_name(*argument, interner, out);
    }
    out.push(')');

    match signature.returns.len() {
      0 => {}
      1 => {
        out.push_str(" -> ");
        self.write_name(signature.returns[0], interner, out);
      }
      _ => {
        out.push_str(" -> (");
        for (index, result) in signature.returns.iter().enumerate() {
          if index > 0 {
            out.push_str(", ");
          }
          self.write_name(*result, interner, out);
        }
        out.push(')');
      }
    }

    for (flag, text) in [
      (ProcedureFlags::IS_C_CALL, " #c_call"),
      (ProcedureFlags::HAS_NO_CONTEXT, " #no_context"),
      (ProcedureFlags::IS_SYMMETRIC, " #symmetric"),
    ] {
      if signature.flags.contains(flag) {
        out.push_str(text);
      }
    }
  }
}
