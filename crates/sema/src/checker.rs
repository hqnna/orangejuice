use std::collections::{HashMap, HashSet};

use oj_diag::{Diagnostic, SourceId, Span};
use oj_lexer::{Interner, Symbol};
use oj_scope::{AstSource, DeclId, DeclKind, Program, Resolution, ScopeId};
use oj_syntax::ast::{Ast, NodeData, NodeId};
use oj_types::{EnumId, StructId, TypeId, Types};

use crate::constants::Const;
use crate::poly::{Expansion, Instance, InstanceId, InstanceKey};

/// What a declared name stands for once its type is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeclType {
  /// The type of the value the name has: the declared type of a variable, the
  /// procedure type of a procedure, `Type` for a type constant.
  pub value: TypeId,
  /// The type the name denotes, when it is a type (**L§3.10**).
  pub denoted: Option<TypeId>,
}

impl DeclType {
  pub const UNKNOWN: Self = Self {
    value: TypeId::UNKNOWN,
    denoted: None,
  };

  pub fn value(type_id: TypeId) -> Self {
    Self {
      value: type_id,
      denoted: None,
    }
  }

  /// A constant that names a type: its own type is `Type`, and it denotes the
  /// type it was declared as.
  pub fn type_name(denoted: TypeId) -> Self {
    Self {
      value: TypeId::TYPE,
      denoted: Some(denoted),
    }
  }
}

/// The result of typechecking an expression.
#[derive(Clone, Debug)]
pub struct Expr {
  pub type_id: TypeId,
  /// The type the expression denotes, when it is a type expression.
  pub denoted: Option<TypeId>,
  pub constant: Option<Const>,
  /// Whether the expression may be assigned to (**L§7.6**).
  pub lvalue: bool,
  /// The declarations a name stood for, when it named more than one: an
  /// overload set is not a type, so only a call site can narrow it
  /// (**L§7.5**).
  pub overloads: Vec<DeclId>,
  /// The instantiation the overloads were reached under, when a name was
  /// looked up inside one: a constant of a baked polymorphic struct is that
  /// specialization's, so a call site has to resolve it there (**L§8.5**).
  pub overload_instance: Option<InstanceId>,
  /// The expression is an explicit `cast(T)`. A bitwise operator whose left
  /// operand was cast keeps the cast's type rather than widening (**L§5.2**).
  pub explicitly_cast: bool,
  /// The expression is `xx e`, whose type is whatever asked for it
  /// (**L§5.6**).
  pub autocast: bool,
}

impl Expr {
  pub const UNKNOWN: Self = Self {
    type_id: TypeId::UNKNOWN,
    denoted: None,
    constant: None,
    lvalue: false,
    overloads: Vec::new(),
    overload_instance: None,
    explicitly_cast: false,
    autocast: false,
  };

  pub fn value(type_id: TypeId) -> Self {
    Self {
      type_id,
      denoted: None,
      constant: None,
      lvalue: false,
      overloads: Vec::new(),
      overload_instance: None,
      explicitly_cast: false,
      autocast: false,
    }
  }

  pub fn place(type_id: TypeId) -> Self {
    Self {
      lvalue: true,
      ..Self::value(type_id)
    }
  }

  pub fn constant(value: Const) -> Self {
    Self {
      type_id: value.type_id,
      denoted: value.as_type(),
      constant: Some(value),
      lvalue: false,
      overloads: Vec::new(),
      overload_instance: None,
      explicitly_cast: false,
      autocast: false,
    }
  }

  pub fn type_expression(denoted: TypeId) -> Self {
    Self {
      type_id: TypeId::TYPE,
      denoted: Some(denoted),
      constant: Some(Const::type_value(denoted)),
      lvalue: false,
      overloads: Vec::new(),
      overload_instance: None,
      explicitly_cast: false,
      autocast: false,
    }
  }

  pub fn is_unknown(&self) -> bool {
    self.type_id == TypeId::UNKNOWN
  }
}

/// One step of the path a `member.field[2].x = value;` statement walks from the
/// member it starts at (**L§8.1**). An index is folded where the statement was
/// written, since that is the only place its names mean anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PathStep {
  Member(Symbol),
  Element(u64),
}

/// One default a struct body wrote for one of its members (**L§8.1**): either
/// the `= expression` on the member's own declaration, or a later
/// `member.field = expression;` statement, which reaches a member of a member.
#[derive(Clone, Debug)]
pub(crate) struct MemberDefault {
  /// The index of the member the path starts at.
  pub member: usize,
  /// The steps after that member, empty when the statement named the member
  /// itself.
  pub path: Vec<PathStep>,
  /// Whether the default was written as a `member… = value;` statement rather
  /// than on the member's own declaration. Those are applied last, since they
  /// may reach past a member the declaration defaults have just filled in.
  pub from_statement: bool,
  pub source: SourceId,
  pub node: NodeId,
}

/// A struct body whose members nobody has resolved yet.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PendingBody {
  pub source: SourceId,
  pub node: NodeId,
  pub scope: ScopeId,
  /// The instantiation the body belongs to, for a polymorphic struct
  /// (**L§8.5**).
  pub instance: Option<InstanceId>,
}

enum State {
  Resolving,
  Done(DeclType),
}

/// The names the compiler resolves without a declaration, or looks up in
/// Preload by name (**L§3.1**, **L§17**).
struct Names {
  basic: Vec<(Symbol, TypeId)>,
  type_name: Symbol,
  os: Symbol,
  cpu: Symbol,
  is_cross_compiling: Symbol,
  machine_options_size: Symbol,
  temporary_storage_size: Symbol,
  stack_trace: Symbol,
  operating_system_tag: Symbol,
  cpu_tag: Symbol,
  count: Symbol,
  data: Symbol,
  allocated: Symbol,
  allocator: Symbol,
  loose: Symbol,
}

impl Names {
  fn new(interner: &Interner) -> Self {
    let basic = [
      (&b"s8"[..], TypeId::S8),
      (b"s16", TypeId::S16),
      (b"s32", TypeId::S32),
      (b"s64", TypeId::S64),
      (b"u8", TypeId::U8),
      (b"u16", TypeId::U16),
      (b"u32", TypeId::U32),
      (b"u64", TypeId::U64),
      (b"int", TypeId::INT),
      (b"float", TypeId::FLOAT),
      (b"float32", TypeId::FLOAT32),
      (b"float64", TypeId::FLOAT64),
      (b"string", TypeId::STRING),
      (b"void", TypeId::VOID),
      (b"bool", TypeId::BOOL),
      (b"Any", TypeId::ANY),
      (b"Code", TypeId::CODE),
      (b"v128", TypeId::V128),
    ]
    .into_iter()
    .map(|(name, type_id)| (interner.intern(name), type_id))
    .collect();

    Self {
      basic,
      type_name: interner.intern(b"Type"),
      os: interner.intern(b"OS"),
      cpu: interner.intern(b"CPU"),
      is_cross_compiling: interner.intern(b"IS_CROSS_COMPILING"),
      machine_options_size: interner.intern(b"MACHINE_OPTIONS_SIZE"),
      temporary_storage_size: interner.intern(b"TEMPORARY_STORAGE_SIZE"),
      stack_trace: interner.intern(b"_STACK_TRACE"),
      operating_system_tag: interner.intern(b"Operating_System_Tag"),
      cpu_tag: interner.intern(b"CPU_Tag"),
      count: interner.intern(b"count"),
      data: interner.intern(b"data"),
      allocated: interner.intern(b"allocated"),
      allocator: interner.intern(b"allocator"),
      loose: interner.intern(b"loose"),
    }
  }

  fn basic_type(&self, name: Symbol) -> Option<TypeId> {
    self
      .basic
      .iter()
      .find(|(candidate, _)| *candidate == name)
      .map(|(_, type_id)| *type_id)
  }
}

/// Gives every declaration of a resolved program a type, building the struct,
/// enum and variant definitions it names on the way (**L§3**, **L§8**,
/// **L§9**).
///
/// Resolution is demand-driven and memoized rather than scheduled: asking for a
/// declaration's type resolves whatever it depends on first, which makes data
/// scopes order-independent (**L§11.7**) without the work queue of
/// `docs/spec.md` §6.2. A declaration that is asked for while it is still being
/// resolved is a circular dependency; a nominal type publishes its (incomplete)
/// `TypeId` before its members are resolved, so a struct may still point at
/// itself.
pub struct Checker<'a> {
  program: &'a Program<'a>,
  interner: &'a Interner,
  types: Types,
  names: Names,
  /// The type of every declaration that has one, keyed by the instantiation it
  /// was resolved in: a parameter of a polymorphic procedure has one type per
  /// specialization, an ordinary declaration one for the whole program
  /// (**L§7.8**).
  states: HashMap<(Option<InstanceId>, DeclId), State>,
  finished: Vec<DeclId>,
  /// The scope each written name was resolved in. The program grows while the
  /// typechecker runs — an `#insert` splices statements in, and a `#import`
  /// inside one walks a whole module (**L§4.3**) — so this is topped up from
  /// the program rather than read once.
  scope_of_node: std::cell::RefCell<HashMap<(SourceId, NodeId), ScopeId>>,
  references_seen: std::cell::Cell<usize>,
  /// How many units this checker has read. The rest arrived after it started.
  units_seen: usize,
  struct_scopes: HashMap<StructId, ScopeId>,
  /// The instantiation a baked polymorphic struct came from (**L§8.5**), so
  /// that a nested declaration of its body — `Table.Entry` — is resolved with
  /// the arguments in place rather than under whatever instance is current.
  struct_instances: HashMap<StructId, InstanceId>,
  enum_scopes: HashMap<EnumId, ScopeId>,
  /// The aggregate body a member scope belongs to, so that asking for one
  /// member's type can build the whole definition.
  aggregate_owners: HashMap<ScopeId, (SourceId, NodeId)>,
  decl_constants: HashMap<(Option<InstanceId>, DeclId), Const>,
  /// The bodies of the structs whose members have not been resolved yet, and
  /// the ones being resolved right now, which is what makes a struct that
  /// contains itself by value an error rather than a hang.
  pending_bodies: HashMap<StructId, PendingBody>,
  completing: Vec<(StructId, SourceId, NodeId)>,
  /// The shared `declaration_properties` of each name in a compound
  /// declaration (**L§4.5**): `a, b: float;` declares two names off one type.
  compound_properties: HashMap<(SourceId, NodeId), (NodeId, usize)>,
  /// The `for` a loop variable belongs to, and whether it is the index
  /// (**L§6.5**).
  iterators: HashMap<DeclId, (SourceId, NodeId, bool)>,
  /// `#Context`, built once from `Context_Base` and the program's
  /// `#add_context` declarations (**L§10.2**).
  context: Option<TypeId>,
  /// The definition a struct or enum node produced, so that a type written
  /// twice in the same place is built once.
  aggregate_types: HashMap<(Option<InstanceId>, SourceId, NodeId), TypeId>,
  /// The default value of each struct member that was declared with one, by
  /// member index (**L§8.1**).
  member_defaults: HashMap<StructId, Vec<MemberDefault>>,
  stack: Vec<DeclId>,
  /// Every declaration by the node it was written at, built on first use: a
  /// back end walking a body finds the local a statement introduced this way.
  pub(crate) decl_nodes: Option<HashMap<(SourceId, NodeId), DeclId>>,
  /// How much of the tree that index covers. An `#insert` declares names into
  /// a tree the checker is already walking, so the index has to catch up
  /// (**L§13.2**).
  pub(crate) decl_nodes_indexed: u32,
  diagnostics: Vec<Diagnostic>,
  reported: HashSet<(SourceId, Span, String)>,
  depth: u32,
  /// Whoever can execute a `#run`, when anything can (**L§12.1**). A dump
  /// stage installs none, and a run whose expression folds needs none.
  pub(crate) compile_time: Option<std::rc::Rc<dyn crate::run::CompileTime>>,
  /// Where the `Code_*` export writes (**C§5.3**). A compilation with a
  /// metaprogram behind it shares this with `oj-meta`, so a `Code` value and
  /// the tree `compiler_get_nodes` answers with are the same storage.
  pub(crate) nodes: Option<std::rc::Rc<std::cell::RefCell<oj_meta::Nodes>>>,
  /// What a metaprogram asked to be left out of a type's record, as
  /// `Type_Info_Flags` numbers it (**C§3.3**).
  pub(crate) type_info_flags: HashMap<TypeId, u32>,
  /// Which compilation this one is, as far as that storage is concerned.
  pub(crate) nodes_generation: u32,
  /// Where each `Code` value the back end asked for ended up, so that one a
  /// `#run` hands back as a pointer is the piece of program it names again
  /// (**L§13.1**).
  code_addresses: HashMap<usize, (SourceId, NodeId, ScopeId)>,
  /// The answer each `#run` gave, so that a run written once executes once —
  /// once per instantiation for one written inside a polymorphic body, which
  /// is why the instance is part of the key (**L§12.1**).
  /// Which instantiation a backticked declaration's storage belongs to. A
  /// macro declares `` `x `` into the block it was expanded into, so what the
  /// caller reads afterwards is the expansion's local (**L§7.13**).
  pub(crate) backticked_owner: HashMap<DeclId, InstanceId>,
  pub(crate) runs: HashMap<(Option<InstanceId>, SourceId, NodeId), Expr>,
  /// The runs being worked out right now, which is what makes a `#run` that
  /// depends on itself an error rather than a hang.
  pub(crate) runs_in_flight: HashSet<(Option<InstanceId>, SourceId, NodeId)>,
  run_index: usize,
  /// How many static `#if`s nobody could decide the checker is inside. The
  /// reference never typechecks a rejected branch (**L§6.10**); orangejuice
  /// walks both when it cannot tell which one that is, so a `#run` or an
  /// `#assert` in there has to stay quiet (`docs/spec.md` §10).
  pub(crate) undecided_static_ifs: u32,
  /// Every instantiation of a polymorphic procedure and every macro expansion
  /// the program has reached (**L§7.8**, **L§7.13**).
  pub(crate) instances: Vec<Instance>,
  /// One instantiation per set of constants, program-wide (**L§7.8**).
  pub(crate) instance_cache: HashMap<InstanceKey, InstanceId>,
  /// The instantiation whose body is being checked, which is what gives the
  /// `$T`s on the way out of it their values.
  pub(crate) current_instance: Option<InstanceId>,
  /// Instantiations whose bodies nobody has checked yet.
  pub(crate) pending_instances: Vec<InstanceId>,
  pub(crate) checked_instances: HashSet<InstanceId>,
  /// The `#modify` blocks being run right now, which is what makes one that
  /// reaches its own header stop instead of looping (**L§7.8**).
  pub(crate) modify_in_flight: HashSet<InstanceId>,
  /// The call being resolved right now, which is where a macro expands
  /// (**L§7.13**). Overload resolution reaches down through operators too, so
  /// this is a single piece of state rather than a parameter of every step.
  pub(crate) call_site: Option<Expansion>,
  /// The `for_expansion` each `for` over a container reached (**L§7.14**).
  pub(crate) loop_expansions: crate::loops::LoopExpansions,
  /// The instantiation each polymorphic procedure written as an argument
  /// became (**L§7.8**).
  pub(crate) argument_instances: HashMap<(Option<InstanceId>, SourceId, NodeId), InstanceId>,
}

/// Deep enough for the module tree's nested types, shallow enough that a
/// pathological expression fails instead of overflowing the stack.
const MAX_DEPTH: u32 = 128;

/// The size of a thread's temporary storage, which the compiler defines for
/// Runtime_Support out of `Build_Options.temporary_storage_size` (**C§4**).
const TEMPORARY_STORAGE_SIZE: i64 = 32768;

impl<'a> Checker<'a> {
  pub fn new(program: &'a Program<'a>) -> Self {
    let interner = program.interner();
    let mut scope_of_node = HashMap::new();
    let mut references_seen = 0usize;
    for reference in program.references().iter() {
      scope_of_node
        .entry((reference.source, reference.node))
        .or_insert(reference.scope);
      references_seen += 1;
    }

    let aggregate_owners = program
      .aggregate_scopes()
      .into_iter()
      .map(|(source, node, scope)| (scope, (source, node)))
      .collect();

    let compound_properties = HashMap::new();
    let iterators = HashMap::new();

    let mut checker = Self {
      program,
      interner,
      nodes: None,
      nodes_generation: 0,
      code_addresses: HashMap::new(),
      type_info_flags: HashMap::new(),
      types: Types::new(),
      names: Names::new(interner),
      states: HashMap::new(),
      finished: Vec::new(),
      scope_of_node: std::cell::RefCell::new(scope_of_node),
      references_seen: std::cell::Cell::new(references_seen),
      struct_scopes: HashMap::new(),
      struct_instances: HashMap::new(),
      enum_scopes: HashMap::new(),
      aggregate_owners,
      decl_constants: HashMap::new(),
      pending_bodies: HashMap::new(),
      completing: Vec::new(),
      compound_properties,
      iterators,
      context: None,
      aggregate_types: HashMap::new(),
      member_defaults: HashMap::new(),
      stack: Vec::new(),
      decl_nodes: None,
      decl_nodes_indexed: 0,
      diagnostics: Vec::new(),
      reported: HashSet::new(),
      depth: 0,
      compile_time: None,
      backticked_owner: HashMap::new(),
      runs: HashMap::new(),
      runs_in_flight: HashSet::new(),
      run_index: 0,
      undecided_static_ifs: 0,
      instances: Vec::new(),
      instance_cache: HashMap::new(),
      current_instance: None,
      pending_instances: Vec::new(),
      checked_instances: HashSet::new(),
      modify_in_flight: HashSet::new(),
      call_site: None,
      loop_expansions: HashMap::new(),
      argument_instances: HashMap::new(),
      units_seen: 0,
    };
    checker.refresh_units();
    checker
  }

  /// Reads the files the program has gained since this checker last looked.
  /// The program grows while the typechecker runs: an `#insert` splices a
  /// parsed string in, and a `#import` inside one walks a whole module of the
  /// distribution (**L§4.3**). Everything derived from a unit's AST — which
  /// loop a nameless `it` belongs to, which compound declaration a name was
  /// written in — has to grow with it.
  fn refresh_units(&mut self) {
    let program = self.program;
    let count = program.unit_count();
    if count <= self.units_seen {
      return;
    }
    let units: Vec<(oj_diag::SourceId, std::sync::Arc<oj_syntax::Parsed>)> = program
      .units()
      .skip(self.units_seen)
      .map(|unit| (unit.source, std::sync::Arc::clone(&unit.parsed)))
      .collect();
    for (unit_source, parsed) in &units {
      let ast = &parsed.ast;
      let unit_source = *unit_source;
      for index in 0..ast.len() {
        let node = NodeId(index as u32);
        // `it` and `it_index` take their types from the loop they belong to
        // (**L§6.5**), which is only reachable from the loop's side.
        if matches!(ast.data(node), NodeData::For(_))
          && let Some(scope) = program.loop_scope(unit_source, node)
        {
          // A loop that did not name its variables declares them with no
          // identifier to be found through, so the loop's own scope — which
          // holds `it` then `it_index` and nothing else — is the key.
          let tree = program.tree();
          for (index, id) in tree
            .declarations(scope)
            .iter()
            .filter(|id| tree.decl(**id).kind == oj_scope::DeclKind::Iterator)
            .take(2)
            .enumerate()
          {
            self.iterators.insert(*id, (unit_source, node, index == 1));
          }
        }
        let NodeData::CompoundDeclaration(compound) = ast.data(node) else {
          continue;
        };
        let NodeData::CommaSeparatedArguments { arguments } =
          ast.data(compound.comma_separated_assignment)
        else {
          continue;
        };
        for (index, argument) in arguments.iter().enumerate() {
          self.compound_properties.insert(
            (unit_source, argument.node),
            (compound.declaration_properties, index),
          );
        }
      }
    }
    self.units_seen = count;
    self.aggregate_owners = program
      .aggregate_scopes()
      .into_iter()
      .map(|(source, node, scope)| (scope, (source, node)))
      .collect();
  }

  /// Installs the engine that executes `#run`s. Without one the checker still
  /// types a program: a run whose expression folds is answered from the front
  /// end, and one that does not fold stays `unknown` (`docs/spec.md` §6.5).
  pub fn set_compile_time(&mut self, engine: std::rc::Rc<dyn crate::run::CompileTime>) {
    self.compile_time = Some(engine);
  }

  /// Installs the storage the `Code_*` export writes into (**C§5.3**).
  /// Without one a `Code` value has no address to be, which is what a dump
  /// stage and a test without a driver behind it see.
  /// `generation` is the compilation this one is, which keys what it exports:
  /// the arena outlives many compilations, and each numbers its sources from
  /// zero.
  pub fn set_nodes(
    &mut self,
    nodes: std::rc::Rc<std::cell::RefCell<oj_meta::Nodes>>,
    generation: u32,
  ) {
    self.nodes = Some(nodes);
    self.nodes_generation = generation;
  }

  /// The address a `Code` value has at compile time: the `Code_Node` the
  /// program was written at, exported on demand (**L§13.1**, **C§5.3**).
  pub fn code_address(&mut self, source: SourceId, node: NodeId, scope: ScopeId) -> Option<usize> {
    let nodes = self.nodes.clone()?;
    let generation = self.nodes_generation;
    let address = {
      let mut nodes = nodes.borrow_mut();
      let mut exporter = crate::export::Exporter::new(self, &mut nodes, generation);
      exporter.tree(source, node).root as usize
    };
    self.code_addresses.insert(address, (source, node, scope));
    Some(address)
  }

  /// The piece of program a `Code` address names, for a value a `#run` handed
  /// back (**L§13.1**).
  pub fn code_at_address(&self, address: usize) -> Option<(SourceId, NodeId, ScopeId)> {
    self.code_addresses.get(&address).copied()
  }

  pub(crate) fn next_run_index(&mut self) -> usize {
    self.run_index += 1;
    self.run_index - 1
  }

  /// Types every declaration of the program, then lays out every struct whose
  /// members nothing asked for.
  pub fn check(&mut self) {
    self.expand_inserts();
    // An `#insert` declares names into a scope the tree already holds, so the
    // count grows while the pass runs (**L§13.2**).
    let mut index = 0;
    while index < self.program.tree().declaration_count() {
      self.decl_type(DeclId(index as u32));
      index += 1;
    }
    while let Some(definition) = self.pending_bodies.keys().copied().next() {
      self.complete_struct(definition);
    }
    self.check_bodies();
    self.check_file_runs();
    self.check_instances();
  }

  pub(crate) fn record_pending_body(
    &mut self,
    definition: StructId,
    source: SourceId,
    node: NodeId,
    scope: ScopeId,
  ) {
    // A polymorphic struct's members are laid out against the arguments of an
    // instantiation, so the body remembers which one it belongs to
    // (**L§8.5**).
    let instance = self.instance_of_scope(scope);
    self.pending_bodies.insert(
      definition,
      PendingBody {
        source,
        node,
        scope,
        instance,
      },
    );
  }

  /// Claims a struct body for resolution. A second claim while the first is
  /// still running is a struct that contains itself.
  pub(crate) fn take_pending_body(&mut self, definition: StructId) -> Option<PendingBody> {
    if let Some((_, source, node)) = self
      .completing
      .iter()
      .copied()
      .find(|(entry, _, _)| *entry == definition)
    {
      self.report_self_containment(source, node);
      return None;
    }
    let body = self.pending_bodies.remove(&definition)?;
    self.completing.push((definition, body.source, body.node));
    Some(body)
  }

  /// Whether a struct's members are being worked out right now, so that a
  /// lookup reaching for them would be the cycle rather than an answer.
  pub(crate) fn is_completing(&self, definition: StructId) -> bool {
    self
      .completing
      .iter()
      .any(|(entry, _, _)| *entry == definition)
  }

  /// Whether a scope holds the members of a struct that is being laid out
  /// right now. Asking one of them for its type is what that layout is doing,
  /// so a lookup that lands there has to give up instead.
  pub(crate) fn scope_is_completing(&self, scope: ScopeId) -> bool {
    self
      .completing
      .iter()
      .any(|(definition, _, _)| self.struct_scope(*definition) == Some(scope))
  }

  pub(crate) fn finish_pending_body(&mut self, definition: StructId) {
    if let Some(index) = self
      .completing
      .iter()
      .rposition(|(entry, _, _)| *entry == definition)
    {
      self.completing.remove(index);
    }
  }

  fn report_self_containment(&mut self, source: SourceId, node: NodeId) {
    let span = match self.ast(source) {
      Some(ast) => ast.node(node).span,
      None => return,
    };
    if self
      .diagnostics
      .iter()
      .any(|diagnostic| diagnostic.source == source && diagnostic.span == span)
    {
      return;
    }
    self.error(source, span, "The program contains circular dependencies.");
  }

  /// Resolves whatever a type's storage depends on: a struct's members, an
  /// array's elements, a variant's base.
  pub(crate) fn complete_type(&mut self, type_id: TypeId) {
    match *self.types.kind(type_id) {
      oj_types::TypeKind::Struct(definition) => self.complete_struct(definition),
      oj_types::TypeKind::Variant(definition) => {
        let base = self.types.variant_info(definition).base;
        self.complete_type(base);
      }
      // Only a fixed array stores its elements inline; a view or a resizable
      // array is two or four words whatever the element is (**L§3.3**).
      oj_types::TypeKind::Array {
        element,
        kind: oj_types::ArrayKind::Fixed(_),
      } => self.complete_type(element),
      _ => {}
    }
  }

  /// The storage of a type, resolving what it depends on first (**L§3.14**).
  pub(crate) fn layout_of(&mut self, type_id: TypeId) -> Option<oj_types::Layout> {
    self.complete_type(type_id);
    self.types.layout(type_id)
  }

  pub fn types(&self) -> &Types {
    &self.types
  }

  pub fn interner(&self) -> &'a Interner {
    self.interner
  }

  pub fn program(&self) -> &'a Program<'a> {
    self.program
  }

  pub fn diagnostics(&self) -> &[Diagnostic] {
    &self.diagnostics
  }

  pub fn has_errors(&self) -> bool {
    self.diagnostics.iter().any(Diagnostic::is_error)
  }

  /// The declarations that were typed, in the order they finished.
  pub fn finished(&self) -> &[DeclId] {
    &self.finished
  }

  pub fn resolved(&self, id: DeclId) -> Option<DeclType> {
    match self.states.get(&self.decl_key(id)) {
      Some(State::Done(type_id)) => Some(*type_id),
      _ => None,
    }
  }

  /// Which active instantiation a declaration's type belongs to. A declaration
  /// outside every polymorphic body has one type for the whole program; one
  /// inside the instantiation being checked has one per specialization
  /// (**L§7.8**).
  pub(crate) fn decl_key(&self, id: DeclId) -> (Option<InstanceId>, DeclId) {
    (
      self.instance_of_scope(self.program.tree().decl(id).scope),
      id,
    )
  }

  /// The innermost active instantiation that encloses a scope, or `None` when
  /// the scope is not inside one — which is the case for everything outside a
  /// polymorphic procedure or a macro.
  pub(crate) fn instance_of_scope(&self, scope: ScopeId) -> Option<InstanceId> {
    if !self.program.is_uninstantiated(scope) {
      return None;
    }
    let mut current = self.current_instance;
    while let Some(id) = current {
      let instance = &self.instances[id.0 as usize];
      if self.scope_encloses(instance.body_root(), scope) {
        return Some(id);
      }
      current = instance.parent;
    }
    None
  }

  pub(crate) fn scope_encloses(&self, outer: ScopeId, inner: ScopeId) -> bool {
    let tree = self.program.tree();
    let mut current = Some(inner);
    while let Some(id) = current {
      if id == outer {
        return true;
      }
      current = tree.parent(id);
    }
    false
  }

  pub(crate) fn instance(&self, id: InstanceId) -> &Instance {
    &self.instances[id.0 as usize]
  }

  pub fn struct_scope(&self, id: StructId) -> Option<ScopeId> {
    self.struct_scopes.get(&id).copied()
  }

  pub fn enum_scope(&self, id: EnumId) -> Option<ScopeId> {
    self.enum_scopes.get(&id).copied()
  }

  pub(crate) fn ast(&self, source: SourceId) -> Option<&'a Ast> {
    // The program owns its units for as long as the checker borrows it, so the
    // trees outlive every walk.
    let program: &'a Program<'a> = self.program;
    program.ast_of(source)
  }

  /// The scope an expression was written in. The scope tree recorded one for
  /// every identifier it had to resolve, which is exactly the set of nodes
  /// whose meaning depends on where they were written.
  pub(crate) fn scope_at(&self, source: SourceId, node: NodeId, fallback: ScopeId) -> ScopeId {
    self.scope_of(source, node).unwrap_or(fallback)
  }

  pub(crate) fn scope_of(&self, source: SourceId, node: NodeId) -> Option<ScopeId> {
    if let Some(scope) = self.scope_of_node.borrow().get(&(source, node)) {
      return Some(*scope);
    }
    self.refresh_scopes();
    self.scope_of_node.borrow().get(&(source, node)).copied()
  }

  /// Takes in the names the program has resolved since this checker last
  /// looked. A whole module can arrive mid-typecheck — `#insert`ing a string
  /// that `#import`s one is how `Metaprogram_Plugins` reaches a plugin — and
  /// every name in it was written after the snapshot `new` took (**L§4.3**).
  fn refresh_scopes(&self) {
    let seen = self.references_seen.get();
    let count = self.program.reference_count();
    if count <= seen {
      return;
    }
    let mut map = self.scope_of_node.borrow_mut();
    for reference in self.program.references_since(seen) {
      map
        .entry((reference.source, reference.node))
        .or_insert(reference.scope);
    }
    self.references_seen.set(count);
  }

  pub(crate) fn record_struct_scope(&mut self, id: StructId, scope: ScopeId) {
    self.struct_scopes.insert(id, scope);
  }

  pub(crate) fn record_struct_instance(&mut self, id: StructId, instance: InstanceId) {
    self.struct_instances.insert(id, instance);
  }

  pub(crate) fn struct_instance(&self, id: StructId) -> Option<InstanceId> {
    self.struct_instances.get(&id).copied()
  }

  pub(crate) fn record_enum_scope(&mut self, id: EnumId, scope: ScopeId) {
    self.enum_scopes.insert(id, scope);
  }

  /// The type a `struct`, `enum` or `#type` node built, ignoring any
  /// instantiation. Only the `$T` of a header is looked up this way: a
  /// polymorph variable belongs to the site that wrote it, not to a
  /// specialization (**L§7.8**).
  pub(crate) fn aggregate_type(&self, source: SourceId, node: NodeId) -> Option<TypeId> {
    self.aggregate_types.get(&(None, source, node)).copied()
  }

  pub(crate) fn record_aggregate_type(&mut self, source: SourceId, node: NodeId, type_id: TypeId) {
    self.aggregate_types.insert((None, source, node), type_id);
  }

  /// The same, for a definition written where an instantiation can change what
  /// it means: `Data :: struct { p: *T; }` inside a polymorphic body is one
  /// type per specialization (**L§7.8**, **L§3.13**).
  pub(crate) fn aggregate_type_in(
    &self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
  ) -> Option<TypeId> {
    self
      .aggregate_types
      .get(&(self.instance_of_scope(scope), source, node))
      .copied()
  }

  pub(crate) fn record_aggregate_type_in(
    &mut self,
    scope: ScopeId,
    source: SourceId,
    node: NodeId,
    type_id: TypeId,
  ) {
    let instance = self.instance_of_scope(scope);
    self
      .aggregate_types
      .insert((instance, source, node), type_id);
  }

  pub(crate) fn types_mut(&mut self) -> &mut Types {
    &mut self.types
  }

  /// The `declaration_properties` a name in a compound declaration shares with
  /// its siblings (**L§4.5**).
  /// Whether a scope on the chain out of `scope` still holds an unfinished
  /// name-inserting construct — a `using` of a value, an `#insert`, an
  /// undecidable `#if` — so that a name resolved there could still turn out to
  /// mean something else (**L§4.3**).
  pub(crate) fn scope_may_gain_names(&self, scope: ScopeId) -> bool {
    let tree = self.program.tree();
    let mut current = Some(scope);
    while let Some(id) = current {
      if tree.has_pending_providers(id) {
        return true;
      }
      current = tree.parent(id);
    }
    false
  }

  /// Whether a declaration's own type is being worked out right now, which is
  /// how a local that shadows an outer name is kept out of its own initializer
  /// (**L§6.13**).
  pub(crate) fn is_resolving(&self, id: DeclId) -> bool {
    matches!(self.states.get(&self.decl_key(id)), Some(State::Resolving))
  }

  pub(crate) fn compound_properties(
    &self,
    source: SourceId,
    node: NodeId,
  ) -> Option<(NodeId, usize)> {
    self.compound_properties.get(&(source, node)).copied()
  }

  /// The `struct`, `union` or `enum` body a member scope belongs to.
  pub(crate) fn aggregate_owner(&self, scope: ScopeId) -> Option<(SourceId, NodeId)> {
    self.aggregate_owners.get(&scope).copied()
  }

  pub(crate) fn record_member_defaults(
    &mut self,
    definition: StructId,
    defaults: Vec<MemberDefault>,
  ) {
    self.member_defaults.insert(definition, defaults);
  }

  pub(crate) fn member_defaults_of(&self, definition: StructId) -> &[MemberDefault] {
    self
      .member_defaults
      .get(&definition)
      .map(Vec::as_slice)
      .unwrap_or(&[])
  }

  pub(crate) fn record_constant(&mut self, id: DeclId, value: Const) {
    let key = self.decl_key(id);
    self.decl_constants.insert(key, value);
  }

  /// The value of a constant declaration, computed on first use (**L§5.11**).
  /// Whether a declaration is a `#module_parameters` parameter, which is a
  /// constant of the module scope rather than a variable (**L§11.3**) — a
  /// procedure's parameter is the same kind of declaration in a scope of its
  /// own, so where it was declared is what tells the two apart.
  pub(crate) fn is_module_parameter(&self, id: DeclId) -> bool {
    let decl = self.program.tree().decl(id);
    decl.kind == DeclKind::Parameter
      && self
        .program
        .tree()
        .scope_kind(decl.scope)
        .is_program_scope()
  }

  pub fn decl_constant(&mut self, id: DeclId) -> Option<Const> {
    if let Some(value) = self.bound_constant(id) {
      return Some(value);
    }
    self.decl_type(id);
    if let Some(value) = self.decl_constants.get(&self.decl_key(id)) {
      return Some(value.clone());
    }
    let decl = self.program.tree().decl(id);
    if !decl
      .flags
      .contains(oj_syntax::ast::DeclarationFlags::IS_CONSTANT)
      && !self.is_module_parameter(id)
    {
      return None;
    }
    let (source, node) = (decl.source?, decl.node?);
    let NodeData::Declaration(declaration) = self.ast(source)?.data(node) else {
      return None;
    };
    let expression = declaration.expression?;
    let declared = self.resolved(id).map(|resolved| resolved.value);
    let value = match declared.filter(|target| !self.types.is_unknown(*target)) {
      Some(target) => self.const_value_at(decl.scope, source, expression, target)?,
      None => self.const_value(decl.scope, source, expression)?,
    };
    // A constant with a type slot carries that type rather than the literal's
    // (**L§5.10** rule 2).
    let value = match declared {
      Some(target) if target != TypeId::TYPE && !self.types.is_unknown(target) => {
        value.convert(&self.types, target).unwrap_or(value)
      }
      _ => value,
    };
    let key = self.decl_key(id);
    self.decl_constants.insert(key, value.clone());
    Some(value)
  }

  pub(crate) fn interned(&self) -> &Interner {
    self.interner
  }

  pub(crate) fn count_name(&self) -> Symbol {
    self.names.count
  }

  pub(crate) fn data_name(&self) -> Symbol {
    self.names.data
  }

  pub(crate) fn allocated_name(&self) -> Symbol {
    self.names.allocated
  }

  pub(crate) fn allocator_name(&self) -> Symbol {
    self.names.allocator
  }

  pub(crate) fn loose_name(&self) -> Symbol {
    self.names.loose
  }

  pub(crate) fn enter(&mut self) -> bool {
    self.depth += 1;
    self.depth < MAX_DEPTH
  }

  pub(crate) fn leave(&mut self) {
    self.depth -= 1;
  }

  pub(crate) fn error(&mut self, source: SourceId, span: Span, message: impl Into<String>) {
    self.report(Diagnostic::error(source, span, message));
  }

  /// Reports something a compile-time engine found, which is how a `#run` that
  /// could not be built or that crashed reaches the same diagnostic stream as
  /// everything else.
  pub fn push_diagnostic(&mut self, diagnostic: Diagnostic) {
    self.report(diagnostic);
  }

  pub(crate) fn info(&mut self, source: SourceId, span: Span, message: impl Into<String>) {
    self.report(Diagnostic::info(source, span, message));
  }

  /// An expression is typed wherever it is asked about — once for the
  /// declaration it belongs to and again when its body is checked — so the
  /// same complaint reaches here more than once. It is reported once.
  fn report(&mut self, diagnostic: Diagnostic) {
    // The reference never typechecks the branch a `#if` rejected (**L§6.10**).
    // orangejuice walks both when it cannot decide which that is, so what it
    // finds in there is not known to be about real code (`docs/spec.md` §10).
    if self.undecided_static_ifs > 0 {
      return;
    }
    if self.reported.insert((
      diagnostic.source,
      diagnostic.span,
      diagnostic.message.clone(),
    )) {
      self.diagnostics.push(diagnostic);
    }
  }

  pub fn type_name(&self, type_id: TypeId) -> String {
    self.types.name(type_id, self.interner)
  }

  pub(crate) fn symbol_text(&self, name: Symbol) -> String {
    self.interner.resolve_lossy(name).into_owned()
  }

  // ----------------------------------------------------------- declarations ---

  /// The type of a declaration, resolving it on first use.
  pub fn decl_type(&mut self, id: DeclId) -> DeclType {
    // A constant the active instantiation bound is that value, whatever the
    // declaration says (**L§7.8**).
    if let Some(value) = self.bound_constant(id) {
      return match value.as_type() {
        Some(denoted) => DeclType::type_name(denoted),
        None => DeclType::value(value.type_id),
      };
    }
    // A parameter written as a bare polymorphic struct is whichever
    // instantiation the call passed (**L§7.8**).
    if let Some(overridden) = self.bound_parameter_type(id) {
      return DeclType::value(overridden);
    }
    let key = self.decl_key(id);
    match self.states.get(&key) {
      Some(State::Done(type_id)) => return *type_id,
      Some(State::Resolving) => {
        self.report_cycle(id);
        return DeclType::UNKNOWN;
      }
      None => {}
    }

    self.states.insert(key, State::Resolving);
    self.stack.push(id);
    let resolved = self.compute_decl_type(id);
    self.stack.pop();
    // A nominal type publishes itself while its members are still being built,
    // so only overwrite a slot that is still marked as being resolved.
    if matches!(self.states.get(&key), Some(State::Resolving)) {
      self.states.insert(key, State::Done(resolved));
      self.finished.push(id);
    }
    self.resolved(id).unwrap_or(resolved)
  }

  /// Publishes a declaration's type before the rest of it is built, so that a
  /// struct may refer to itself through a pointer (**L§8.1**).
  pub(crate) fn publish(&mut self, id: DeclId, resolved: DeclType) {
    let key = self.decl_key(id);
    self.states.insert(key, State::Done(resolved));
    self.finished.push(id);
  }

  fn report_cycle(&mut self, id: DeclId) {
    let tree = self.program.tree();
    let decl = tree.decl(id);
    let Some(source) = decl.source else { return };
    // Only the outermost report is useful: the cycle is one error, not one per
    // declaration on it.
    if self
      .diagnostics
      .iter()
      .any(|diagnostic| diagnostic.span == decl.span && diagnostic.source == source)
    {
      return;
    }
    self.error(
      source,
      decl.span,
      "The program contains circular dependencies.",
    );
    let cycle: Vec<DeclId> = match self.stack.iter().position(|entry| *entry == id) {
      Some(start) => self.stack[start..].to_vec(),
      None => Vec::new(),
    };
    for entry in cycle {
      let member = self.program.tree().decl(entry);
      if let Some(member_source) = member.source {
        let name = self.symbol_text(member.name);
        self.info(
          member_source,
          member.span,
          format!("Info: (declaration of '{name}')"),
        );
      }
    }
  }

  fn compute_decl_type(&mut self, id: DeclId) -> DeclType {
    // A declaration of a file that arrived mid-typecheck is one this checker
    // has read nothing about yet (**L§4.3**).
    self.refresh_units();
    let decl = self.program.tree().decl(id);
    // A loop variable takes its type from the loop rather than from a
    // declaration; one the program did not name has no node at all
    // (**L§6.5**).
    if let Some((source, loop_node, is_index)) = self.iterators.get(&id).copied() {
      return DeclType::value(self.iterator_type(decl.scope, source, loop_node, is_index));
    }
    match decl.kind {
      DeclKind::Builtin => self.builtin_type(id, decl.name),
      // A module name is not a value; member access through it goes to the
      // module's scope instead (**L§11.2**).
      DeclKind::Module(_) | DeclKind::Placeholder => DeclType::UNKNOWN,
      // A register an `#asm` block declared is a `__reg`, which is what lets a
      // macro take one as a parameter (**L§15**).
      DeclKind::AsmRegister => {
        let name = self.interned().intern(b"__reg");
        DeclType::value(self.preload_type(name))
      }
      _ => {
        let (Some(node), Some(source)) = (decl.node, decl.source) else {
          return DeclType::UNKNOWN;
        };
        let Some(ast) = self.ast(source) else {
          return DeclType::UNKNOWN;
        };
        match ast.data(node) {
          NodeData::Declaration(declaration) => {
            self.declaration_type(id, decl.scope, source, declaration)
          }
          // A name declared by a `$T` or by a compound declaration has an
          // identifier for its declaration node (**L§4.5**, **L§7.8**).
          NodeData::Ident(ident) => {
            if ident
              .flags
              .contains(oj_syntax::ast::IdentFlags::DEFINES_POLYMORPH_VARIABLE)
            {
              return DeclType::type_name(self.polymorph_type(source, node, ident.name));
            }
            let Some((properties, index)) = self.compound_properties(source, node) else {
              return DeclType::UNKNOWN;
            };
            let Some(NodeData::Declaration(declaration)) =
              self.ast(source).map(|ast| ast.data(properties))
            else {
              return DeclType::UNKNOWN;
            };
            self.compound_declaration_type(id, decl.scope, source, declaration, index)
          }
          _ => DeclType::UNKNOWN,
        }
      }
    }
  }

  /// `s8`…`Code` resolve directly to their types and cannot be shadowed;
  /// `Type`, `OS`, `CPU` and `IS_CROSS_COMPILING` are injected but ordinary
  /// (**L§3.1**, **L§17**).
  fn builtin_type(&mut self, id: DeclId, name: Symbol) -> DeclType {
    if let Some(type_id) = self.names.basic_type(name) {
      return DeclType::type_name(type_id);
    }
    if name == self.names.type_name {
      return DeclType::type_name(TypeId::TYPE);
    }
    if name == self.names.is_cross_compiling {
      self.record_constant(id, Const::bool(false));
      return DeclType::value(TypeId::BOOL);
    }
    if name == self.names.machine_options_size {
      self.record_constant(id, Const::new(TypeId::S64, crate::Value::Int(256)));
      return DeclType::value(TypeId::S64);
    }
    // Runtime_Support sizes the first thread's temporary storage by this, and
    // the compiler is what defines it, from `Build_Options` (**C§4**).
    if name == self.names.stack_trace {
      self.record_constant(id, Const::bool(true));
      return DeclType::value(TypeId::BOOL);
    }
    if name == self.names.temporary_storage_size {
      let size = i128::from(TEMPORARY_STORAGE_SIZE);
      self.record_constant(id, Const::new(TypeId::S64, crate::Value::Int(size)));
      return DeclType::value(TypeId::S64);
    }
    // `OS` and `CPU` are the target's, which is Linux x86-64 (`docs/spec.md`
    // §2); they are constants, so `#if OS == .LINUX` folds without running
    // anything (**L§5.11**).
    for (builtin, tag, member) in [
      (
        self.names.os,
        self.names.operating_system_tag,
        &b"LINUX"[..],
      ),
      (self.names.cpu, self.names.cpu_tag, b"X64"),
    ] {
      if name != builtin {
        continue;
      }
      let type_id = self.preload_type(tag);
      let member = self.interner.intern(member);
      if let Some(definition) = self.types.enum_of(type_id)
        && let Some(value) = self.types.enum_info(definition).value_of(member)
      {
        self.record_constant(
          id,
          Const::new(type_id, crate::Value::Int(i128::from(value))),
        );
      }
      return DeclType::value(type_id);
    }
    DeclType::UNKNOWN
  }

  /// The type of `context` (**L§10.1**), built on first use.
  pub fn context_type(&mut self) -> TypeId {
    if let Some(context) = self.context {
      return context;
    }
    // Recording it before it is built keeps a `#add_context` member whose type
    // mentions the context from recursing.
    self.context = Some(TypeId::UNKNOWN);
    let context = self.build_context_type();
    self.context = Some(context);
    context
  }

  /// A type declared in Preload, by name. Preload is not loaded when a single
  /// file is checked on its own, so a miss is not an error.
  pub(crate) fn preload_type(&mut self, name: Symbol) -> TypeId {
    let preload = self.program.preload_scope();
    let Resolution::Found(candidates) = self.program.tree().lookup(preload, name) else {
      return TypeId::UNKNOWN;
    };
    candidates
      .into_iter()
      .find_map(|id| self.decl_type(id).denoted)
      .unwrap_or(TypeId::UNKNOWN)
  }
}
