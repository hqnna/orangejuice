use oj_diag::SourceId;
use oj_lexer::Symbol;
use oj_scope::{DeclId, ScopeId};
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
  /// Where the argument was written, and the scope it was written in: a
  /// `Code` parameter is bound to that rather than to a value (**L§13.1**).
  pub written: Option<(SourceId, NodeId, oj_scope::ScopeId)>,
}

impl CallArgument {
  pub fn positional(value: Expr) -> Self {
    Self {
      name: None,
      value,
      spread: false,
      written: None,
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
  /// Where that default was written, when it is not the header's own — a
  /// `#bake_arguments` supplies one from its own site (**L§7.10**).
  pub default_source: Option<SourceId>,
}

/// A callable candidate: a procedure declaration, or a value of procedure type.
#[derive(Clone, Debug)]
pub(crate) struct Signature {
  pub parameters: Vec<Parameter>,
  pub returns: Vec<TypeId>,
  /// Which parameter is the `..T` one, when there is one (**L§7.3**).
  pub vararg_slot: Option<usize>,
  /// The parameters a `#bake_arguments` already gave values to: a call site
  /// neither fills them nor counts them (**L§7.10**).
  pub hidden: Vec<usize>,
  pub polymorphic: bool,
  /// The header was written `#expand`, so a call site expands it rather than
  /// calling it (**L§7.13**).
  pub is_macro: bool,
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
    let candidates = self.live_candidates(candidates);
    let mut scored: Vec<(u32, Signature)> = Vec::new();
    let mut any_signature = false;
    for candidate in &candidates {
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
    let winners: Vec<Signature> = scored
      .into_iter()
      .filter(|(distance, _)| *distance == best)
      .map(|(_, signature)| signature)
      .collect();
    // An overload set gathers outwards, so a tie may be between a declaration
    // written here and one an outer scope contributed — `compare_and_swap`
    // declared next to the call and the one Preload declares. The nearer one
    // wins, which is what shadowing amounts to; a tie *within* one scope is
    // the ambiguity the reference reports (**L§7.5**).
    let scope_of = |checker: &Self, signature: &Signature| {
      signature
        .decl
        .map(|decl| checker.program().tree().decl(decl).scope)
    };
    let nearest = scope_of(
      self,
      winners.first().expect("the minimum came from the list"),
    );
    if winners
      .iter()
      .skip(1)
      .any(|signature| scope_of(self, signature) == nearest)
    {
      return Resolved::Ambiguous;
    }
    Resolved::One(winners.into_iter().next().expect("checked above"))
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
    if !signature.polymorphic && !signature.is_macro {
      let distance = self.score(&signature, arguments)?;
      return Some((distance, signature));
    }
    // A macro goes through instantiation whatever its parameters are: the
    // expansion is what the call site names (**L§7.13**).
    let penalty = match signature.polymorphic {
      true => convert::POLYMORPH,
      false => 0,
    };
    let specialized = self.specialize(&signature, arguments)?;
    let distance = self.score(&specialized, arguments)?;
    Some((distance.saturating_add(penalty), specialized))
  }

  /// Whether a header was written `#expand` (**L§7.13**).
  pub(crate) fn is_macro_header(&self, source: SourceId, header: NodeId) -> bool {
    let Some(ast) = self.ast(source) else {
      return false;
    };
    match ast.data(header) {
      NodeData::ProcedureHeader(payload) => payload
        .procedure_flags
        .contains(oj_syntax::ast::ProcedureFlags::MACRO),
      _ => false,
    }
  }

  /// How far the arguments are from a candidate's parameters, or `None` when
  /// the candidate does not accept them at all.
  fn score(&mut self, signature: &Signature, arguments: &[CallArgument]) -> Option<u32> {
    let slots = self.argument_slots(signature, arguments)?;
    // Every parameter the call left out has to have a value of its own; a
    // `..T` slot is happy with nothing at all (**L§7.3**, **L§7.4**).
    for (index, parameter) in signature.parameters.iter().enumerate() {
      if parameter.has_default || Some(index) == signature.vararg_slot {
        continue;
      }
      if !slots.contains(&index) {
        return None;
      }
    }

    let mut total = 0u32;
    for (argument, index) in arguments.iter().zip(slots) {
      let parameter = signature.parameters.get(index)?;
      // The varargs parameter is a `[] T`; an argument matches its element,
      // unless it was written `..xs`, which hands over the whole array.
      let target = if Some(index) == signature.vararg_slot && !argument.spread {
        match self.types().array_of(parameter.type_id) {
          Some((element, _)) => element,
          None => parameter.type_id,
        }
      } else {
        parameter.type_id
      };
      // An argument to a macro's `Code` parameter is wrapped rather than
      // converted: whatever was written is the code (**L§7.13**).
      if signature.is_macro && target == TypeId::CODE {
        total = total.saturating_add(convert::LITERAL);
        continue;
      }
      total = total.saturating_add(self.argument_distance(&argument.value, target)?);
    }
    // A non-varargs candidate wins over a varargs one when both match
    // (**L§7.5**), which is what picks `array_add(array)` over
    // `array_add(array, to_append: ..T)` given nothing to append.
    if signature.vararg_slot.is_some() {
      total = total.saturating_add(convert::VARARGS);
    }
    if signature.polymorphic {
      total = total.saturating_add(convert::POLYMORPH);
    }
    Some(total)
  }

  /// Which parameter each written argument fills (**L§7.3**). Positional
  /// arguments take the declared parameters in order; once the `..T` slot is
  /// reached they all go into it, so a parameter written after one can only be
  /// filled by name.
  pub(crate) fn argument_slots(
    &self,
    signature: &Signature,
    arguments: &[CallArgument],
  ) -> Option<Vec<usize>> {
    let mut slots = Vec::with_capacity(arguments.len());
    // A slot a named argument claimed is not one a positional argument can
    // land in: `f(i = 5, s = "How", v = "are", "you")` puts `"you"` in `v`,
    // since `s` and `i` are spoken for (**L§7.3**).
    let claimed: Vec<usize> = arguments
      .iter()
      .filter_map(|argument| argument.name)
      .filter_map(|name| {
        signature
          .parameters
          .iter()
          .position(|parameter| parameter.name == Some(name))
      })
      .collect();
    let mut next = 0usize;
    for argument in arguments {
      while argument.name.is_none() && claimed.contains(&next) {
        next += 1;
      }
      let index = match argument.name {
        Some(name) => signature
          .parameters
          .iter()
          .position(|parameter| parameter.name == Some(name))?,
        // `..xs` fills the whole slot in one go, so what follows it is the
        // next declared parameter rather than another element (**L§7.3**).
        None if signature.hidden.contains(&next) => {
          // A baked parameter is not there as far as the call site is
          // concerned (**L§7.10**).
          while signature.hidden.contains(&next) {
            next += 1;
          }
          match next < signature.parameters.len() {
            true => {
              next += 1;
              next - 1
            }
            false => signature.vararg_slot?,
          }
        }
        None if Some(next) == signature.vararg_slot && argument.spread => {
          next += 1;
          next - 1
        }
        None if Some(next) == signature.vararg_slot => next,
        None if next < signature.parameters.len() => {
          next += 1;
          next - 1
        }
        None => signature.vararg_slot?,
      };
      slots.push(index);
    }
    Some(slots)
  }

  /// An argument converts to its parameter the way an assignment does, plus
  /// the auto-dereference of a struct pointer (**L§7.6**).
  fn argument_distance(&mut self, value: &Expr, target: TypeId) -> Option<u32> {
    if let Some(distance) = self.implicit_conversion(value, target) {
      return Some(distance);
    }
    // A name that stands for a whole overload set is narrowed by the parameter
    // it is being passed to (**L§7.5**): `map(fruits, to_upper)` means the
    // `to_upper` that takes a string, not `Basic`'s that takes a `u8`.
    if !value.overloads.is_empty() && self.types().procedure_of(target).is_some() {
      return self
        .narrowed_overloads(value)
        .into_iter()
        .filter_map(|candidate| self.argument_distance(&candidate, target))
        .min();
    }
    // A polymorphic procedure passed where a concrete one is wanted is
    // instantiated to it (**L§7.8**).
    if self.types().procedure_of(target).is_some()
      && (self.polymorphic_procedure(value).is_some()
        || self
          .types()
          .procedure_of(value.type_id)
          .is_some_and(|signature| {
            signature
              .flags
              .contains(oj_types::ProcedureFlags::IS_POLYMORPHIC)
          }))
    {
      return Some(convert::POLYMORPH);
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
    // `#bake_arguments f(y = 42)` is `f` with `y` already given (**L§7.10**).
    if let Some(baked) = self.baked_signature(candidate) {
      return Some(baked);
    }
    let decl = self.program().tree().decl(candidate);
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
    // A header the scope tree could not see was polymorphic — one whose only
    // variable is a parameter typed by a polymorphic struct *family* — has to
    // be marked now, since that is what keys what is written inside it by the
    // instantiation rather than by the program (**L§7.8**, **L§8.5**).
    if polymorphic && let Some((source, node)) = header {
      self.mark_body_uninstantiated(source, node);
    }
    let is_macro = header.is_some_and(|(source, node)| self.is_macro_header(source, node));

    // The defaults live on the *annotation*: `other: type_of(f);` is `f`'s type
    // written out, and a call through `other` takes `f`'s defaults with it —
    // which the reference calls strange and inconsistent, and does (**L§7.2**).
    let annotated = header.or_else(|| self.annotated_header(candidate));
    let parameters = match annotated {
      Some((source, node)) => {
        let mut parameters = self.header_parameters(source, node, &signature.arguments);
        // A default written on another declaration's header lives in that
        // file, not in whichever one this signature came from.
        if header.is_none() {
          for parameter in &mut parameters {
            if parameter.default.is_some() {
              parameter.default_source = Some(source);
            }
          }
        }
        parameters
      }
      None => signature
        .arguments
        .iter()
        .map(|type_id| Parameter {
          name: None,
          type_id: *type_id,
          has_default: false,
          default: None,
          default_source: None,
        })
        .collect(),
    };

    Some(Signature {
      parameters,
      returns: signature.returns.clone(),
      vararg_slot: signature.vararg_index.map(|index| index as usize),
      hidden: Vec::new(),
      polymorphic,
      is_macro,
      decl: Some(candidate),
      header,
      type_id: resolved.value,
      instance: None,
    })
  }

  /// The polymorphic procedure a name or a written header stands for, when it
  /// has no runtime value of its own (**L§7.8**).
  pub(crate) fn polymorphic_procedure(&mut self, value: &Expr) -> Option<DeclId> {
    let [only] = value.overloads[..] else {
      return None;
    };
    let signature = self.signature_of(only)?;
    signature.polymorphic.then_some(only)
  }

  /// The procedure header a declaration's *type slot* named, when it was
  /// written `x: type_of(f)`. The type is `f`'s either way; what this finds is
  /// the parameter names and defaults written on it (**L§7.2**).
  fn annotated_header(&mut self, candidate: DeclId) -> Option<(SourceId, NodeId)> {
    let info = self.program().tree().decl(candidate);
    let (source, node) = (info.source?, info.node?);
    let NodeData::Declaration(declaration) = self.ast(source)?.data(node) else {
      return None;
    };
    let NodeData::TypeInstantiation(inst) = self.ast(source)?.data(declaration.type_inst?) else {
      return None;
    };
    let NodeData::ExpressionQuery {
      query_kind: oj_syntax::ast::ExpressionQueryKind::TypeOf,
      expression_to_query,
    } = self.ast(source)?.data(inst.type_valued_expression?)
    else {
      return None;
    };
    let queried = *expression_to_query;
    let scope = self.scope_at(source, queried, info.scope);
    let named = self.expression_type(scope, source, queried);
    let [only] = named.overloads[..] else {
      return None;
    };
    let other = self.program().tree().decl(only);
    let (other_source, other_node) = (other.source?, other.node?);
    let NodeData::Declaration(other) = self.ast(other_source)?.data(other_node) else {
      return None;
    };
    let expression = other.expression?;
    matches!(
      self.ast(other_source)?.data(expression),
      NodeData::ProcedureHeader(_)
    )
    .then_some((other_source, expression))
  }

  /// Says that a header is polymorphic after all, so that everything written
  /// inside it is keyed by the instantiation (**L§7.8**).
  fn mark_body_uninstantiated(&mut self, source: SourceId, header: NodeId) {
    if let Some(scopes) = self.program().procedure_scopes(source, header) {
      self.program().mark_uninstantiated(scopes.constants);
    }
  }

  /// A candidate built from a header nobody declared: a quick lambda written
  /// where an argument goes (**L§7.9**).
  pub(crate) fn signature_of_header(
    &mut self,
    source: SourceId,
    header: NodeId,
    scope: ScopeId,
  ) -> Option<Signature> {
    let type_id = self.procedure_type(source, header, scope);
    let procedure = self.types().procedure_of(type_id)?.clone();
    let parameters = self.header_parameters(source, header, &procedure.arguments);
    let polymorphic = procedure
      .flags
      .contains(oj_types::ProcedureFlags::IS_POLYMORPHIC)
      || procedure
        .arguments
        .iter()
        .any(|argument| self.is_polymorphic_type(*argument));
    Some(Signature {
      parameters,
      returns: procedure.returns.clone(),
      vararg_slot: procedure.vararg_index.map(|index| index as usize),
      hidden: Vec::new(),
      polymorphic,
      is_macro: self.is_macro_header(source, header),
      decl: None,
      header: Some((source, header)),
      type_id,
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
          default_source: None,
        })
        .collect(),
      returns: signature.returns.clone(),
      vararg_slot: signature.vararg_index.map(|index| index as usize),
      hidden: Vec::new(),
      polymorphic: signature
        .flags
        .contains(oj_types::ProcedureFlags::IS_POLYMORPHIC),
      is_macro: false,
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
          default_source: None,
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
      // `f: (T) -> $S` is polymorphic even though nothing outside the
      // signature mentions `S`: unifying the argument's procedure type with
      // this one is what decides it (**L§7.8**).
      oj_types::TypeKind::Procedure(signature) => signature
        .arguments
        .iter()
        .chain(signature.returns.iter())
        .any(|member| self.is_polymorphic_type(*member)),
      // A parameter written as the polymorphic struct *family* rather than as
      // one of its instantiations — `tc: Typechecked` where `Typechecked ::
      // struct (T: Type)` — takes whichever instantiation the call passes,
      // which is what makes the header polymorphic (**L§7.8**, **L§8.5**).
      // So does one written as an instantiation over variables,
      // `holder: Holder($T, $N)`.
      _ => self.is_polymorph_family(type_id) || self.is_polymorph_instantiation(type_id),
    }
  }

  /// Each member of an overload set as a value of its own type (**L§7.5**).
  pub(crate) fn narrowed_overloads(&mut self, value: &Expr) -> Vec<Expr> {
    let candidates = value.overloads.clone();
    candidates
      .into_iter()
      .filter_map(|id| {
        let signature = self.signature_of(id)?;
        Some(Expr {
          type_id: signature.type_id,
          overloads: Vec::new(),
          ..value.clone()
        })
      })
      .collect()
  }

  /// Whether `type_id` is a baked polymorphic struct one of whose arguments is
  /// a polymorph variable — `Holder($T, $N)` — which makes it a pattern rather
  /// than a type (**L§8.5**).
  pub(crate) fn is_polymorph_instantiation(&self, type_id: TypeId) -> bool {
    let Some(definition) = self.types().struct_of(self.types().underlying(type_id)) else {
      return false;
    };
    let Some(instance) = self.struct_instance(definition) else {
      return false;
    };
    self.instance(instance).bindings.iter().any(|(_, value)| {
      value.as_type().is_some_and(|bound| {
        matches!(
          self.types().kind(bound),
          oj_types::TypeKind::Polymorph(_) | oj_types::TypeKind::Unknown
        )
      })
    })
  }

  /// Whether `type_id` is a polymorphic struct that nothing has baked yet.
  pub(crate) fn is_polymorph_family(&self, type_id: TypeId) -> bool {
    let Some(definition) = self.types().struct_of(self.types().underlying(type_id)) else {
      return false;
    };
    let info = self.types().struct_info(definition);
    info
      .nontextual_flags
      .contains(oj_types::StructNontextualFlags::POLYMORPHIC)
      && info.polymorph_source.is_none()
  }

  /// The named and positional arguments of a call, typed.
  pub(crate) fn call_arguments(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    arguments: &[Argument],
  ) -> Vec<CallArgument> {
    arguments
      .iter()
      .map(|argument| CallArgument {
        name: argument.name.and_then(|node| self.ident_name(source, node)),
        value: self.expression_type(scope, source, argument.expression),
        spread: self.is_spread(source, argument.expression),
        written: Some((
          source,
          argument.expression,
          self.scope_at(source, argument.expression, scope),
        )),
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
