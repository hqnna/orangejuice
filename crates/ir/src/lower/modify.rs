//! Lowering one `#modify` block (**L§7.8**).
//!
//! The block becomes `void <symbol>(void *result)`, like a `#run`'s wrapper,
//! except that the polymorph variables go in as *locals* holding what solving
//! decided and come back out with whatever the block left in them. The result
//! is `{accept: s64, reason: string, variables…}`, each variable a machine
//! word, which is what the engine reads the answer out of.

use super::*;

use crate::ir::{Abi, AbiParameter, ParameterKind};

/// Where each part of a `#modify`'s answer sits in the result buffer.
pub const MODIFY_ACCEPT: u64 = 0;
pub const MODIFY_REASON: u64 = 8;
pub const MODIFY_VARIABLES: u64 = 24;
/// Every variable comes back as one machine word.
pub const MODIFY_VARIABLE_SIZE: u64 = 8;

/// What a `#modify` asks the back end to build.
#[derive(Clone, Debug)]
pub struct Modify {
  pub source: SourceId,
  pub scope: ScopeId,
  pub block: NodeId,
  pub instance: oj_sema::InstanceId,
  pub variables: Vec<oj_sema::ModifyVariable>,
  pub symbol: String,
  /// The globals an earlier run or modify of this compilation already gave
  /// storage, by symbol (`docs/spec.md` §6.5).
  pub already_initialized: std::collections::HashSet<String>,
}

/// The size of the buffer a `#modify` writes its answer into.
pub fn modify_result_size(variables: usize) -> usize {
  (MODIFY_VARIABLES + MODIFY_VARIABLE_SIZE * variables as u64) as usize
}

/// Lowers one `#modify` block into a program the JIT can compile.
pub fn lower_modify(checker: &mut Checker, modify: &Modify) -> Lowered {
  let mut lowering = Lowering::new(checker, Mode::CompileTime);
  lowering.already_initialized = modify.already_initialized.clone();
  lowering.lower_modify(modify);
  lowering.finish()
}

impl Lowering<'_, '_> {
  fn lower_modify(&mut self, modify: &Modify) {
    self.start_procedure();
    self.body_source = modify.source;
    self.body_scope = modify.scope;
    self.type_table_symbol = format!("__oj_type_table${}", modify.symbol);

    let result_pointer = self.pointer_to(TypeId::VOID);
    let id = ProcId(self.procedures.len() as u32);
    let type_id = self
      .checker
      .types_table_mut()
      .procedure(oj_types::ProcedureType::new(Vec::new(), Vec::new()));
    let abi = Abi {
      parameters: vec![AbiParameter {
        type_id: result_pointer,
        kind: ParameterKind::Value,

        class: None,
      }],
      direct_return: None,
      return_class: None,
      variadic: false,
    };
    self.procedures.push(Procedure {
      symbol: modify.symbol.clone(),
      name: modify.symbol.clone(),
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
      location: None,
    });
    self.entry = Some(id);

    let out = self.value(result_pointer);
    // A `#modify` runs through the same engine a `#run` does, and gets the same
    // `#Context` (**C§6.3**).
    let context_address = match self.checker.procedure_named("__jai_runtime_init") {
      Some(decl) => {
        let init = self.procedure_id(decl);
        self.call_runtime_init(init)
      }
      None => {
        let context = self.new_local(String::from("context"), self.context_type);
        let address = self.local_address(context);
        self.clear(address, self.context_type);
        address
      }
    };
    self.context_value = Some(context_address);

    // A `#modify` reads globals the same way a `#run` does, so the ones whose
    // initializers did not fold are assigned here too (**L§12.3**).
    let initializers_at = self.blocks[0].instructions.len();

    let previous = self.checker.enter_instance(Some(modify.instance));
    self.modify_body(modify, out);
    self.checker.enter_instance(previous);

    let procedure = &mut self.procedures[id.0 as usize];
    procedure.abi = abi;
    procedure.locals = std::mem::take(&mut self.locals);
    procedure.blocks = std::mem::take(&mut self.blocks);
    procedure.value_types = std::mem::take(&mut self.value_types);
    procedure.entry = BlockId(0);

    self.drain_queue();
    self.emit_global_initializers();
    self.drain_queue();
    if let Some(init) = self.global_initializer() {
      let signature = self.procedures[init.0 as usize].type_id;
      self.procedures[id.0 as usize].blocks[0]
        .instructions
        .insert(
          initializers_at,
          Inst::Call {
            dest: None,
            callee: Callee::Direct(init),
            signature,
            arguments: vec![context_address],
          },
        );
    }
  }

  fn modify_body(&mut self, modify: &Modify, out: ValueId) {
    let string = self.checker.preload_named_type("string");
    let string = match self.checker.types().is_unknown(string) {
      true => TypeId::STRING,
      false => string,
    };

    // The answer defaults to "accepted, no reason": a block that falls off its
    // end accepts (**L§7.8**).
    let accept = self.offset(out, MODIFY_ACCEPT, TypeId::S64);
    let yes = self.constant(Constant::Int(1), TypeId::S64);
    self.store(accept, yes);
    let reason = self.offset(out, MODIFY_REASON, string);
    self.clear(reason, string);

    // Each variable is a local the block may read and write.
    let mut locals = Vec::with_capacity(modify.variables.len());
    for variable in &modify.variables {
      let name = {
        let symbol = self.checker.program().tree().decl(variable.decl).name;
        self.text(symbol)
      };
      let local = self.new_local(name, variable.type_id);
      let address = self.local_address(local);
      match self.constant_value(&variable.value, variable.type_id) {
        Some(value) => self.store(address, value),
        None => self.clear(address, variable.type_id),
      }
      let key = self.local_key(variable.decl);
      self.local_of_decl.insert(key, local);
      locals.push(local);
    }

    // A `return accept, reason;` writes the answer and leaves the block, which
    // is exactly what an expansion frame does.
    let exit = self.new_block();
    self.expansions.push(crate::lower::Expansion {
      exit,
      results: vec![(accept, TypeId::S64), (reason, string)],
      defers: self.defers.len(),
    });
    self.defers.push(Vec::new());
    self.statement(modify.block);
    let scope = self.defers.pop().unwrap_or_default();
    if !self.terminated() {
      self.run_defers(&scope);
      self.terminate(Terminator::Jump(exit));
    }
    self.expansions.pop();
    self.current = exit;

    // Whatever the block left in the variables is what the instantiation is
    // built with.
    for (index, (variable, local)) in modify.variables.iter().zip(locals).enumerate() {
      let at = MODIFY_VARIABLES + MODIFY_VARIABLE_SIZE * index as u64;
      let slot = self.offset(out, at, variable.type_id);
      let address = self.local_address(local);
      let value = Val {
        id: address,
        type_id: variable.type_id,
        indirect: true,
      };
      self.store(slot, value);
    }
    self.terminate(Terminator::Return(Vec::new()));
  }
}
