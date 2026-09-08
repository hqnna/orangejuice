//! Lowering one `#run` (**L§12.1**).
//!
//! A run becomes a program of its own, reachable from a generated wrapper
//! rather than from `main`. The wrapper is `void <symbol>(void *result)`: it
//! hands the run a zeroed `#Context` and writes whatever the run produced
//! through the pointer, so the compiler reads the answer back without having
//! to know the return convention the run's own type would have used.

use super::*;

use crate::ir::{Abi, AbiParameter, ParameterKind};

/// What a `#run` asks the back end to build.
#[derive(Clone, Debug)]
pub struct Run {
  pub source: SourceId,
  pub scope: ScopeId,
  /// The `ProcedureHeader` the parser wraps a run's body in (**L§12.1**).
  pub header: NodeId,
  /// The block that header carries.
  pub block: NodeId,
  /// The type the run produces, `void` when it produces nothing.
  pub result: TypeId,
  /// The single expression a `#run expr` evaluates. A run whose body is a
  /// block of statements has none, and returns through its header instead.
  pub value: Option<NodeId>,
  /// The symbol the JIT looks the wrapper up under.
  pub symbol: String,
}

/// Lowers one `#run` into a program the JIT can compile.
pub fn lower_run(checker: &mut Checker, run: &Run) -> Lowered {
  let mut lowering = Lowering::new(checker, Mode::CompileTime);
  lowering.lower_run(run);
  lowering.finish()
}

impl Lowering<'_, '_> {
  fn lower_run(&mut self, run: &Run) {
    self.start_procedure();
    self.body_source = run.source;
    self.body_scope = run.scope;
    // Every run's module is added to the same dylib, so its type table needs a
    // symbol of its own (`docs/spec.md` §6.5).
    self.type_table_symbol = format!("__oj_type_table${}", run.symbol);

    let result_pointer = self.pointer_to(run.result);
    let id = ProcId(self.procedures.len() as u32);
    let type_id = self
      .checker
      .procedure_type_at(run.source, run.header, run.scope);
    let abi = Abi {
      parameters: vec![AbiParameter {
        type_id: result_pointer,
        kind: ParameterKind::Value,
      }],
      direct_return: None,
    };
    self.procedures.push(Procedure {
      symbol: run.symbol.clone(),
      name: run.symbol.clone(),
      type_id,
      parameters: Vec::new(),
      returns: Vec::new(),
      flags: ProcedureFlags::COMPILER_GENERATED | ProcedureFlags::NO_CONTEXT,
      library: None,
      abi: abi.clone(),
      locals: Vec::new(),
      blocks: Vec::new(),
      value_types: Vec::new(),
      entry: BlockId(0),
    });
    self.entry = Some(id);

    // The result pointer is the wrapper's one machine parameter, so it has to
    // be its first value.
    let out = self.value(result_pointer);

    // Compile-time code runs with a zeroed context, the same one the generated
    // entry point hands `main` (`docs/spec.md` §10).
    let context = self.new_local(String::from("context"), self.context_type);
    let context_address = self.local_address(context);
    self.clear(context_address, self.context_type);
    self.context_value = Some(context_address);

    match run.value {
      Some(expression) => self.run_expression(run, out, expression),
      None => self.run_block(run, out, context_address),
    }
    self.terminate(Terminator::Return(Vec::new()));

    let procedure = &mut self.procedures[id.0 as usize];
    procedure.abi = abi;
    procedure.locals = std::mem::take(&mut self.locals);
    procedure.blocks = std::mem::take(&mut self.blocks);
    procedure.value_types = std::mem::take(&mut self.value_types);
    procedure.entry = BlockId(0);

    self.drain_queue();
  }

  /// `#run expr`: the wrapper evaluates the expression itself, since a single
  /// expression has no `return` for a body to carry.
  fn run_expression(&mut self, run: &Run, out: ValueId, expression: NodeId) {
    let scope = self
      .checker
      .scope_for(run.source, expression, self.body_scope);
    let Some(value) = self.expression(scope, run.source, expression, Some(run.result)) else {
      return;
    };
    if run.result != TypeId::VOID {
      self.store(out, value);
    }
  }

  /// `#run { … }`: the block is a procedure of its own, so that a `return`
  /// inside it means what it says, and the wrapper calls it.
  fn run_block(&mut self, run: &Run, out: ValueId, context: ValueId) {
    let body = self.run_body_id(run);
    let abi = self.procedures[body.0 as usize].abi.clone();
    let signature = self.procedures[body.0 as usize].type_id;

    let mut arguments = Vec::with_capacity(abi.parameters.len());
    let mut returns_seen = 0usize;
    for parameter in &abi.parameters {
      match parameter.kind {
        ParameterKind::ReturnPointer => {
          returns_seen += 1;
          if returns_seen == 1 {
            arguments.push(out);
          } else {
            let local = self.new_local(String::from("discarded"), parameter.type_id);
            let address = self.local_address(local);
            arguments.push(address);
          }
        }
        ParameterKind::Context => arguments.push(context),
        ParameterKind::Value | ParameterKind::Pointer => {
          self.error(
            run.source,
            run.header,
            "A '#run' block cannot take arguments.",
          );
          return;
        }
      }
    }

    let dest = abi.direct_return.map(|type_id| self.value(type_id));
    self.emit(Inst::Call {
      dest,
      callee: Callee::Direct(body),
      signature,
      arguments,
    });
    if let Some(dest) = dest {
      self.emit(Inst::Store {
        address: out,
        value: dest,
      });
    }
  }

  /// The procedure a `#run` block's header stands for, queued for lowering the
  /// way a called declaration would be.
  fn run_body_id(&mut self, run: &Run) -> ProcId {
    let key = ProcKey::Node(run.source, run.header);
    if let Some(id) = self.procedure_ids.get(&key) {
      return *id;
    }
    let id = ProcId(self.procedures.len() as u32);
    self.procedure_ids.insert(key, id);

    let type_id = self
      .checker
      .procedure_type_at(run.source, run.header, run.scope);
    let signature = self.checker.types().procedure_of(type_id).cloned();
    let (parameters, returns) = match &signature {
      Some(signature) => (signature.arguments.clone(), signature.returns.clone()),
      None => (Vec::new(), Vec::new()),
    };
    let flags = ProcedureFlags::COMPILER_GENERATED;
    let abi = self.abi_of(type_id, flags);
    let name = format!("{}$body", run.symbol);
    self.procedures.push(Procedure {
      symbol: name.clone(),
      name,
      type_id,
      parameters,
      returns,
      flags,
      library: None,
      abi,
      locals: Vec::new(),
      blocks: Vec::new(),
      value_types: Vec::new(),
      entry: BlockId(0),
    });
    self.queue.push_back((id, key));
    id
  }
}
