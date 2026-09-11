//! The listing `oj dump ir` prints.

use std::fmt::Write;

use oj_lexer::Interner;
use oj_types::Types;

use crate::ir::{
  Callee, Constant, ConvertKind, GlobalInit, Inst, ParameterKind, Procedure, Program, Terminator,
};

/// The whole program, or the one procedure whose name matches `only`.
pub fn print_ir(program: &Program, interner: &Interner, only: Option<&str>) -> String {
  let mut out = String::new();
  if only.is_none() {
    for global in &program.globals {
      let type_name = program.types.name(global.type_id, interner);
      let init = match &global.init {
        GlobalInit::Zero => String::from("zero"),
        GlobalInit::Constant(value) => constant_text(value),
        GlobalInit::Bytes(bytes) => format!("{} bytes kept from compile time", bytes.len()),
        GlobalInit::Image { relocations, .. } => {
          let procedures = relocations
            .iter()
            .filter(|(_, link)| matches!(link, crate::ir::ConstLink::Procedure(_)))
            .count();
          format!(
            "an image with {} pointers into itself and {procedures} into procedures",
            relocations.len() - procedures
          )
        }
      };
      let kind = if global.imported {
        "extern global"
      } else {
        "global"
      };
      let _ = writeln!(
        out,
        "{kind} {} : {type_name} = {init}  // size {}, align {}",
        global.symbol, global.size, global.alignment
      );
    }
    if !program.globals.is_empty() {
      out.push('\n');
    }
  }

  for procedure in &program.procedures {
    if only.is_some_and(|name| name != procedure.name && name != procedure.symbol) {
      continue;
    }
    print_procedure(&mut out, program, procedure, interner);
    out.push('\n');
  }
  out
}

fn print_procedure(
  out: &mut String,
  program: &Program,
  procedure: &Procedure,
  interner: &Interner,
) {
  let parameters: Vec<String> = procedure
    .abi
    .parameters
    .iter()
    .enumerate()
    .map(|(index, parameter)| {
      let type_name = program.types.name(parameter.type_id, interner);
      let prefix = match parameter.kind {
        ParameterKind::Value => "",
        ParameterKind::Pointer => "*",
        ParameterKind::ReturnPointer => "return *",
        ParameterKind::Context => "context *",
      };
      format!("%{index}: {prefix}{type_name}")
    })
    .collect();
  let returns = match procedure.abi.direct_return {
    Some(type_id) => format!(" -> {}", program.types.name(type_id, interner)),
    None => String::new(),
  };
  let _ = writeln!(
    out,
    "procedure {} ({}){returns} {}",
    procedure.symbol,
    parameters.join(", "),
    if procedure.has_body() {
      "{"
    } else {
      "#foreign;"
    }
  );
  if !procedure.has_body() {
    return;
  }

  for (index, local) in procedure.locals.iter().enumerate() {
    let type_name = program.types.name(local.type_id, interner);
    let _ = writeln!(
      out,
      "  local ${index} {} : {type_name}  // size {}, align {}",
      local.name, local.size, local.alignment
    );
  }

  for (index, block) in procedure.blocks.iter().enumerate() {
    let _ = writeln!(out, "  block{index}:");
    for instruction in &block.instructions {
      let _ = writeln!(
        out,
        "    {}",
        instruction_text(program, procedure, instruction, &program.types, interner)
      );
    }
    let _ = writeln!(out, "    {}", terminator_text(&block.terminator));
  }
  out.push_str("}\n");
}

fn terminator_text(terminator: &Terminator) -> String {
  match terminator {
    Terminator::Return(values) => {
      let values: Vec<String> = values.iter().map(|value| format!("%{}", value.0)).collect();
      format!("return {}", values.join(", "))
    }
    Terminator::Jump(block) => format!("jump block{}", block.0),
    Terminator::Branch {
      condition,
      then_block,
      else_block,
    } => format!(
      "branch %{} block{} block{}",
      condition.0, then_block.0, else_block.0
    ),
    Terminator::Unreachable => String::from("unreachable"),
  }
}

fn instruction_text(
  program: &Program,
  procedure: &Procedure,
  instruction: &Inst,
  types: &Types,
  interner: &Interner,
) -> String {
  let named = |value: crate::ir::ValueId| -> String {
    format!(
      "%{}: {}",
      value.0,
      types.name(procedure.value_type(value), interner)
    )
  };
  match instruction {
    Inst::Const { dest, value } => format!("{} = {}", named(*dest), constant_text(value)),
    Inst::LocalAddress { dest, local } => format!("{} = local ${}", named(*dest), local.0),
    Inst::GlobalAddress { dest, global } => format!(
      "{} = global {}",
      named(*dest),
      program.global(*global).symbol
    ),
    Inst::ProcedureAddress {
      dest,
      procedure: id,
    } => format!(
      "{} = procedure {}",
      named(*dest),
      program.procedure(*id).symbol
    ),
    Inst::Asm {
      text,
      inputs,
      outputs,
      clobbers,
    } => {
      let binding =
        |binding: &crate::ir::AsmBinding| format!("{} = %{}", binding.constraint, binding.value.0);
      let outputs: Vec<String> = outputs.iter().map(binding).collect();
      let inputs: Vec<String> = inputs.iter().map(binding).collect();
      format!(
        "asm {:?} out [{}] in [{}] clobbers [{}]",
        text,
        outputs.join(", "),
        inputs.join(", "),
        clobbers.join(", ")
      )
    }
    Inst::Load { dest, address } => format!("{} = load %{}", named(*dest), address.0),
    Inst::Store { address, value } => format!("store %{} <- %{}", address.0, value.0),
    Inst::Copy {
      destination,
      source,
      size,
      alignment,
    } => format!(
      "copy %{} <- %{} ({size} bytes, align {alignment})",
      destination.0, source.0
    ),
    Inst::Clear {
      destination,
      size,
      alignment,
    } => format!("clear %{} ({size} bytes, align {alignment})", destination.0),
    Inst::Offset { dest, base, offset } => {
      format!("{} = offset %{} + {offset}", named(*dest), base.0)
    }
    Inst::Index {
      dest,
      base,
      index,
      stride,
    } => format!(
      "{} = index %{} + %{} * {stride}",
      named(*dest),
      base.0,
      index.0
    ),
    Inst::Unary {
      dest,
      operator,
      operand,
    } => format!("{} = {operator:?} %{}", named(*dest), operand.0),
    Inst::Binary {
      dest,
      operator,
      left,
      right,
    } => format!(
      "{} = {} %{} %{}",
      named(*dest),
      operator.name(),
      left.0,
      right.0
    ),
    Inst::Convert {
      dest,
      kind,
      operand,
    } => format!("{} = {} %{}", named(*dest), convert_name(*kind), operand.0),
    Inst::AtomicCompareExchange {
      success,
      previous,
      address,
      expected,
      desired,
    } => format!(
      "{}, {} = cmpxchg %{} %{} %{}",
      named(*success),
      named(*previous),
      address.0,
      expected.0,
      desired.0
    ),
    Inst::Call {
      dest,
      callee,
      arguments,
      ..
    } => {
      let arguments: Vec<String> = arguments
        .iter()
        .map(|value| format!("%{}", value.0))
        .collect();
      let target = match callee {
        Callee::Direct(id) => program.procedure(*id).symbol.clone(),
        Callee::Indirect(value) => format!("%{}", value.0),
      };
      match dest {
        Some(dest) => format!("{} = call {target}({})", named(*dest), arguments.join(", ")),
        None => format!("call {target}({})", arguments.join(", ")),
      }
    }
  }
}

fn convert_name(kind: ConvertKind) -> &'static str {
  match kind {
    ConvertKind::IntegerZeroExtend => "zext",
    ConvertKind::IntegerSignExtend => "sext",
    ConvertKind::IntegerTruncate => "trunc",
    ConvertKind::SignedToFloat => "sitofp",
    ConvertKind::UnsignedToFloat => "uitofp",
    ConvertKind::FloatToSigned => "fptosi",
    ConvertKind::FloatToUnsigned => "fptoui",
    ConvertKind::FloatExtend => "fpext",
    ConvertKind::FloatTruncate => "fptrunc",
    ConvertKind::IntegerToPointer => "inttoptr",
    ConvertKind::PointerToInteger => "ptrtoint",
    ConvertKind::Bitcast => "bitcast",
  }
}

fn constant_text(value: &Constant) -> String {
  match value {
    Constant::Int(number) => number.to_string(),
    Constant::Float(number) => format!("{number:?}"),
    Constant::Bool(flag) => flag.to_string(),
    Constant::Null => String::from("null"),
    Constant::String(text) => format!("{:?}", String::from_utf8_lossy(text)),
    Constant::Bytes { bytes, links } => match links.is_empty() {
      true => format!("{} bytes of compile-time data", bytes.len()),
      false => format!(
        "{} bytes of compile-time data, {} pointers into it",
        bytes.len(),
        links.len()
      ),
    },
    Constant::Zero => String::from("zero"),
  }
}

/// A one-line count of what was lowered, for the end of a dump.
pub fn summary(program: &Program) -> String {
  let bodies = program
    .procedures
    .iter()
    .filter(|procedure| procedure.has_body())
    .count();
  let foreign = program.procedures.len() - bodies;
  format!(
    "{bodies} procedures lowered, {foreign} foreign, {} globals",
    program.globals.len()
  )
}
