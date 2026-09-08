mod body;
mod expr;
mod run;

pub use run::{Run, lower_run};

use std::collections::{HashMap, VecDeque};

use oj_diag::{Diagnostic, SourceId, Span};
use oj_lexer::Symbol;
use oj_scope::{AstSource, DeclId, DeclKind, ScopeId};
use oj_sema::{CallPlan, Checker, Const, Expr, InstanceId, ProcedureBody, Value};
use oj_syntax::ast::{
  self, DeclarationFlags, ForFlags, IfFlags, LiteralValue, LoopControlType, NodeData, NodeId,
  OperatorType,
};
use oj_types::{ArrayKind, IntKind, TypeId, TypeKind};

use crate::ir::{
  Block, BlockId, Callee, Constant, ConvertKind, Global, GlobalId, GlobalInit, Inst, Library,
  Local, LocalId, ProcId, Procedure, ProcedureFlags, Program, Terminator, UnaryOp, ValueId,
};

/// A lowered expression. `indirect` means the value is the *address* of
/// something of `type_id` rather than the thing itself: every lvalue is
/// indirect, and so is any value too big for a register, which is how an
/// aggregate stays in memory from the front end to the back end.
#[derive(Clone, Copy, Debug)]
struct Val {
  id: ValueId,
  type_id: TypeId,
  indirect: bool,
}

/// Where a `break` and a `continue` go, and how many defer scopes they leave.
struct Loop {
  break_block: BlockId,
  continue_block: BlockId,
  depth: usize,
}

/// One macro expanded into the procedure being lowered (**L§7.13**): where a
/// `return` written in its body goes, and what it leaves its value in.
struct Expansion {
  exit: BlockId,
  /// The address and type of each of the macro's return values.
  results: Vec<(ValueId, TypeId)>,
  /// How deep the defer stack was when the expansion began, so a `return`
  /// inside the macro runs the macro's defers and no more.
  defers: usize,
}

/// The result of lowering a whole program.
pub struct Lowered {
  pub program: Program,
  pub diagnostics: Vec<Diagnostic>,
}

impl Lowered {
  pub fn has_errors(&self) -> bool {
    self.diagnostics.iter().any(Diagnostic::is_error)
  }
}

/// What a lowering is producing, which is what decides how a symbol is named
/// and who owns the program's data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
  /// The executable: symbols are as short as the linker allows.
  Executable,
  /// One `#run`: symbols have to be the same in every module of the
  /// compilation, since the JIT dylib holds them all (`docs/spec.md` §6.5).
  CompileTime,
}

/// A procedure the lowering has reached. A `#run` block is a procedure with no
/// declaration, so a node identifies it instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ProcKey {
  Decl(DeclId),
  Node(SourceId, NodeId),
  /// One instantiation of a polymorphic procedure: the header is shared, the
  /// code is not (**L§7.8**).
  Instance(InstanceId),
}

/// Lowers a typechecked program to IR, starting from its entry points and
/// following calls (**L§11.6**): a procedure nothing reachable calls is never
/// lowered, which is what keeps a module's unused polymorphic procedures out
/// of the back end.
pub fn lower(checker: &mut Checker) -> Lowered {
  let mut lowering = Lowering::new(checker, Mode::Executable);
  lowering.run();
  lowering.finish()
}

struct Lowering<'c, 'p> {
  checker: &'c mut Checker<'p>,
  mode: Mode,
  procedures: Vec<Procedure>,
  globals: Vec<Global>,
  procedure_ids: HashMap<ProcKey, ProcId>,
  global_ids: HashMap<DeclId, GlobalId>,
  queue: VecDeque<(ProcId, ProcKey)>,
  libraries: Vec<Library>,
  symbols: HashMap<String, u32>,
  diagnostics: Vec<Diagnostic>,
  entry: Option<ProcId>,
  /// The declaration the entry point calls, which takes the symbol the
  /// reference gives it rather than `main` — the C runtime calls a `main` that
  /// orangejuice generates (**C§13**).
  entry_decl: Option<DeclId>,
  context_type: TypeId,
  /// Global declarations whose initializer is not a constant, in the order
  /// they were reached; they run in a generated procedure before `main`.
  deferred_initializers: Vec<(GlobalId, DeclId)>,
  /// The `Type_Info` graph of every type the program asked about, and the
  /// global it is placed in (**L§17**).
  type_table: crate::typetable::TypeTable,
  type_table_global: Option<GlobalId>,
  type_table_symbol: String,

  // ------------------------------------------------- the procedure in hand ---
  blocks: Vec<Block>,
  locals: Vec<Local>,
  value_types: Vec<TypeId>,
  current: BlockId,
  /// The local a declaration became. A macro body's declarations are one
  /// local per expansion, so the instantiation is part of the key
  /// (**L§7.13**).
  local_of_decl: HashMap<(Option<InstanceId>, DeclId), LocalId>,
  loops: Vec<Loop>,
  /// The macros whose bodies are being spliced in right now, innermost last
  /// (**L§7.13**).
  expansions: Vec<Expansion>,
  /// The calls whose arguments are being evaluated, innermost last, and the
  /// macros whose bodies are being spliced in. `#caller_location` is the top
  /// of this (**L§7.13**).
  call_sites: Vec<(SourceId, NodeId)>,
  defers: Vec<Vec<(ScopeId, SourceId, NodeId)>>,
  returns: Vec<TypeId>,
  return_pointers: Vec<ValueId>,
  context_value: Option<ValueId>,
  body_scope: ScopeId,
  body_source: SourceId,
}

impl<'c, 'p> Lowering<'c, 'p> {
  fn new(checker: &'c mut Checker<'p>, mode: Mode) -> Self {
    let context_type = checker.context_type();
    Self {
      checker,
      mode,
      procedures: Vec::new(),
      globals: Vec::new(),
      procedure_ids: HashMap::new(),
      global_ids: HashMap::new(),
      queue: VecDeque::new(),
      libraries: Vec::new(),
      symbols: HashMap::new(),
      diagnostics: Vec::new(),
      entry: None,
      entry_decl: None,
      context_type,
      deferred_initializers: Vec::new(),
      type_table: crate::typetable::TypeTable::default(),
      type_table_global: None,
      type_table_symbol: String::from(TYPE_TABLE_SYMBOL),
      blocks: Vec::new(),
      locals: Vec::new(),
      value_types: Vec::new(),
      current: BlockId(0),
      local_of_decl: HashMap::new(),
      loops: Vec::new(),
      expansions: Vec::new(),
      call_sites: Vec::new(),
      defers: Vec::new(),
      returns: Vec::new(),
      return_pointers: Vec::new(),
      context_value: None,
      body_scope: ScopeId(0),
      body_source: SourceId(0),
    }
  }

  fn run(&mut self) {
    let Some(main) = self.find_entry() else {
      let span = Span::at(0);
      let source = self
        .checker
        .program()
        .units()
        .first()
        .map(|unit| unit.source)
        .unwrap_or(SourceId(0));
      self.diagnostics.push(Diagnostic::error(
        source,
        span,
        "No entry point was found. The program has no 'main'.",
      ));
      return;
    };

    self.entry_decl = Some(main);
    let entry = self.procedure_id(main);
    self.entry = Some(entry);
    self.drain_queue();
    // A global initializer may be the first thing to reach a procedure, so
    // whatever it named still has to be lowered.
    self.emit_global_initializers();
    self.drain_queue();
  }

  fn drain_queue(&mut self) {
    while let Some((id, key)) = self.queue.pop_front() {
      self.lower_procedure(id, key);
    }
  }

  /// The global the type table lives in, created the first time anything asks
  /// for a `Type_Info` (**L§17**). Its bytes are filled in at the end, once
  /// every type that was asked about is in the image.
  fn type_table_id(&mut self) -> GlobalId {
    if let Some(id) = self.type_table_global {
      return id;
    }
    let id = GlobalId(self.globals.len() as u32);
    self.globals.push(Global {
      symbol: self.type_table_symbol.clone(),
      name: String::from("__type_table"),
      type_id: TypeId::VOID,
      init: GlobalInit::Zero,
      size: 0,
      alignment: 8,
      decl: None,
      no_reset: false,
      external: false,
      imported: false,
    });
    self.type_table_global = Some(id);
    id
  }

  fn place_type_table(&mut self) {
    let Some(id) = self.type_table_global else {
      return;
    };
    let bytes: Box<[u8]> = Box::from(self.type_table.bytes());
    let relocations: Box<[(u64, u64)]> = Box::from(self.type_table.relocations());
    let global = &mut self.globals[id.0 as usize];
    global.size = bytes.len() as u64;
    global.init = GlobalInit::Image { bytes, relocations };
  }

  fn finish(mut self) -> Lowered {
    self.place_type_table();
    let Self {
      checker,
      procedures,
      globals,
      libraries,
      diagnostics,
      entry,
      context_type,
      ..
    } = self;
    let global_init = procedures
      .iter()
      .position(|procedure| procedure.symbol == GLOBAL_INIT_SYMBOL)
      .map(|index| ProcId(index as u32));
    Lowered {
      program: Program {
        types: checker.types().clone(),
        procedures,
        globals,
        entry,
        global_init,
        context_type,
        libraries,
      },
      diagnostics,
    }
  }

  /// The program's `main`: a procedure of that name in the root file's scope
  /// (**L§11.1**).
  fn find_entry(&mut self) -> Option<DeclId> {
    let program = self.checker.program();
    let name = program.interner().intern(b"main");
    let main_scope = program.main_scope();
    let root = program
      .units()
      .iter()
      .find(|unit| program.tree().scope(unit.scope).parent == Some(main_scope))
      .map(|unit| unit.scope)
      .unwrap_or(main_scope);
    let candidates = match program.tree().lookup(root, name) {
      oj_scope::Resolution::Found(candidates) => candidates,
      _ => Vec::new(),
    };
    candidates
      .into_iter()
      .find(|id| program.tree().decl(*id).kind == DeclKind::Procedure)
  }

  // --------------------------------------------------------------- symbols ---

  /// A linker-visible name for a declaration. Jai has no name mangling, so two
  /// overloads of one name are told apart by a suffix rather than by their
  /// types (**C§11**).
  fn unique_symbol(&mut self, base: &str) -> String {
    let count = self.symbols.entry(base.to_string()).or_insert(0);
    *count += 1;
    if *count == 1 {
      base.to_string()
    } else {
      format!("{base}.{}", *count - 1)
    }
  }

  fn text(&self, name: Symbol) -> String {
    self
      .checker
      .interner()
      .resolve_lossy(name)
      .into_owned()
      .chars()
      .map(|c| if c.is_ascii_graphic() { c } else { '_' })
      .collect()
  }

  /// The symbol a procedure or a global takes. Inside the executable a name is
  /// only made unique when it collides; in a compile-time module it also has
  /// to be the same in every other module of the compilation, so the
  /// declaration it came from is what makes it unique.
  fn symbol_for(
    &mut self,
    base: &str,
    decl: Option<DeclId>,
    instance: Option<InstanceId>,
  ) -> String {
    match self.mode {
      Mode::Executable => self.unique_symbol(base),
      Mode::CompileTime => {
        let decl = decl.map(|decl| decl.0).unwrap_or(u32::MAX);
        match instance {
          // Two instantiations of one header share a declaration, so the
          // specialization is what tells their symbols apart.
          Some(instance) => format!("{base}${decl}.{}", instance.0),
          None => format!("{base}${decl}"),
        }
      }
    }
  }

  /// The procedure one instantiation generates. Two call sites that solve the
  /// same constants share it; two that do not each get their own (**L§7.8**).
  fn instance_id(&mut self, instance: InstanceId) -> ProcId {
    if let Some(id) = self.procedure_ids.get(&ProcKey::Instance(instance)) {
      return *id;
    }
    let info = self.checker.instance_info(instance);
    let previous = self.checker.enter_instance(Some(instance));
    let id = self.declare_procedure(
      ProcKey::Instance(instance),
      info.decl,
      info.type_id,
      Some(instance),
    );
    self.checker.enter_instance(previous);
    id
  }

  fn procedure_id(&mut self, decl: DeclId) -> ProcId {
    if let Some(id) = self.procedure_ids.get(&ProcKey::Decl(decl)) {
      return *id;
    }
    let type_id = self.checker.decl_type(decl).value;
    self.declare_procedure(ProcKey::Decl(decl), Some(decl), type_id, None)
  }

  /// Registers a procedure and queues its body. `decl` is the declaration its
  /// name and `#foreign`/`#c_call` directives come from — an instantiation
  /// shares its polymorphic header's.
  fn declare_procedure(
    &mut self,
    key: ProcKey,
    decl: Option<DeclId>,
    type_id: TypeId,
    instance: Option<InstanceId>,
  ) -> ProcId {
    let id = ProcId(self.procedures.len() as u32);
    self.procedure_ids.insert(key, id);

    let name = match decl {
      Some(decl) => {
        let symbol = self.checker.program().tree().decl(decl).name;
        self.text(symbol)
      }
      None => String::from("procedure"),
    };
    let signature = self.checker.types().procedure_of(type_id).cloned();
    let body = match instance {
      Some(instance) => self.checker.instance_body(instance),
      None => decl.and_then(|decl| self.checker.procedure_body(decl)),
    };
    let span = decl
      .map(|decl| self.checker.program().tree().decl(decl).span)
      .unwrap_or_else(|| Span::at(0));
    let decl_source = decl.and_then(|decl| self.checker.program().tree().decl(decl).source);

    let mut flags = ProcedureFlags::empty();
    let mut library = None;
    let mut symbol = None;
    if let Some(body) = &body {
      // A body that lives in a library is called the C way and has no context
      // to be handed (**L§12.1**).
      if body.block.is_none() {
        flags |= ProcedureFlags::FOREIGN | ProcedureFlags::C_CALL | ProcedureFlags::NO_CONTEXT;
      }
      if body.flags.contains(ast::ProcedureFlags::C_CALL) {
        flags |= ProcedureFlags::C_CALL | ProcedureFlags::NO_CONTEXT;
      }
      if body
        .flags
        .contains(ast::ProcedureFlags::SYNTACTICALLY_MARKED_AS_NO_CONTEXT)
      {
        flags |= ProcedureFlags::NO_CONTEXT;
      }
      if let Some(export) = &body.export_name {
        flags |= ProcedureFlags::EXPORT;
        symbol = Some(String::from_utf8_lossy(export).into_owned());
      }
      if let Some(foreign) = &body.foreign_name {
        symbol = Some(String::from_utf8_lossy(foreign).into_owned());
      } else if flags.contains(ProcedureFlags::FOREIGN) {
        symbol = Some(name.clone());
      }
      library = body.library.map(|name| self.text(name));
      if let (Some(library), Some(decl)) = (&library, decl) {
        self.record_library(library.clone(), decl);
      }
    }

    // The C runtime's `main` is the one orangejuice generates; the program's
    // own entry point is the `__program_main` the reference names (**C§13**).
    let symbol = match symbol {
      Some(symbol) => symbol,
      None if decl.is_some() && self.entry_decl == decl => self.unique_symbol(PROGRAM_MAIN_SYMBOL),
      None => self.symbol_for(&name, decl, instance),
    };
    let (parameters, returns) = match &signature {
      Some(signature) => (signature.arguments.clone(), signature.returns.clone()),
      None => (Vec::new(), Vec::new()),
    };
    let abi = self.abi_of(type_id, flags);
    // Between two Jai procedures the convention is ours, so an aggregate goes
    // by pointer. A `#c_call` has to follow the platform's, where an aggregate
    // that fits in registers is passed in them (**L§7.11**) — orangejuice does
    // not classify those yet, so it says so rather than mis-calling.
    if flags.contains(ProcedureFlags::C_CALL)
      && abi
        .parameters
        .iter()
        .any(|parameter| parameter.kind != crate::ir::ParameterKind::Value)
      && let Some(source) = decl_source
    {
      self.report(
        source,
        span,
        format!(
          "'{name}' passes a value by the C convention that orangejuice cannot classify yet: \
           only registers-sized arguments and returns are supported (milestone M9)."
        ),
      );
    }

    self.procedures.push(Procedure {
      symbol,
      name,
      type_id,
      parameters,
      returns,
      flags,
      library,
      locals: Vec::new(),
      abi,
      blocks: Vec::new(),
      value_types: Vec::new(),
      entry: BlockId(0),
    });
    if !flags.contains(ProcedureFlags::FOREIGN) {
      self.queue.push_back((id, key));
    }
    id
  }

  /// The `#library`/`#library,system` a `#foreign` header named, resolved to
  /// what the linker needs (**L§12.2**).
  fn record_library(&mut self, name: String, user: DeclId) {
    if self.libraries.iter().any(|entry| entry.name == name) {
      return;
    }
    let program = self.checker.program();
    let scope = program.tree().decl(user).scope;
    let symbol = program.interner().intern(name.as_bytes());
    let mut resolved = None;
    if let oj_scope::Resolution::Found(candidates) = program.tree().lookup(scope, symbol) {
      for candidate in candidates {
        let decl = program.tree().decl(candidate).clone();
        let (Some(source), Some(node)) = (decl.source, decl.node) else {
          continue;
        };
        let Some(ast) = program.ast_of(source) else {
          continue;
        };
        let NodeData::Declaration(declaration) = ast.data(node) else {
          continue;
        };
        let Some(expression) = declaration.expression else {
          continue;
        };
        if let NodeData::DirectiveLibrary {
          name,
          library_flags,
        } = ast.data(expression)
        {
          let directory = program
            .units()
            .iter()
            .find(|unit| unit.source == source)
            .and_then(|unit| unit.path.parent().map(std::path::Path::to_path_buf));
          resolved = Some(Library {
            name: String::from_utf8_lossy(name.as_ref()).into_owned(),
            system: library_flags.contains(ast::LibraryFlags::IS_SYSTEM_LIBRARY),
            directory,
          });
        }
      }
    }
    self.libraries.push(resolved.unwrap_or(Library {
      name,
      system: true,
      directory: None,
    }));
  }

  fn global_id(&mut self, decl: DeclId) -> GlobalId {
    if let Some(id) = self.global_ids.get(&decl) {
      return *id;
    }
    let id = GlobalId(self.globals.len() as u32);
    self.global_ids.insert(decl, id);

    let info = self.checker.program().tree().decl(decl).clone();
    let name = self.text(info.name);
    let type_id = self.checker.decl_type(decl).value;
    let (size, alignment) = self.size_align(type_id);
    let imported = info.flags.contains(DeclarationFlags::ELSEWHERE);
    let external = imported || info.flags.contains(DeclarationFlags::PROGRAM_EXPORT);
    let symbol = if external {
      name.clone()
    } else {
      self.symbol_for(&name, Some(decl), None)
    };

    self.globals.push(Global {
      symbol,
      name,
      type_id,
      init: GlobalInit::Zero,
      size,
      alignment,
      decl: Some(decl),
      no_reset: info.flags.contains(DeclarationFlags::NO_RESET),
      external,
      imported,
    });

    if !imported {
      self.plan_global_initializer(id, decl);
    }
    id
  }

  /// A global starts out as data when its value folds, and is assigned by the
  /// generated initializer procedure when it does not.
  fn plan_global_initializer(&mut self, id: GlobalId, decl: DeclId) {
    let info = self.checker.program().tree().decl(decl).clone();
    let (Some(source), Some(node)) = (info.source, info.node) else {
      return;
    };
    let Some(NodeData::Declaration(declaration)) =
      self.checker.tree_of(source).map(|ast| ast.data(node))
    else {
      return;
    };
    if declaration
      .flags
      .contains(DeclarationFlags::IS_UNINITIALIZED)
    {
      return;
    }
    let Some(expression) = declaration.expression else {
      return;
    };
    let type_id = self.globals[id.0 as usize].type_id;
    let scope = self.checker.scope_for(source, expression, info.scope);
    let value = self.checker.expression(scope, source, expression);
    if let Some(constant) = value
      .constant
      .as_ref()
      .and_then(|value| self.fold(value, type_id))
    {
      self.globals[id.0 as usize].init = GlobalInit::Constant(constant);
      return;
    }
    self.deferred_initializers.push((id, decl));
  }

  fn fold(&mut self, value: &Const, target: TypeId) -> Option<Constant> {
    // A `.NAME` takes its value from the enum the context supplied
    // (**L§5.12**).
    if let Value::EnumName(name) = value.value {
      let definition = self.checker.types().enum_of(target)?;
      let member = self.checker.types().enum_info(definition).value_of(name)?;
      return Some(Constant::Int(i128::from(member)));
    }
    let converted = value.convert(self.checker.types(), target);
    let value = converted.as_ref().unwrap_or(value);
    match &value.value {
      Value::Int(number) => Some(Constant::Int(*number)),
      Value::Float(number) => Some(Constant::Float(*number)),
      Value::Bool(flag) => Some(Constant::Bool(*flag)),
      Value::Null => Some(Constant::Null),
      Value::String(text) => Some(Constant::String(text.clone())),
      Value::Bytes(bytes) => Some(Constant::Bytes(bytes.clone())),
      Value::Type(_) | Value::EnumName(_) => None,
    }
  }

  // --------------------------------------------------------------- helpers ---

  fn size_align(&mut self, type_id: TypeId) -> (u64, u64) {
    match self.checker.layout(type_id) {
      Some(layout) => (layout.size, layout.alignment.max(1)),
      None => (0, 1),
    }
  }

  /// Whether a value of this type lives in a register. Everything else stays
  /// in memory and is addressed rather than copied around (**L§3.14**).
  fn is_scalar(&mut self, type_id: TypeId) -> bool {
    let underlying = self.checker.types().underlying(type_id);
    matches!(
      self.checker.types().kind(underlying),
      TypeKind::Bool
        | TypeKind::Integer(_)
        | TypeKind::Float(_)
        | TypeKind::Enum(_)
        | TypeKind::Pointer(_)
        | TypeKind::Procedure(_)
        | TypeKind::Type
    )
  }

  fn pointer_to(&mut self, type_id: TypeId) -> TypeId {
    self.checker.types_table_mut().pointer_to(type_id)
  }

  fn value(&mut self, type_id: TypeId) -> ValueId {
    let id = ValueId(self.value_types.len() as u32);
    self.value_types.push(type_id);
    id
  }

  fn emit(&mut self, instruction: Inst) {
    self.blocks[self.current.0 as usize]
      .instructions
      .push(instruction);
  }

  fn new_block(&mut self) -> BlockId {
    let id = BlockId(self.blocks.len() as u32);
    self.blocks.push(Block::new());
    id
  }

  fn terminate(&mut self, terminator: Terminator) {
    let block = &mut self.blocks[self.current.0 as usize];
    if matches!(block.terminator, Terminator::Unreachable) {
      block.terminator = terminator;
    }
  }

  fn terminated(&self) -> bool {
    !matches!(
      self.blocks[self.current.0 as usize].terminator,
      Terminator::Unreachable
    )
  }

  /// The key a declaration's local is stored under: the same declaration is a
  /// different local in every expansion of the macro that wrote it
  /// (**L§7.13**).
  fn local_key(&self, decl: DeclId) -> (Option<InstanceId>, DeclId) {
    (self.checker.decl_instance(decl), decl)
  }

  fn new_local(&mut self, name: String, type_id: TypeId) -> LocalId {
    let (size, alignment) = self.size_align(type_id);
    let id = LocalId(self.locals.len() as u32);
    self.locals.push(Local {
      name,
      type_id,
      size,
      alignment,
    });
    id
  }

  fn local_address(&mut self, local: LocalId) -> ValueId {
    let type_id = self.locals[local.0 as usize].type_id;
    let pointer = self.pointer_to(type_id);
    let dest = self.value(pointer);
    self.emit(Inst::LocalAddress { dest, local });
    dest
  }

  fn constant(&mut self, value: Constant, type_id: TypeId) -> Val {
    let dest = self.value(type_id);
    self.emit(Inst::Const { dest, value });
    Val {
      id: dest,
      type_id,
      indirect: false,
    }
  }

  /// The address of a value, spilling it to a temporary when it only exists in
  /// a register.
  fn address_of(&mut self, value: Val) -> ValueId {
    if value.indirect {
      return value.id;
    }
    let local = self.new_local(String::from("temp"), value.type_id);
    let address = self.local_address(local);
    self.emit(Inst::Store {
      address,
      value: value.id,
    });
    address
  }

  /// The value itself, loading it out of memory when it is a place.
  fn scalar(&mut self, value: Val) -> ValueId {
    if !value.indirect {
      return value.id;
    }
    let dest = self.value(value.type_id);
    self.emit(Inst::Load {
      dest,
      address: value.id,
    });
    dest
  }

  /// Writes a value into storage, by register or by copy depending on size.
  fn store(&mut self, address: ValueId, value: Val) {
    if self.is_scalar(value.type_id) {
      let scalar = self.scalar(value);
      self.emit(Inst::Store {
        address,
        value: scalar,
      });
      return;
    }
    let source = self.address_of(value);
    let (size, alignment) = self.size_align(value.type_id);
    self.emit(Inst::Copy {
      destination: address,
      source,
      size,
      alignment,
    });
  }

  fn clear(&mut self, address: ValueId, type_id: TypeId) {
    let (size, alignment) = self.size_align(type_id);
    if size == 0 {
      return;
    }
    self.emit(Inst::Clear {
      destination: address,
      size,
      alignment,
    });
  }

  fn offset(&mut self, base: ValueId, offset: u64, type_id: TypeId) -> ValueId {
    let pointer = self.pointer_to(type_id);
    let dest = self.value(pointer);
    self.emit(Inst::Offset {
      dest,
      base,
      offset: offset as i64,
    });
    dest
  }

  fn error(&mut self, source: SourceId, node: NodeId, message: impl Into<String>) {
    let span = self
      .checker
      .tree_of(source)
      .map_or(Span::at(0), |ast| ast.node(node).span);
    self.report(source, span, message);
  }

  fn report(&mut self, source: SourceId, span: Span, message: impl Into<String>) {
    let message = message.into();
    if self
      .diagnostics
      .iter()
      .any(|entry| entry.source == source && entry.span == span && entry.message == message)
    {
      return;
    }
    self
      .diagnostics
      .push(Diagnostic::error(source, span, message));
  }

  /// What a construct the back end does not implement yet reports. The
  /// milestone is named so that a failure says which one owns it.
  fn unsupported(&mut self, source: SourceId, node: NodeId, what: &str, milestone: &str) {
    self.error(
      source,
      node,
      format!("Code generation for {what} is not implemented yet (milestone {milestone})."),
    );
  }
}

const GLOBAL_INIT_SYMBOL: &str = "__oj_global_init";

/// The symbol the `Type_Info` image takes (**L§17**).
const TYPE_TABLE_SYMBOL: &str = "__oj_type_table";

/// The symbol the program's own entry point gets, so that the generated `main`
/// the C runtime calls is free (**C§13**).
const PROGRAM_MAIN_SYMBOL: &str = "__program_main";
