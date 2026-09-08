use oj_diag::SourceId;
use oj_lexer::Symbol;
use oj_scope::DeclId;
use oj_syntax::ast::{Argument, DeclarationFlags, NodeData, NodeFlags, NodeId};
use oj_types::TypeId;

use crate::checker::{Checker, Expr};
use crate::convert;
use crate::poly::InstanceId;

/// One argument a call site wrote, typed.
#[derive(Clone, Debug)]
pub(crate) struct CallArgument {
  pub name: Option<Symbol>,
  pub value: Expr,
  /// `..xs` hands a whole array to a `..T` slot rather than one of its
  /// elements (**L§7.3**).
  pub spread: bool,
}

impl CallArgument {
  pub fn positional(value: Expr) -> Self {
    Self {
      name: None,
      value,
      spread: false,
    }
  }
}

/// One parameter of a candidate, as a call site sees it (**L§7.3**).
#[derive(Clone, Debug)]
pub(crate) struct Parameter {
  pub name: Option<Symbol>,
  pub type_id: TypeId,
  pub has_default: bool,
  /// The default value written in the header, which a call site that leaves
  /// the parameter out evaluates in the header's own scope (**L§7.4**).
  pub default: Option<NodeId>,
}

/// A callable candidate: a procedure declaration, or a value of procedure type.
#[derive(Clone, Debug)]
pub(crate) struct Signature {
  pub parameters: Vec<Parameter>,
  pub returns: Vec<TypeId>,
  pub varargs: bool,
  pub polymorphic: bool,
  /// The declaration this candidate came from, so that a resolved call site
  /// can name the procedure it calls. A value of procedure type has none.
  pub decl: Option<DeclId>,
  /// Where the header was written, which is the scope its default values are
  /// evaluated in.
  pub header: Option<(SourceId, NodeId)>,
  pub type_id: TypeId,
  /// The specialization a polymorphic candidate produced for these arguments
  /// (**L§7.8**). A candidate that needed no instantiation has none.
  pub instance: Option<InstanceId>,
}

impl Signature {
  fn required(&self) -> usize {
    self
      .parameters
      .iter()
      .filter(|parameter| !parameter.has_default)
      .count()
  }
}

/// What a call site resolved to.
pub(crate) enum Resolved {
  One(Signature),
  /// Several candidates tie, or the front end cannot tell them apart yet.
  Ambiguous,
  /// No candidate accepts the arguments.
  None,
}

impl Checker<'_> {
  /// Picks the candidate a call resolves to, scoring each by the implicit
  /// conversions its arguments need (**L§7.5**).
  pub(crate) fn resolve_overload(
    &mut self,
    candidates: &[DeclId],
    arguments: &[CallArgument],
  ) -> Resolved {
    let mut scored: Vec<(u32, Signature)> = Vec::new();
    let mut any_signature = false;
    for candidate in candidates {
      let Some(signature) = self.signature_of(*candidate) else {
        // A candidate whose own type is not worked out yet cannot be ruled
        // out, so neither can any of the others.
        return Resolved::Ambiguous;
      };
      any_signature = true;
      if let Some(scored_candidate) = self.score_candidate(signature, arguments) {
        scored.push(scored_candidate);
      }
    }
    if !any_signature {
      return Resolved::Ambiguous;
    }

    let Some(best) = scored.iter().map(|(distance, _)| *distance).min() else {
      return Resolved::None;
    };
    let mut winners = scored.into_iter().filter(|(distance, _)| *distance == best);
    let winner = winners.next().expect("the minimum came from the list");
    if winners.next().is_some() {
      return Resolved::Ambiguous;
    }
    Resolved::One(winner.1)
  }

  pub(crate) fn accepts(&mut self, signature: &Signature, arguments: &[CallArgument]) -> bool {
    self.score(signature, arguments).is_some()
  }

  /// Scores one candidate, instantiating it first when it is polymorphic
  /// (**L§7.8**): what a call site is really matched against is the
  /// specialization its arguments produce, so a `$T` that cannot be solved is
  /// a candidate that does not accept them.
  fn score_candidate(
    &mut self,
    signature: Signature,
    arguments: &[CallArgument],
  ) -> Option<(u32, Signature)> {
    if !signature.polymorphic {
      let distance = self.score(&signature, arguments)?;
      return Some((distance, signature));
    }
    let specialized = self.specialize(&signature, arguments)?;
    let distance = self.score(&specialized, arguments)?;
    Some((distance.saturating_add(convert::POLYMORPH), specialized))
  }

  /// How far the arguments are from a candidate's parameters, or `None` when
  /// the candidate does not accept them at all.
  fn score(&mut self, signature: &Signature, arguments: &[CallArgument]) -> Option<u32> {
    let count = signature.parameters.len();
    let positional = arguments
      .iter()
      .take_while(|argument| argument.name.is_none())
      .count();
    if !signature.varargs && (arguments.len() > count || positional > count) {
      return None;
    }
    if arguments.len() < signature.required() && !signature.varargs {
      // A named argument may still fill a required slot further along.
      if arguments.iter().all(|argument| argument.name.is_none()) {
        return None;
      }
    }

    let mut total = 0u32;
    for (index, argument) in arguments.iter().enumerate() {
      let parameter = match argument.name {
        Some(name) => signature
          .parameters
          .iter()
          .find(|parameter| parameter.name == Some(name))?,
        None => match signature.parameters.get(index) {
          Some(parameter) => parameter,
          // Past the last declared parameter is the varargs slot, whose
          // distance is the first member's (**L§7.5**).
          None if signature.varargs => {
            total = total.saturating_add(convert::VARARGS);
            continue;
          }
          None => return None,
        },
      };
      // The varargs parameter is a `[] T`; an argument matches its element,
      // unless it was written `..xs`, which hands over the whole array.
      let target = if signature.varargs
        && Some(parameter) == signature.parameters.last()
        && !argument.spread
      {
        total = total.saturating_add(convert::VARARGS);
        match self.types().array_of(parameter.type_id) {
          Some((element, _)) => element,
          None => parameter.type_id,
        }
      } else {
        parameter.type_id
      };
      total = total.saturating_add(self.argument_distance(&argument.value, target)?);
    }
    if signature.polymorphic {
      total = total.saturating_add(convert::POLYMORPH);
    }
    Some(total)
  }

  /// An argument converts to its parameter the way an assignment does, plus
  /// the auto-dereference of a struct pointer (**L§7.6**).
  fn argument_distance(&mut self, value: &Expr, target: TypeId) -> Option<u32> {
    if let Some(distance) = self.implicit_conversion(value, target) {
      return Some(distance);
    }
    let pointee = self.types().pointee(value.type_id)?;
    self.types().struct_of(pointee)?;
    let mut dereferenced = value.clone();
    dereferenced.type_id = pointee;
    self
      .implicit_conversion(&dereferenced, target)
      .map(|distance| distance + convert::POINTER)
  }

  /// The parameters and returns of one candidate. A procedure declaration is
  /// read from its header, so that names and defaults are known; anything else
  /// callable contributes its procedure type alone.
  pub(crate) fn signature_of(&mut self, candidate: DeclId) -> Option<Signature> {
    let decl = self.program().tree().decl(candidate).clone();
    let resolved = self.decl_type(candidate);
    if self.types().is_unknown(resolved.value) {
      return None;
    }

    let header = decl.node.zip(decl.source).and_then(|(node, source)| {
      let NodeData::Declaration(declaration) = self.ast(source)?.data(node) else {
        return None;
      };
      let expression = declaration.expression?;
      matches!(
        self.ast(source)?.data(expression),
        NodeData::ProcedureHeader(_)
      )
      .then_some((source, expression))
    });

    let signature = self.types().procedure_of(resolved.value)?.clone();
    let polymorphic = signature
      .flags
      .contains(oj_types::ProcedureFlags::IS_POLYMORPHIC)
      || signature
        .arguments
        .iter()
        .any(|argument| self.is_polymorphic_type(*argument));

    let parameters = match header {
      Some((source, node)) => self.header_parameters(source, node, &signature.arguments),
      None => signature
        .arguments
        .iter()
        .map(|type_id| Parameter {
          name: None,
          type_id: *type_id,
          has_default: false,
          default: None,
        })
        .collect(),
    };

    Some(Signature {
      parameters,
      returns: signature.returns.clone(),
      varargs: signature.varargs,
      polymorphic,
      decl: Some(candidate),
      header,
      type_id: resolved.value,
      instance: None,
    })
  }

  /// A candidate built straight from a procedure type: a variable holding a
  /// procedure, or a member of one.
  pub(crate) fn signature_of_type(&self, type_id: TypeId) -> Option<Signature> {
    let signature = self.types().procedure_of(type_id)?;
    Some(Signature {
      parameters: signature
        .arguments
        .iter()
        .map(|argument| Parameter {
          name: None,
          type_id: *argument,
          has_default: false,
          default: None,
        })
        .collect(),
      returns: signature.returns.clone(),
      varargs: signature.varargs,
      polymorphic: signature
        .flags
        .contains(oj_types::ProcedureFlags::IS_POLYMORPHIC),
      decl: None,
      header: None,
      type_id,
      instance: None,
    })
  }

  pub(crate) fn header_parameters(
    &mut self,
    source: SourceId,
    header: NodeId,
    types: &[TypeId],
  ) -> Vec<Parameter> {
    let Some(ast) = self.ast(source) else {
      return Vec::new();
    };
    let NodeData::ProcedureHeader(payload) = ast.data(header) else {
      return Vec::new();
    };
    payload
      .arguments
      .iter()
      .enumerate()
      .map(|(index, parameter)| {
        let type_id = types.get(index).copied().unwrap_or(TypeId::UNKNOWN);
        let declaration = match ast.data(*parameter) {
          NodeData::Declaration(declaration) => Some(declaration),
          NodeData::Using(using) => match ast.data(using.expression) {
            NodeData::Declaration(declaration) => Some(declaration),
            _ => None,
          },
          _ => None,
        };
        let name = declaration
          .and_then(|declaration| declaration.name)
          .and_then(|node| self.ident_name(source, node));
        // `$T` and `$$T` parameters are baked at the call site, so they always
        // have a value to take (**L§7.8**).
        let has_default = declaration.is_some_and(|declaration| {
          declaration.expression.is_some()
            || declaration.flags.intersects(
              DeclarationFlags::AUTO_VALUE_BAKE | DeclarationFlags::AUTO_VALUE_BAKE_IS_REQUIRED,
            )
        });
        Parameter {
          name,
          type_id,
          has_default,
          default: declaration.and_then(|declaration| declaration.expression),
        }
      })
      .collect()
  }

  /// Whether a type mentions a polymorph variable, which makes the candidate
  /// an instantiation rather than a match (**L§7.8**).
  pub(crate) fn is_polymorphic_type(&self, type_id: TypeId) -> bool {
    match self.types().kind(type_id) {
      oj_types::TypeKind::Polymorph(_) | oj_types::TypeKind::Unknown => true,
      oj_types::TypeKind::Pointer(pointee) => self.is_polymorphic_type(*pointee),
      oj_types::TypeKind::Array { element, .. } => self.is_polymorphic_type(*element),
      _ => false,
    }
  }

  /// The named and positional arguments of a call, typed.
  pub(crate) fn call_arguments(
    &mut self,
    scope: oj_scope::ScopeId,
    source: SourceId,
    arguments: &[Argument],
  ) -> Vec<CallArgument> {
    arguments
      .iter()
      .map(|argument| CallArgument {
        name: argument.name.and_then(|node| self.ident_name(source, node)),
        value: self.expression_type(scope, source, argument.expression),
        spread: self.is_spread(source, argument.expression),
      })
      .collect()
  }

  /// Whether a call site wrote `..xs` for this argument (**L§7.3**).
  pub(crate) fn is_spread(&self, source: SourceId, node: NodeId) -> bool {
    self.ast(source).is_some_and(|ast| {
      ast
        .node(node)
        .flags
        .contains(NodeFlags::EXPRESSION_IS_SPREAD)
    })
  }
}

impl PartialEq for Parameter {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name && self.type_id == other.type_id
  }
}
