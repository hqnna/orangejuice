mod asm;
mod body;
mod expr;
mod modify;
mod run;
mod trace;

pub use modify::{
  MODIFY_ACCEPT, MODIFY_REASON, MODIFY_VARIABLE_SIZE, MODIFY_VARIABLES, Modify, lower_modify,
  modify_result_size,
};
pub use run::{Run, lower_run};

pub use Options as LowerOptions;

use std::collections::{HashMap, VecDeque};

use oj_diag::{Diagnostic, SourceId, Span};
use oj_lexer::Symbol;
use oj_scope::{AstSource, DeclId, DeclKind, ScopeId};
use oj_sema::{CallPlan, Checker, Const, Expr, InstanceId, LoopExpansion, ProcedureBody, Value};
use oj_syntax::ast::{
  self, DeclarationFlags, ForFlags, IfFlags, LiteralValue, LoopControlType, NodeData, NodeId,
  OperatorType,
};
use oj_types::{ArrayKind, IntKind, TypeId, TypeKind};

use crate::ir::{
  Abi, BinaryOp, Block, BlockId, Callee, Constant, ConvertKind, Global, GlobalId, GlobalInit, Inst,
  Library, Local, LocalId, ProcId, Procedure, ProcedureFlags, Program, Terminator, UnaryOp,
  ValueId,
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
/// What an `#insert(break=…, continue=…, remove=…)` says a loop control in the
/// program it splices means instead (**L§13.2**). It applies only to a control
/// that would leave the loop the body was spliced into, which is what `loops`
/// being no deeper than it was at the splice says.
struct InsertControls {
  source: SourceId,
  scope: ScopeId,
  loops: usize,
  break_replacement: Option<NodeId>,
  continue_replacement: Option<NodeId>,
  remove_replacement: Option<NodeId>,
}

/// One deferred statement (**L§6.6**).
#[derive(Clone, Copy, Debug)]
struct Deferred {
  scope: ScopeId,
  source: SourceId,
  node: NodeId,
  instance: Option<InstanceId>,
}

struct Loop {
  break_block: BlockId,
  continue_block: BlockId,
  depth: usize,
  /// The name a labelled `break` or `continue` reaches this loop by: the
  /// variable a `while` declared for its condition, or a `for`'s iterator
  /// (**L§6.4**, **L§6.5**). A `case`'s break target has none.
  label: Option<Symbol>,
  /// What `remove` needs, when the loop is a `for` over an array that admits
  /// one (**L§6.5**).
  removal: Option<Removal>,
}

/// The pieces an unordered `remove` works on: the last element is moved into
/// the current slot and the count comes down by one (**L§6.5**).
#[derive(Clone, Copy)]
struct Removal {
  /// The loop's index, which is also its `it_index`.
  index: LocalId,
  /// The loop's own copy of the count, which its head tests against.
  count: LocalId,
  /// The loop's copy of the array's data pointer.
  data: LocalId,
  element: TypeId,
  /// The array's own count word, so that the container shrinks too.
  array_count: ValueId,
  /// Whether the loop runs downwards, in which case the element moved in has
  /// already been visited and the index needs no adjustment.
  reverse: bool,
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
  /// A procedure written at a declaration, keyed by the instantiation it was
  /// resolved in: one nested inside a polymorphic body sees that body's
  /// constants, so there is one of it per specialization (**L§7.8**).
  Decl(Option<InstanceId>, DeclId),
  Node(SourceId, NodeId),
  /// `initializer_of(T)`: the procedure that writes a `T`'s default value
  /// through a pointer (**L§17**). The compiler generates it, so it has no
  /// declaration of its own.
  Initializer(TypeId),
  /// One instantiation of a polymorphic procedure: the header is shared, the
  /// code is not (**L§7.8**).
  Instance(InstanceId),
}

/// Lowers a typechecked program to IR, starting from its entry points and
/// following calls (**L§11.6**): a procedure nothing reachable calls is never
/// lowered, which is what keeps a module's unused polymorphic procedures out
/// of the back end.
pub fn lower(checker: &mut Checker) -> Lowered {
  lower_with_roots(checker, &[], Options::default())
}

/// What the driver asks the back end for, beyond the program itself.
#[derive(Clone, Copy, Debug)]
pub struct Options {
  /// `Build_Options.stack_trace` (**C§4**): every Jai-convention procedure
  /// keeps a `Stack_Trace_Node`, which `-release` turns off.
  pub stack_trace: bool,
}

impl Default for Options {
  fn default() -> Self {
    Self { stack_trace: true }
  }
}

/// The same, with procedures a metaprogram asked to be lowered whether or not
/// anything calls them (**C§3.3**): each is named by the file it was written
/// in and the name it was written with.
pub fn lower_with_roots(
  checker: &mut Checker,
  live: &[(String, String)],
  options: Options,
) -> Lowered {
  let mut lowering = Lowering::new(checker, Mode::Executable);
  lowering.stack_trace = options.stack_trace;
  lowering.run();
  lowering.make_live(live);
  lowering.finish()
}

/// Lowers a library: there is no entry point, so what is reachable is what
/// every `#program_export` reaches (**L§11.6**, **C§4**).
pub fn lower_library(checker: &mut Checker, runtime_support: bool, options: Options) -> Lowered {
  let mut lowering = Lowering::new(checker, Mode::Executable);
  lowering.stack_trace = options.stack_trace;
  lowering.run_exports();
  if runtime_support {
    lowering.export_runtime_support();
  }
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
  /// The procedure being generated, which is what `#this` written inside one
  /// that has no name of its own is the address of (**L§5.11**).
  current_procedure: Option<ProcId>,
  /// The local a declaration became. A macro body's declarations are one
  /// local per expansion, so the instantiation is part of the key
  /// (**L§7.13**).
  local_of_decl: HashMap<(Option<InstanceId>, DeclId), LocalId>,
  /// The constants whose value is being built right now, so that one that
  /// names itself stops instead of looping.
  constants: std::collections::HashSet<DeclId>,
  loops: Vec<Loop>,
  insert_controls: Vec<InsertControls>,
  /// The macros whose bodies are being spliced in right now, innermost last
  /// (**L§7.13**).
  expansions: Vec<Expansion>,
  /// The calls whose arguments are being evaluated, innermost last, and the
  /// macros whose bodies are being spliced in. `#caller_location` is the top
  /// of this (**L§7.13**).
  call_sites: Vec<(SourceId, NodeId)>,
  /// The deferred statements of each open scope, with the instantiation each
  /// was written under: a `` `defer `` a macro left in its caller's block still
  /// names the expansion's locals (**L§7.13**).
  defers: Vec<Vec<Deferred>>,
  returns: Vec<TypeId>,
  /// The declarations of the named return values, so that a `return` that
  /// leaves one out can take its default (**L§7.2**).
  return_decls: Vec<Option<DeclId>>,
  return_pointers: Vec<ValueId>,
  context_value: Option<ValueId>,
  body_scope: ScopeId,
  body_source: SourceId,
  /// The registers the `#asm` blocks of the procedure in hand have declared,
  /// which the blocks after them still name (**L§15**).
  asm_registers: HashMap<(Option<InstanceId>, DeclId), asm::AsmRegisterState>,
  /// A macro parameter of type `__reg` names the caller's register rather than
  /// storage of its own (**L§15**), so it is bound here instead of becoming a
  /// local.
  asm_register_aliases: HashMap<(Option<InstanceId>, DeclId), DeclId>,
  /// `Runtime_Support.__jai_runtime_init`, which the generated entry point
  /// calls before the program (**C§13**).
  runtime_init: Option<ProcId>,
  /// `Build_Options.stack_trace`: every Jai-convention procedure keeps a
  /// `Stack_Trace_Node` of its own (**C§13**).
  stack_trace: bool,
  /// `Stack_Trace_Node` and `Stack_Trace_Procedure_Info`, looked up once.
  trace_types: Option<(TypeId, TypeId)>,
  /// The static info record of every procedure that keeps a node, and what the
  /// generated initializer writes into it.
  trace_infos: Vec<trace::TraceInfo>,
  trace_init: Option<ProcId>,
  /// The node the procedure in hand keeps, while it is being lowered.
  trace_frame: Option<trace::TraceFrame>,
  /// Where whatever is being lowered right now was written, which every
  /// instruction it emits is attributed to (**C§4**).
  current_loc: Option<crate::ir::Loc>,
  /// The files locations name, and where each one landed in the list.
  debug_files: Vec<String>,
  debug_file_ids: HashMap<SourceId, u32>,
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
      current_procedure: None,
      local_of_decl: HashMap::new(),
      constants: std::collections::HashSet::new(),
      loops: Vec::new(),
      insert_controls: Vec::new(),
      expansions: Vec::new(),
      call_sites: Vec::new(),
      defers: Vec::new(),
      returns: Vec::new(),
      return_decls: Vec::new(),
      return_pointers: Vec::new(),
      context_value: None,
      body_scope: ScopeId(0),
      body_source: SourceId(0),
      asm_registers: HashMap::new(),
      asm_register_aliases: HashMap::new(),
      runtime_init: None,
      stack_trace: true,
      trace_types: None,
      trace_infos: Vec::new(),
      trace_init: None,
      trace_frame: None,
      current_loc: None,
      debug_files: Vec::new(),
      debug_file_ids: HashMap::new(),
    }
  }

  /// Where a node was written, as the back end records it (**C§4**).
  fn loc_of(&mut self, source: SourceId, node: NodeId) -> Option<crate::ir::Loc> {
    let span = self.checker.tree_of(source)?.node(node).span;
    self.span_loc(source, span)
  }

  /// The same, for a span the caller already has.
  fn span_loc(&mut self, source: SourceId, span: Span) -> Option<crate::ir::Loc> {
    if source == SourceId::NONE {
      return None;
    }
    let file = match self.debug_file_ids.get(&source) {
      Some(id) => *id,
      None => {
        let path = {
          let file = self.checker.program().sources().file(source);
          std::path::absolute(file.path())
            .unwrap_or_else(|_| file.path().to_path_buf())
            .to_string_lossy()
            .into_owned()
        };
        let id = self.debug_files.len() as u32;
        self.debug_files.push(path);
        self.debug_file_ids.insert(source, id);
        id
      }
    };
    let position = self
      .checker
      .program()
      .sources()
      .file(source)
      .location(span.start);
    Some(crate::ir::Loc {
      file,
      line: position.line,
      column: position.column,
    })
  }

  fn run(&mut self) {
    let Some(main) = self.find_entry() else {
      // The reference names the entry point it looked for and gives no place
      // in the program, since the program is what is missing it (**C§12**).
      self.diagnostics.push(Diagnostic::error(
        SourceId::NONE,
        Span::at(0),
        "No program entry point was found. (The designated entry point name is 'main'.)",
      ));
      return;
    };

    self.entry_decl = Some(main);
    let entry = self.procedure_id(main);
    self.entry = Some(entry);
    // The generated entry point hands the program the context
    // `Runtime_Support.__jai_runtime_init` builds — the allocator, the logger
    // and the temporary storage a `#Context` starts with (**C§13**).
    if let Some(init) = self.checker.procedure_named("__jai_runtime_init") {
      self.runtime_init = Some(self.procedure_id(init));
    }
    self.drain_queue();
    // A global initializer may be the first thing to reach a procedure, so
    // whatever it named still has to be lowered.
    self.emit_global_initializers();
    self.drain_queue();
  }

  /// The roots of a library are its exports rather than its `main`. Only the
  /// program's own are taken: what Runtime_Support exports depends on
  /// `runtime_support_definitions`, which nothing sets yet, and its entry point
  /// has no place in a library (**C§4**).
  fn run_exports(&mut self) {
    let program = self.checker.program();
    let tree = program.tree();
    let main = program.main_scope();
    let exports: Vec<DeclId> = (0..tree.declaration_count() as u32)
      .map(DeclId)
      .filter(|id| {
        let decl = tree.decl(*id);
        decl.kind == oj_scope::DeclKind::Procedure
          && decl.flags.contains(ast::DeclarationFlags::PROGRAM_EXPORT)
          && tree.enclosing_module(decl.scope) == Some(main)
      })
      .collect();
    for export in exports {
      self.procedure_id(export);
    }
    self.drain_queue();
    self.emit_global_initializers();
    self.drain_queue();
  }

  /// What `Build_Options.runtime_support_definitions` asks a library to take
  /// from Runtime_Support (**C§4**): `__jai_runtime_init` and
  /// `__jai_runtime_fini`, under the names the reference gives them, so that a
  /// non-Jai program linking the library can build the context the code in it
  /// expects. `__system_entry_point` is not among them — orangejuice generates
  /// its own `main` (`docs/spec.md` §10) — so `ENTRY_POINT_AND_INIT` and
  /// `ONLY_INIT` amount to the same thing here.
  fn export_runtime_support(&mut self) {
    for name in ["__jai_runtime_init", "__jai_runtime_fini"] {
      let Some(decl) = self.checker.procedure_named(name) else {
        continue;
      };
      let id = self.procedure_id(decl);
      self.drain_queue();
      let procedure = &mut self.procedures[id.0 as usize];
      procedure.flags |= ProcedureFlags::EXPORT;
      procedure.symbol = String::from(name);
    }
  }

  /// Lowers procedures nothing reachable calls, because a metaprogram said to
  /// (**C§3.3**). A name that matches nothing is passed over: the metaprogram
  /// named a procedure this compilation does not have.
  fn make_live(&mut self, live: &[(String, String)]) {
    if live.is_empty() {
      return;
    }
    let tree = self.checker.program().tree();
    let mut roots = Vec::new();
    for id in (0..tree.declaration_count() as u32).map(DeclId) {
      let decl = tree.decl(id);
      if decl.kind != DeclKind::Procedure {
        continue;
      }
      let Some(source) = decl.source else { continue };
      let path = self
        .checker
        .program()
        .sources()
        .file(source)
        .path()
        .display()
        .to_string();
      let name = self
        .checker
        .interner()
        .resolve_lossy(decl.name)
        .into_owned();
      if live
        .iter()
        .any(|(file, wanted)| *wanted == name && (file.is_empty() || *file == path))
      {
        roots.push(id);
      }
    }
    for root in roots {
      self.procedure_id(root);
    }
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
    // The image begins with the `Runtime_Info` the program reads through
    // `__runtime_info`, and ends with the `[] *Type_Info` that struct points
    // at (**C§3.3**).
    let mut table = std::mem::take(&mut self.type_table);
    table.finish(self.checker);
    self.type_table = table;
    let bytes: Box<[u8]> = Box::from(self.type_table.bytes());
    let relocations: Box<[(u64, u64)]> = Box::from(self.type_table.relocations());
    let global = &mut self.globals[id.0 as usize];
    global.size = bytes.len() as u64;
    global.init = GlobalInit::Image { bytes, relocations };
  }

  fn finish(mut self) -> Lowered {
    // Every procedure that keeps a node is known by now, which is what the
    // generated initializer needs before it can be written (**C§13**).
    self.emit_trace_info_init();
    self.place_type_table();
    self.resolve_symbol_collisions();
    let type_table = crate::ir::TypeTableImage {
      symbol: self
        .type_table_global
        .map(|id| self.globals[id.0 as usize].symbol.clone()),
      offsets: self.type_table.placements(),
    };
    let Self {
      checker,
      procedures,
      globals,
      libraries,
      diagnostics,
      entry,
      runtime_init,
      context_type,
      trace_init,
      debug_files,
      ..
    } = self;
    // The back end has the type table but not the interner, so the names go
    // with the program (**C§4**).
    let names = {
      let types = checker.types().clone();
      let interner = checker.interner();
      crate::ir::DebugNames {
        types: (0..types.len() as u32)
          .map(|id| types.name(TypeId(id), interner))
          .collect(),
        members: (0..types.struct_count() as u32)
          .map(|id| {
            types
              .struct_info(oj_types::StructId(id))
              .members
              .iter()
              .map(|member| interner.resolve_lossy(member.name).into_owned())
              .collect()
          })
          .collect(),
      }
    };
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
        runtime_init,
        global_init,
        stack_trace_init: trace_init,
        context_type,
        libraries,
        type_table,
        debug_files,
        names,
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
      .find(|unit| program.tree().parent(unit.scope) == Some(main_scope))
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

  /// Gives up a symbol an ordinary procedure took to the one that has to have
  /// it. Jai has no name mangling, so two overloads of a name compete for one
  /// symbol (**C§11**), and a `#foreign` or `#compiler` procedure cannot be
  /// renamed — the name is what it binds to. `Compiler`'s two
  /// `compiler_report`s are the pair that shows it: the one with a Jai body is
  /// reached first and would otherwise take the name the intrinsic needs.
  fn resolve_symbol_collisions(&mut self) {
    let bound: std::collections::HashSet<String> = self
      .procedures
      .iter()
      .filter(|procedure| {
        procedure
          .flags
          .intersects(ProcedureFlags::FOREIGN | ProcedureFlags::EXPORT)
      })
      .map(|procedure| procedure.symbol.clone())
      .collect();
    if bound.is_empty() {
      return;
    }
    let mut used: std::collections::HashSet<String> = self
      .procedures
      .iter()
      .map(|procedure| procedure.symbol.clone())
      .chain(self.globals.iter().map(|global| global.symbol.clone()))
      .collect();
    for index in 0..self.procedures.len() {
      if self.procedures[index]
        .flags
        .intersects(ProcedureFlags::FOREIGN | ProcedureFlags::EXPORT)
      {
        continue;
      }
      let symbol = self.procedures[index].symbol.clone();
      if !bound.contains(&symbol) {
        continue;
      }
      let mut suffix = 1u32;
      let renamed = loop {
        let candidate = format!("{symbol}.{suffix}");
        if !used.contains(&candidate) {
          break candidate;
        }
        suffix += 1;
      };
      used.insert(renamed.clone());
      self.procedures[index].symbol = renamed;
    }
  }

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
    let key = ProcKey::Decl(self.checker.decl_instance(decl), decl);
    if let Some(id) = self.procedure_ids.get(&key) {
      return *id;
    }
    let type_id = self.checker.decl_type(decl).value;
    self.declare_procedure(key, Some(decl), type_id, None)
  }

  /// The procedure `initializer_of(T)` names, written out once per type
  /// (**L§17**).
  fn initializer_id(&mut self, type_id: TypeId) -> ProcId {
    let key = ProcKey::Initializer(type_id);
    if let Some(id) = self.procedure_ids.get(&key) {
      return *id;
    }
    let id = ProcId(self.procedures.len() as u32);
    self.procedure_ids.insert(key, id);
    let mut signature = oj_types::ProcedureType::new(vec![TypeId::VOID_POINTER], Vec::new());
    signature.flags = oj_types::ProcedureFlags::HAS_NO_CONTEXT;
    let procedure_type = self.checker.types_table_mut().procedure(signature);
    let name = format!("__oj_initializer_{}", type_id.0);
    let symbol = self.unique_symbol(&name);
    let pointer = self.pointer_to(type_id);
    self.procedures.push(Procedure {
      symbol,
      name,
      type_id: procedure_type,
      parameters: vec![pointer],
      returns: Vec::new(),
      flags: ProcedureFlags::COMPILER_GENERATED | ProcedureFlags::NO_CONTEXT,
      library: None,
      abi: crate::ir::Abi {
        parameters: vec![crate::ir::AbiParameter {
          type_id: pointer,
          kind: crate::ir::ParameterKind::Value,
          class: None,
        }],
        direct_return: None,
        return_class: None,
      },
      locals: Vec::new(),
      blocks: Vec::new(),
      value_types: Vec::new(),
      entry: BlockId(0),
      location: None,
    });
    self.queue.push_back((id, key));
    id
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
    let mut flags = ProcedureFlags::empty();
    let mut library = None;
    let mut symbol = None;
    if let Some(body) = &body {
      // A `#compiler` procedure has no body either, but the compiler is what
      // answers it, so it keeps the Jai convention and binds to `oj-meta`
      // under the name `#compiler "…"` gave it (**C§3.3**).
      // A `#compiler` procedure with a body — Preload writes one for
      // `get_current_workspace` — keeps that body in the executable and is
      // answered by the compiler only while compile-time code runs (**C§3.3**).
      let intrinsic = body
        .flags
        .contains(ast::ProcedureFlags::SYNTACTICALLY_MARKED_AS_COMPILER)
        && (body.block.is_none() || self.mode == Mode::CompileTime);
      if intrinsic {
        flags |= ProcedureFlags::FOREIGN | ProcedureFlags::COMPILER;
        symbol = Some(match &body.intrinsic_name {
          Some(name) => String::from_utf8_lossy(name).into_owned(),
          None => name.clone(),
        });
      }
      // A body that lives in a library is called the C way and has no context
      // to be handed (**L§12.1**).
      else if body.block.is_none() {
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
      // `#program_export` makes the symbol the linker sees the procedure's own
      // name, or the one the directive gave it (**L§7.4**).
      if decl.is_some_and(|decl| {
        self
          .checker
          .program()
          .tree()
          .decl(decl)
          .flags
          .contains(ast::DeclarationFlags::PROGRAM_EXPORT)
      }) {
        flags |= ProcedureFlags::EXPORT;
        symbol = Some(name.clone());
      }
      if let Some(export) = &body.export_name {
        flags |= ProcedureFlags::EXPORT;
        symbol = Some(String::from_utf8_lossy(export).into_owned());
      }
      if let Some(foreign) = &body.foreign_name {
        symbol = Some(String::from_utf8_lossy(foreign).into_owned());
      } else if flags.contains(ProcedureFlags::FOREIGN) && symbol.is_none() {
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
      location: None,
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
        let decl = program.tree().decl(candidate);
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
    let info = self.checker.program().tree().decl(decl);
    let name = self.text(info.name);
    // `__runtime_info` is the compiler's, not somebody else's: the type table
    // image *is* that struct, so the `#elsewhere` declaration
    // `get_runtime_info` reads through resolves to it rather than to a symbol
    // nothing defines (**C§3.3**).
    if info.flags.contains(DeclarationFlags::ELSEWHERE) && name == RUNTIME_INFO_SYMBOL {
      let table = self.type_table_id();
      self.global_ids.insert(decl, table);
      return table;
    }

    let id = GlobalId(self.globals.len() as u32);
    self.global_ids.insert(decl, id);
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
    let info = self.checker.program().tree().decl(decl);
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
      // A literal in a `*u8` is its bytes with a trailing zero rather than the
      // `{count, data}` pair a `string` is, which is what lets one reach a C
      // procedure (**L§3.4**, **L§5.10**).
      Value::String(text) if self.checker.types().pointee(target).is_some() => {
        let mut bytes = text.to_vec();
        bytes.push(0);
        Some(Constant::Bytes {
          bytes: bytes.into_boxed_slice(),
          links: Box::default(),
        })
      }
      Value::String(text) => Some(Constant::String(text.clone())),
      // A pointer among the bytes names compile-time storage, which only the
      // instruction form can carry along; as plain data it is dropped.
      Value::Bytes(bytes) => Some(Constant::Bytes {
        bytes: bytes.data.clone(),
        links: Box::default(),
      }),
      // A `Code` value is a piece of the program, not data (**L§13.1**), and a
      // procedure is an address only the module that generates it can supply.
      // A `.data` or a global address is settled by the linker rather than by
      // the compiler (**L§5.11**), so the expression it came from is lowered
      // instead of the value.
      Value::Type(_)
      | Value::EnumName(_)
      | Value::Code { .. }
      | Value::Procedure(_)
      | Value::Address(_)
      | Value::Location { .. }
      | Value::Written { .. } => None,
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
        | TypeKind::Code
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
    let loc = self.current_loc;
    let block = &mut self.blocks[self.current.0 as usize];
    block.instructions.push(instruction);
    block.locations.push(loc);
  }

  fn new_block(&mut self) -> BlockId {
    let id = BlockId(self.blocks.len() as u32);
    self.blocks.push(Block::new());
    id
  }

  fn terminate(&mut self, terminator: Terminator) {
    let loc = self.current_loc;
    let block = &mut self.blocks[self.current.0 as usize];
    if matches!(block.terminator, Terminator::Unreachable) {
      block.terminator = terminator;
      block.terminator_location = loc;
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
    let location = self.current_loc;
    self.locals.push(Local {
      name,
      type_id,
      size,
      alignment,
      location,
      parameter: None,
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

  /// The span a node was written at, for a diagnostic that names a place
  /// rather than a node.
  fn span_of(&self, source: SourceId, node: NodeId) -> Span {
    self
      .checker
      .tree_of(source)
      .map_or(Span::at(0), |ast| ast.node(node).span)
  }

  /// A warning, deduplicated the way an error is: one instantiation per
  /// specialization would otherwise repeat it.
  fn warn(&mut self, source: SourceId, span: Span, message: impl Into<String>) {
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
      .push(Diagnostic::warning(source, span, message));
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

/// The generated procedure that fills in the stack trace info records
/// (**C§13**).
const TRACE_INIT_SYMBOL: &str = "__oj_stack_trace_init";

/// The symbol the `Type_Info` image takes (**L§17**).
const TYPE_TABLE_SYMBOL: &str = "__oj_type_table";

/// The `Runtime_Info` the distribution reads the type table through, which
/// `get_runtime_info` declares `#elsewhere` and the compiler defines: the type
/// table image begins with it (**C§3.3**).
const RUNTIME_INFO_SYMBOL: &str = "__runtime_info";

/// The symbol the program's own entry point gets, so that the generated `main`
/// the C runtime calls is free (**C§13**).
const PROGRAM_MAIN_SYMBOL: &str = "__program_main";
