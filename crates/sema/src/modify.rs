//! `#modify`: deciding a specialization after it has been solved (**L§7.8**).
//!
//! The block between a polymorphic header and its body runs at compile time
//! with the polymorph variables as *mutable values*: it may reject the
//! candidate, and it may change what the variables are before the body is read
//! against them. Deduplication happens afterwards, so two call sites that
//! `#modify` into the same constants share one instantiation.

use oj_diag::{SourceId, Span};
use oj_scope::{DeclId, ProcedureScopes, ScopeId};
use oj_syntax::ast::{NodeData, NodeId};
use oj_types::TypeId;

use crate::checker::Checker;
use crate::constants::{Const, Value};
use crate::poly::{Instance, InstanceId};

/// The prefix every `#modify` entry point's symbol takes.
pub const MODIFY_SYMBOL_PREFIX: &str = "__oj_modify_";

/// One `#modify` block, as the checker worked it out.
#[derive(Clone, Debug)]
pub struct ModifyRequest {
  pub source: SourceId,
  /// The scope the block was written in, which is the header's arguments.
  pub scope: ScopeId,
  pub block: NodeId,
  /// The instantiation whose scopes the block's names belong to. It binds
  /// nothing: inside `#modify` the variables are values, not constants.
  pub instance: InstanceId,
  /// The variables the block may read and reassign, in declaration order.
  pub variables: Vec<ModifyVariable>,
  /// The symbol the block's entry point takes, unique in the compilation.
  pub symbol: String,
  pub span: Span,
}

/// One polymorph variable, as `#modify` sees it.
#[derive(Clone, Debug)]
pub struct ModifyVariable {
  pub decl: DeclId,
  /// The type it has *inside* the block: a type variable is a `Type` there.
  pub type_id: TypeId,
  /// What solving decided, before the block ran.
  pub value: Const,
}

/// What running one `#modify` produced.
#[derive(Clone, Debug)]
pub enum ModifyOutcome {
  /// The candidate is accepted, with these values for its variables.
  Accepted(Vec<Const>),
  /// The candidate is rejected; the reason is printed if nothing matches.
  Rejected(String),
  /// The block could not be run. The engine has already reported why.
  Failed,
}

impl Checker<'_> {
  /// The type a `$x` variable's *value* has inside a `#modify` block
  /// (**L§7.8**).
  ///
  /// A type variable is an ordinary `Type` there: it is not constant, so
  /// `size_of(T)` and `y: T;` are not allowed. Everything else is a value of
  /// the type the header declared, which is not always the type the solved
  /// constant carries — a call site that wrote `.B` or `3` handed over an
  /// untyped constant, and a block comparing it to another member of its enum
  /// needs the enum.
  fn modify_variable_type(&mut self, decl: DeclId, value: &Const) -> TypeId {
    if matches!(value.value, Value::Type(_)) {
      return TypeId::TYPE;
    }
    if !self.types().is_untyped(value.type_id) {
      return value.type_id;
    }
    let declared = self.declared_parameter_type(decl);
    match self.types().is_unknown(declared) {
      true => self.harden(value.type_id),
      false => declared,
    }
  }

  /// The type a parameter was written with, read from the header rather than
  /// from anything a call site decided.
  fn declared_parameter_type(&mut self, decl: DeclId) -> TypeId {
    let info = self.program().tree().decl(decl);
    let (Some(source), Some(node), scope) = (info.source, info.node, info.scope) else {
      return TypeId::UNKNOWN;
    };
    let Some(NodeData::Declaration(declaration)) = self.ast(source).map(|ast| ast.data(node))
    else {
      return TypeId::UNKNOWN;
    };
    let Some(type_inst) = declaration.type_inst else {
      return TypeId::UNKNOWN;
    };
    self.type_from_node(scope, source, type_inst)
  }

  /// Runs the `#modify` blocks of a header that has just been solved
  /// (**L§7.8**). `None` rejects the candidate; `Some` gives the values the
  /// instantiation should be keyed and built with.
  pub(crate) fn run_modify(
    &mut self,
    source: SourceId,
    header: NodeId,
    scopes: ProcedureScopes,
    outer_scope: ScopeId,
    bindings: Vec<(DeclId, Const)>,
  ) -> Option<Vec<(DeclId, Const)>> {
    let blocks = self.modify_blocks(source, header);
    if blocks.is_empty() || self.compile_time.is_none() || bindings.is_empty() {
      return Some(bindings);
    }

    let mut bindings = bindings;
    for block in blocks {
      let variables: Vec<ModifyVariable> = bindings
        .iter()
        .map(|(decl, value)| ModifyVariable {
          decl: *decl,
          type_id: self.modify_variable_type(*decl, value),
          value: value.clone(),
        })
        .collect();
      // The block reads the variables as values, so the instantiation it runs
      // under binds nothing and only says what their types are.
      let instance = self.value_instance(source, header, scopes, outer_scope, &variables);
      if !self.modify_in_flight.insert(instance) {
        return Some(bindings);
      }
      let request = ModifyRequest {
        source,
        scope: scopes.arguments,
        block,
        instance,
        variables,
        symbol: format!("{MODIFY_SYMBOL_PREFIX}{}", self.next_run_index()),
        span: self
          .ast(source)
          .map_or(Span::at(0), |ast| ast.node(block).span),
      };
      let Some(engine) = self.compile_time.clone() else {
        return Some(bindings);
      };
      let outcome = engine.modify(self, &request);
      self.modify_in_flight.remove(&instance);
      match outcome {
        ModifyOutcome::Accepted(values) => {
          for (binding, value) in bindings.iter_mut().zip(values) {
            binding.1 = value;
          }
        }
        ModifyOutcome::Rejected(_) => return None,
        // A block that could not be run leaves the candidate as it was
        // solved, so one unfinished corner does not reject working code.
        ModifyOutcome::Failed => return Some(bindings),
      }
    }
    Some(bindings)
  }

  fn modify_blocks(&self, source: SourceId, header: NodeId) -> Vec<NodeId> {
    let Some(ast) = self.ast(source) else {
      return Vec::new();
    };
    let NodeData::ProcedureHeader(payload) = ast.data(header) else {
      return Vec::new();
    };
    payload
      .modify_directives
      .iter()
      .filter_map(|directive| match ast.data(*directive) {
        NodeData::DirectiveModify { block } => Some(*block),
        _ => None,
      })
      .collect()
  }

  /// An instantiation that gives the polymorph variables *types* but no
  /// values, which is what `#modify` sees them as (**L§7.8**).
  fn value_instance(
    &mut self,
    source: SourceId,
    header: NodeId,
    scopes: ProcedureScopes,
    outer_scope: ScopeId,
    variables: &[ModifyVariable],
  ) -> InstanceId {
    let id = InstanceId(self.instances.len() as u32);
    self.instances.push(Instance {
      decl: None,
      source,
      header,
      root: scopes.constants,
      outer_scope,
      bindings: Vec::new(),
      overrides: variables
        .iter()
        .map(|variable| (variable.decl, variable.type_id))
        .collect(),
      type_id: TypeId::UNKNOWN,
      parent: self.current_instance,
      expansion: None,
    });
    id
  }
}
