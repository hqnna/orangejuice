use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use oj_diag::{Diagnostic, SourceId, SourceMap, Span};
use oj_lexer::{Interner, Symbol};
use oj_source::{ImportPath, ModuleError};
use oj_syntax::Parsed;
use oj_syntax::ast::{
  Argument, AsmOperandKind, AsmSize, DeclarationFlags, FilterType, ImportType, NodeData, NodeId,
  ProcedureFlags, ScopeType,
};

use crate::constants::{AstSource, ConstValue, Evaluator};
use crate::tree::{
  Branch, Decl, DeclId, DeclKind, ImportEdge, PendingProvider, ScopeId, ScopeKind, ScopeTree,
  UsedValue, Visibility,
};

/// The three scopes a procedure header opens (**L§7.8**): the constants block
/// holding its `$T`s and baked values, the arguments, and the named returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcedureScopes {
  pub constants: ScopeId,
  pub arguments: ScopeId,
  pub returns: ScopeId,
  /// Whether the header has to be instantiated before its body exists — it is
  /// polymorphic, a macro, or it bakes a value.
  pub polymorphic: bool,
}

/// The identifiers whose lookup the program still owes an answer, collected
/// while the tree is built and resolved once it is complete so that data scopes
/// stay order-independent (**L§11.7**).
#[derive(Clone, Debug)]
pub struct Reference {
  pub name: Symbol,
  pub scope: ScopeId,
  pub source: SourceId,
  pub span: Span,
  pub node: NodeId,
  /// Written in a branch of a `#if` M3 could not decide, or in a module only
  /// such a branch reaches. The reference compiler never typechecks the branch
  /// it discards, so a name that misses here is not an error.
  pub speculative: bool,
}

#[derive(Clone, Debug)]
pub struct Options {
  /// The distribution whose `modules/` holds Preload and the standard modules.
  pub jai_dir: Option<PathBuf>,
  /// `-import_dir` directories, in command-line order.
  pub import_dirs: Vec<PathBuf>,
  /// Follow `#load` and `#import`. Turning this off checks one file on its own,
  /// which is what the corpus test does for all 702 vendor files.
  pub follow_imports: bool,
  /// Parse the distribution's `Preload.jai` into the root scope (**L§11.4**).
  pub load_preload: bool,
  /// `remap_import` calls a metaprogram made for this workspace: the module
  /// the `#import` is written in, the name it names, and what to import
  /// instead — or nothing at all, which blocks it (**C§3.3**).
  pub import_remaps: Vec<ImportRemap>,
}

/// One `remap_import` (**C§3.3**). `host` is `""` for the main program and
/// `"*"` for every module; `import` is `"*"` for every import, in which case
/// `replacement` has to be empty.
#[derive(Clone, Debug)]
pub struct ImportRemap {
  pub host: String,
  pub import: String,
  pub replacement: String,
}

impl ImportRemap {
  fn matches(&self, host: &str, import: &str) -> bool {
    (self.host == "*" || self.host == host) && (self.import == "*" || self.import == import)
  }
}

impl Default for Options {
  fn default() -> Self {
    Self {
      jai_dir: None,
      import_dirs: Vec::new(),
      follow_imports: true,
      load_preload: true,
      import_remaps: Vec::new(),
    }
  }
}

impl Options {
  pub fn single_file() -> Self {
    Self {
      follow_imports: false,
      load_preload: false,
      ..Self::default()
    }
  }
}

/// One module instantiation: what it was imported as, the file it starts at,
/// and the scope it became (**L§11.2**).
#[derive(Clone, Debug)]
pub struct Module {
  pub name: String,
  pub entry: PathBuf,
  pub scope: ScopeId,
  pub kind: ModuleKind,
}

/// What a module is to the program that has it, which is what `Message_Import`
/// reports (**C§3.2**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleKind {
  Preload,
  RuntimeSupport,
  MainProgram,
  File,
}

/// One parsed file and the file scope it was resolved into.
pub struct Unit {
  pub source: SourceId,
  pub path: PathBuf,
  pub scope: ScopeId,
  pub parsed: Arc<Parsed>,
}

/// A parsed `#insert` string: the program it stands for, and where it went.
/// It is not a [`Unit`] — nothing loaded it, and a `#run` written in it belongs
/// to whatever scope the `#insert` expanded into rather than to a file.
pub struct Insertion {
  pub path: PathBuf,
  pub scope: ScopeId,
  pub parsed: Arc<Parsed>,
}

/// An `#insert` the scope tree left for the typechecker, and where its
/// program goes once the text is known (**L§13.2**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingInsert {
  pub source: SourceId,
  pub node: NodeId,
  pub scope: ScopeId,
  pub kind: InsertKind,
}

/// What one `#insert` expanded to (**L§13.2**): the block its string parsed
/// into, and the scope its statements were admitted to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expansion {
  pub source: SourceId,
  pub root: NodeId,
  pub scope: ScopeId,
}

/// The kind of scope an `#insert` expands into (**L§13.2**). A data scope
/// obeys the `#scope_*` directive in effect; a struct or enum body has one
/// target; a block takes the statements in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertKind {
  Data(Visibility),
  Members,
  Imperative,
  /// An `#insert` in expression position, whose text has to be one expression
  /// statement and which declares nothing (**L§13.2**).
  Expression,
}

/// Where declarations written at this point in a data scope go. A file's
/// `#scope_*` directive moves the target between the file scope and the module
/// scope (**L§4.4**); a struct or enum body has only one target.
#[derive(Clone, Copy, Debug)]
struct DataTarget {
  scope: ScopeId,
  module: Option<ScopeId>,
  visibility: Visibility,
  conditional: bool,
}

impl DataTarget {
  fn nested(scope: ScopeId) -> Self {
    Self {
      scope,
      module: None,
      visibility: Visibility::Export,
      conditional: false,
    }
  }

  /// The scope a declaration lands in: the module scope under `#scope_export`
  /// and `#scope_module`, the file scope under `#scope_file`.
  fn destination(&self) -> ScopeId {
    match (self.visibility, self.module) {
      (Visibility::File, _) | (_, None) => self.scope,
      (_, Some(module)) => module,
    }
  }

  /// The scope a bare `#import` widens. `#scope_file` does *not* narrow this:
  /// the reference makes an unnamed file-level import visible to every file of
  /// the module whatever directive is in effect, and only the *declaration* a
  /// named import introduces is file-private (**L§4.4**, **L§11.2**).
  fn import_destination(&self) -> ScopeId {
    self.module.unwrap_or(self.scope)
  }
}

/// A `#if` whose condition did not fold on first sight. The scheduler retries
/// these until nothing changes, which is what makes `#if USE_X` work when `X`
/// is declared further down the file (**C§6.1**).
enum PendingIf {
  Data {
    node: NodeId,
    target: DataTarget,
    source: SourceId,
  },
  Imperative {
    node: NodeId,
    scope: ScopeId,
    source: SourceId,
  },
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct ModuleKey {
  entry: PathBuf,
  parameters: String,
  instance: u32,
}

pub struct Program<'a> {
  sources: &'a SourceMap,
  interner: &'a Interner,
  options: Options,
  import_path: ImportPath,
  tree: ScopeTree,
  units: boxcar::Vec<Unit>,
  unit_of_source: RefCell<HashMap<SourceId, usize>>,
  /// The programs `#insert`s expanded to, which grow after the tree is first
  /// built: this is the mutable program of **L§13.2**.
  inserted: boxcar::Vec<Insertion>,
  inserted_of_source: RefCell<HashMap<SourceId, usize>>,
  expansions: RefCell<HashMap<(SourceId, NodeId), Expansion>>,
  /// Every module instantiation, in the order it was made, so that a
  /// metaprogram can be told about each one (**C§3.2**).
  module_records: RefCell<Vec<Module>>,
  /// The `#insert`s whose text the scope tree could not work out on its own:
  /// they need a type or a `#run`, so the typechecker expands them.
  pending_inserts: RefCell<Vec<PendingInsert>>,
  modules: RefCell<HashMap<ModuleKey, ScopeId>>,
  loaded: RefCell<HashSet<(ScopeId, PathBuf)>>,
  /// The member scope of each struct and enum definition, so that a `using` of
  /// a type declaration can widen the scope around it without waiting for
  /// types (**L§6.8**).
  aggregate_scopes: RefCell<HashMap<(SourceId, NodeId), ScopeId>>,
  /// The scope a `for` loop declares its `it` and `it_index` in, by the loop
  /// node: an implicit one has no identifier to be found through (**L§6.5**).
  loop_scopes: RefCell<HashMap<(SourceId, NodeId), ScopeId>>,
  /// The scopes every procedure header opened, by the header node. An
  /// instantiation binds the `$T`s declared in the constants block, so it
  /// needs a way in that does not go through a name (**L§7.8**).
  procedure_scopes: RefCell<HashMap<(SourceId, NodeId), ProcedureScopes>>,
  /// The constants scope of every polymorphic procedure and macro. Nothing
  /// inside one of those exists until an instantiation makes it exist, so a
  /// `#run` written there waits for an instantiation rather than running (**L§12.1**).
  uninstantiated_scopes: RefCell<HashSet<ScopeId>>,
  /// The constants scope of every macro. A name that misses inside one of
  /// these is a wait for the expansion, not an error: the macro's body sees
  /// the caller's locals (**L§7.13**).
  macro_scopes: RefCell<HashSet<ScopeId>>,
  builtins: RefCell<HashMap<DeclId, ConstValue>>,
  references: RefCell<Vec<Reference>>,
  /// Names some macro declares with a backtick, which land in whatever block
  /// the macro expands into rather than where they are written (**L§7.13**).
  /// A miss on one of these is a wait for the expansion, not an error.
  macro_injected: RefCell<HashSet<Symbol>>,
  diagnostics: RefCell<Vec<Diagnostic>>,
  pending_ifs: RefCell<Vec<PendingIf>>,
  /// Nonzero while the branches of an undecidable `#if` are being admitted, so
  /// nothing reached from there is known to be real: its diagnostics are held
  /// back until the condition can be decided (M6 runs the `#run`s most of them
  /// wait on).
  speculative: Cell<u32>,
  /// Nonzero while declarations belong to one branch of an undecidable `#if`.
  /// Unlike `speculative` this stops at a module boundary: a module is loaded
  /// once however it was reached, so its own declarations are not conditional.
  conditional: Cell<u32>,
  /// The branch of an undecidable `#if` whose statements are being admitted
  /// right now, so that whoever can fold the condition later knows which
  /// declarations to drop (**L§6.10**).
  branch: Cell<Option<Branch>>,
  unshared_instances: Cell<u32>,
  preload: ScopeId,
  main: ScopeId,
}

impl AstSource for Program<'_> {
  fn ast_of(&self, source: SourceId) -> Option<&oj_syntax::ast::Ast> {
    if let Some(index) = self.unit_of_source.borrow().get(&source).copied() {
      return Some(&self.units[index].parsed.ast);
    }
    let index = *self.inserted_of_source.borrow().get(&source)?;
    Some(&self.inserted[index].parsed.ast)
  }
}

/// The identifiers the compiler injects into every program (**L§3.1**,
/// **L§4.3**). They resolve directly and cannot be shadowed.
const BASIC_TYPE_NAMES: [&[u8]; 18] = [
  b"s8", b"s16", b"s32", b"s64", b"u8", b"u16", b"u32", b"u64", b"v128", b"int", b"float",
  b"float32", b"float64", b"string", b"void", b"bool", b"Any", b"Code",
];

impl<'a> Program<'a> {
  /// Resolves `root` and everything it loads and imports into a scope tree.
  pub fn build(
    sources: &'a SourceMap,
    interner: &'a Interner,
    root: &Path,
    options: Options,
  ) -> Self {
    Self::build_input(sources, interner, &[root.to_path_buf()], &[], options)
  }

  /// Resolves a whole workspace: the files a metaprogram added and the strings
  /// it added beside them, all of them files of the main program (**C§3.1**).
  /// An added string is a source of its own under the name it is given, so a
  /// diagnostic in one points somewhere a reader can find.
  pub fn build_input(
    sources: &'a SourceMap,
    interner: &'a Interner,
    roots: &[PathBuf],
    strings: &[(PathBuf, String)],
    options: Options,
  ) -> Self {
    let anchor = roots
      .first()
      .cloned()
      .or_else(|| strings.first().map(|(path, _)| path.clone()))
      .unwrap_or_default();
    let jai_dir = options.jai_dir.clone().unwrap_or_default();
    let mut import_path = ImportPath::default_for(&anchor, &jai_dir);
    for directory in options.import_dirs.iter().rev() {
      import_path.prepend(directory.clone());
    }

    let tree = ScopeTree::new();
    let preload = tree.push_scope(ScopeKind::Preload, None);
    let main = tree.push_scope(ScopeKind::Module, Some(preload));

    let program = Self {
      sources,
      interner,
      options,
      import_path,
      tree,
      units: boxcar::Vec::new(),
      unit_of_source: RefCell::default(),
      inserted: boxcar::Vec::new(),
      inserted_of_source: RefCell::default(),
      expansions: RefCell::default(),
      pending_inserts: RefCell::default(),
      module_records: RefCell::default(),
      modules: RefCell::default(),
      loaded: RefCell::default(),
      aggregate_scopes: RefCell::default(),
      loop_scopes: RefCell::default(),
      procedure_scopes: RefCell::default(),
      uninstantiated_scopes: RefCell::default(),
      macro_scopes: RefCell::default(),
      builtins: RefCell::default(),
      references: RefCell::default(),
      macro_injected: RefCell::default(),
      diagnostics: RefCell::default(),
      pending_ifs: RefCell::default(),
      speculative: Cell::new(0),
      conditional: Cell::new(0),
      branch: Cell::new(None),
      unshared_instances: Cell::new(0),
      preload,
      main,
    };

    program.record_module(String::new(), anchor.clone(), main, ModuleKind::MainProgram);
    program.declare_builtins();
    if program.options.load_preload {
      program.load_preload();
    }
    for root in roots {
      program.load_file_into(main, root, None);
    }
    for (path, text) in strings {
      program.load_text_into(main, path, text);
    }
    program.settle_pending_ifs();
    program
  }

  pub fn tree(&self) -> &ScopeTree {
    &self.tree
  }

  pub fn units(&self) -> impl Iterator<Item = &Unit> + '_ {
    self.units.iter().map(|(_, unit)| unit)
  }

  pub fn unit_count(&self) -> usize {
    self.units.count()
  }

  pub fn references(&self) -> std::cell::Ref<'_, Vec<Reference>> {
    self.references.borrow()
  }

  /// Whether `name` is one a macro injects into its caller's block.
  pub fn is_macro_injected(&self, name: Symbol) -> bool {
    self.macro_injected.borrow().contains(&name)
  }

  pub fn diagnostics(&self) -> std::cell::Ref<'_, Vec<Diagnostic>> {
    self.diagnostics.borrow()
  }

  pub fn interner(&self) -> &'a Interner {
    self.interner
  }

  /// The files the program was read from, which is what a `#location` needs to
  /// turn a span into a name, a line and a character (**L§5.14**).
  pub fn sources(&self) -> &'a SourceMap {
    self.sources
  }

  pub fn preload_scope(&self) -> ScopeId {
    self.preload
  }

  pub fn main_scope(&self) -> ScopeId {
    self.main
  }

  /// The scope holding the members of the `struct`, `union` or `enum` written
  /// at `node`. The typechecker reads the members from it in declaration
  /// order, which is the order they are laid out in (**L§3.14**).
  pub fn aggregate_scope(&self, source: SourceId, node: NodeId) -> Option<ScopeId> {
    self.aggregate_scopes.borrow().get(&(source, node)).copied()
  }

  /// Every recorded aggregate body, as `(source, node, members scope)`. The
  /// typechecker inverts this to find the definition a member scope belongs to.
  /// The scope a `for` loop declared its `it` and `it_index` in (**L§6.5**). A
  /// loop that did not name them has no identifier to find them through, so
  /// this is the only way in.
  pub fn loop_scope(&self, source: SourceId, node: NodeId) -> Option<ScopeId> {
    self.loop_scopes.borrow().get(&(source, node)).copied()
  }

  /// The scopes a procedure header opened (**L§7.8**).
  pub fn procedure_scopes(&self, source: SourceId, header: NodeId) -> Option<ProcedureScopes> {
    self
      .procedure_scopes
      .borrow()
      .get(&(source, header))
      .copied()
  }

  /// Whether a scope lies inside a polymorphic procedure or a macro, whose
  /// body only exists once something instantiates or expands it. Nothing
  /// written there is checked or executed until then (**L§7.8**, **L§7.13**).
  pub fn is_uninstantiated(&self, scope: ScopeId) -> bool {
    let mut current = Some(scope);
    while let Some(id) = current {
      if self.uninstantiated_scopes.borrow().contains(&id) {
        return true;
      }
      current = self.tree.parent(id);
    }
    false
  }

  /// Whether a scope lies inside a macro's body, where a name the macro does
  /// not declare may still be one of the caller's (**L§7.13**). A lookup that
  /// misses there is a wait for the expansion rather than an error.
  pub fn is_in_macro(&self, scope: ScopeId) -> bool {
    let mut current = Some(scope);
    while let Some(id) = current {
      if self.macro_scopes.borrow().contains(&id) {
        return true;
      }
      current = self.tree.parent(id);
    }
    false
  }

  pub fn aggregate_scopes(&self) -> Vec<(SourceId, NodeId, ScopeId)> {
    self
      .aggregate_scopes
      .borrow()
      .iter()
      .map(|((source, node), scope)| (*source, *node, *scope))
      .collect()
  }

  pub fn has_errors(&self) -> bool {
    self.diagnostics.borrow().iter().any(Diagnostic::is_error)
  }

  fn declare_builtins(&self) {
    for name in BASIC_TYPE_NAMES {
      self.declare_builtin(name, None);
    }
    // `Type` is compiler-provided but looked up normally, so it lives in the
    // Preload scope and may be shadowed (**L§3.1**); `_` is always declared
    // (**L§4.6**).
    self.declare_builtin(b"Type", None);
    let discard = self.interner.intern(b"_");
    self.tree.set_discard_name(discard);
    self.declare_builtin(b"_", None);
    let linux = self.interner.intern(b"LINUX");
    let x64 = self.interner.intern(b"X64");
    self.declare_builtin(b"OS", Some(ConstValue::EnumName(linux)));
    self.declare_builtin(b"CPU", Some(ConstValue::EnumName(x64)));
    self.declare_builtin(b"IS_CROSS_COMPILING", Some(ConstValue::Bool(false)));
    self.declare_builtin(b"MACHINE_OPTIONS_SIZE", Some(ConstValue::Int(256)));
    // Runtime_Support sizes the first thread's temporary storage by this, and
    // the compiler is what defines it (**C§4**).
    self.declare_builtin(b"TEMPORARY_STORAGE_SIZE", Some(ConstValue::Int(32768)));
  }

  fn declare_builtin(&self, name: &[u8], value: Option<ConstValue>) {
    let name = self.interner.intern(name);
    let scope = self.preload;
    let id = self
      .tree
      .declare(Decl {
        name,
        scope,
        kind: DeclKind::Builtin,
        visibility: Visibility::Export,
        flags: DeclarationFlags::IS_CONSTANT,
        source: None,
        span: Span::at(0),
        node: None,
        conditional: false,
        branch: None,
        overloadable: false,
      })
      .unwrap_or_else(|previous| previous);
    if let Some(value) = value {
      self.builtins.borrow_mut().insert(id, value);
    }
  }

  fn load_preload(&self) {
    let Some(jai_dir) = self.options.jai_dir.clone() else {
      return;
    };
    let preload = jai_dir.join("modules").join("Preload.jai");
    if preload.is_file() {
      let scope = self.preload;
      self.record_module(
        String::from("Preload"),
        preload.clone(),
        scope,
        ModuleKind::Preload,
      );
      self.load_file_into(scope, &preload, None);
    }
    // `Runtime_Support` is imported by the compiler into every workspace, not
    // by the program, so its exports are visible the way Preload's are
    // (**L§11.4**).
    let scope = self.preload;
    self.import_module_by_name("Runtime_Support", scope);
  }

  /// Instantiates a module the compiler imports on the program's behalf, with
  /// no parameters, and makes its exports visible in `into`.
  fn import_module_by_name(&self, name: &str, into: ScopeId) {
    let Ok(resolved) = oj_source::resolve_module(name, &self.import_path) else {
      return;
    };
    let key = ModuleKey {
      entry: resolved.entry.clone(),
      parameters: String::new(),
      instance: 0,
    };
    let existing = self.modules.borrow().get(&key).copied();
    let module = match existing {
      Some(existing) => existing,
      None => {
        let module = self.tree.push_scope(ScopeKind::Module, Some(self.preload));
        self.modules.borrow_mut().insert(key, module);
        let kind = match name {
          "Runtime_Support" => ModuleKind::RuntimeSupport,
          _ => ModuleKind::File,
        };
        self.record_module(name.to_string(), resolved.entry.clone(), module, kind);
        self.load_module_files(module, &resolved.entry, None);
        module
      }
    };
    self.tree.add_import(
      into,
      ImportEdge {
        target: module,
        only: Vec::new(),
        except: Vec::new(),
        span: Span::at(0),
        source: None,
        transitive: false,
      },
    );
  }

  // ---------------------------------------------------------------- files ---

  /// Parses `path` as a new file scope under `module` and walks it. `origin`
  /// is the `#load`/`#import` site a read error is reported at.
  fn load_file_into(&self, module: ScopeId, path: &Path, origin: Option<(SourceId, Span)>) {
    let key = (module, path.to_path_buf());
    if !self.loaded.borrow_mut().insert(key) {
      if let Some((source, span)) = origin {
        self.error(
          source,
          span,
          format!(
            "The file '{}' is loaded twice into the same scope.",
            path.display()
          ),
        );
      }
      return;
    }

    let source = match oj_source::load_file(self.sources, path) {
      Ok(source) => source,
      Err(error) => {
        let message = format!("Could not read '{}': {error}", path.display());
        match origin {
          Some((source, span)) => self.error(source, span, message),
          // The root file has no `#load` or `#import` to point at, so the
          // unreadable file stands in for itself: an empty source under its own
          // path, which every diagnostic needs to be renderable.
          None => {
            let source = self.sources.add_bytes(path, Vec::new());
            self.error(source, Span::at(0), message);
          }
        }
        return;
      }
    };

    self.admit_file(module, source, path);
  }

  /// Admits a string a metaprogram added as a file of `module`, under a path of
  /// its own so that a `#load` inside it and a diagnostic about it both have
  /// somewhere to point (**C§3.1**).
  fn load_text_into(&self, module: ScopeId, path: &Path, text: &str) {
    let source = self.sources.add_string(path, text);
    self.admit_file(module, source, path);
  }

  /// Parses one already-loaded source as a file scope under `module` and walks
  /// it.
  fn admit_file(&self, module: ScopeId, source: SourceId, path: &Path) {
    let file = self.sources.file(source);
    let parsed = Arc::new(oj_syntax::parse(file.bytes(), source, self.interner));
    self
      .diagnostics
      .borrow_mut()
      .extend(parsed.diagnostics.iter().cloned());

    let scope = self.tree.push_scope(ScopeKind::File, Some(module));
    self.tree.set_file(scope, source, path);
    self
      .unit_of_source
      .borrow_mut()
      .insert(source, self.units.count());
    self.units.push(Unit {
      source,
      path: path.to_path_buf(),
      scope,
      parsed: Arc::clone(&parsed),
    });

    let mut target = DataTarget {
      scope,
      module: Some(module),
      visibility: Visibility::Export,
      conditional: false,
    };
    let NodeData::Block(block) = parsed.ast.data(parsed.root) else {
      return;
    };
    let statements = block.statements.clone();
    self.data_statements(&parsed, &statements, &mut target, source);
  }

  /// A module is loaded exactly once however it was reached, so its contents
  /// are not conditional even when the `#import` that pulled it in sits in a
  /// `#if` branch M3 could not decide.
  fn load_module_files(&self, module: ScopeId, entry: &Path, origin: Option<(SourceId, Span)>) {
    let conditional = self.conditional.replace(0);
    self.load_file_into(module, entry, origin);
    self.conditional.set(conditional);
  }

  fn path_of(&self, source: SourceId) -> PathBuf {
    if let Some(index) = self.unit_of_source.borrow().get(&source).copied() {
      return self.units[index].path.clone();
    }
    self
      .inserted_of_source
      .borrow()
      .get(&source)
      .map(|index| self.inserted[*index].path.clone())
      .unwrap_or_default()
  }

  // ------------------------------------------------------------ data scope ---

  fn data_statements(
    &self,
    parsed: &Parsed,
    statements: &[NodeId],
    target: &mut DataTarget,
    source: SourceId,
  ) {
    for statement in statements {
      self.data_statement(parsed, *statement, target, source);
    }
  }

  fn data_statement(
    &self,
    parsed: &Parsed,
    statement: NodeId,
    target: &mut DataTarget,
    source: SourceId,
  ) {
    match parsed.ast.data(statement) {
      NodeData::DirectiveScope { scope_type } => {
        target.visibility = match scope_type {
          ScopeType::Export => Visibility::Export,
          ScopeType::File => Visibility::File,
          ScopeType::Internal => Visibility::Module,
        };
      }
      NodeData::Declaration(_) => {
        self.declare(parsed, statement, target, source);
      }
      NodeData::CompoundDeclaration(_) => {
        self.compound_declaration(parsed, statement, target, source);
      }
      NodeData::Using(using) => {
        let expression = using.expression;
        let filter_type = using.filter_type;
        let filter = using.filter_expression;
        self.using(
          parsed,
          statement,
          expression,
          filter_type,
          filter,
          target,
          source,
        );
      }
      NodeData::DirectiveLoad { name } => {
        let name = name.clone();
        self.load(&name, statement, target, source);
      }
      NodeData::DirectiveImport(_) => {
        let destination = target.import_destination();
        self.import(parsed, statement, destination, None, source);
      }
      NodeData::Placeholder => {
        // `#placeholder NAME;` parses as a declaration whose value is the
        // placeholder marker; a bare marker has nothing to declare.
      }
      NodeData::DirectiveInsert(_) => {
        let kind = match target.module {
          Some(_) => InsertKind::Data(target.visibility),
          None => InsertKind::Members,
        };
        self.insert(parsed, statement, target.scope, kind, source);
      }
      NodeData::Struct(_) => self.anonymous_aggregate(parsed, statement, target, source),
      // An enum body's `#if` branches parse as imperative statements, so a bare
      // member name arrives here as an identifier rather than a declaration; in
      // an enum scope it is still a member (**L§6.10**, **L§9**).
      NodeData::Ident(ident) if self.tree.scope_kind(target.scope) == ScopeKind::Enum => {
        let (name, span) = (ident.name, parsed.ast.node(statement).span);
        let scope = target.scope;
        let conditional = self.is_conditional(target);
        let _ = self.tree.declare(Decl {
          name,
          scope,
          kind: DeclKind::Constant,
          visibility: Visibility::Export,
          flags: DeclarationFlags::IS_CONSTANT,
          source: Some(source),
          span,
          node: Some(statement),
          conditional,
          branch: self.branch.get(),
          overloadable: false,
        });
      }
      NodeData::If(_) => self.static_if(parsed, statement, target, source),
      NodeData::DirectiveModuleParameters(_) => {
        self.module_parameters(parsed, statement, target, source);
      }
      NodeData::DirectivePokeName { module, name } => {
        let (module, name) = (*module, *name);
        self.poke_name(parsed, module, name, target, source);
      }
      NodeData::DirectiveAddContext { expression } => {
        // The declaration joins the `Context` type, not this scope (**L§10.2**).
        let expression = *expression;
        self.walk(parsed, expression, target.scope, source);
      }
      _ => self.walk(parsed, statement, target.scope, source),
    }
  }

  fn declare(
    &self,
    parsed: &Parsed,
    node: NodeId,
    target: &mut DataTarget,
    source: SourceId,
  ) -> Option<DeclId> {
    let NodeData::Declaration(declaration) = parsed.ast.data(node) else {
      return None;
    };
    let declaration = declaration.clone();
    let declared = self.declare_name(parsed, node, &declaration, target, source);
    // A declaration with no name still has slots to resolve: `-> T` names no
    // return value, but `T` is looked up where the return was written
    // (**L§7.2**).
    if let Some(type_inst) = declaration.type_inst {
      self.walk(parsed, type_inst, target.scope, source);
    }
    // A named `#import` was already instantiated to give the name its meaning,
    // and unlike a bare one it adds nothing to this scope (**L§11.2**).
    if let Some(expression) = declaration.expression
      && !matches!(parsed.ast.data(expression), NodeData::DirectiveImport(_))
    {
      self.walk(parsed, expression, target.scope, source);
    }
    if let Some(alignment) = declaration.alignment_expression {
      self.walk(parsed, alignment, target.scope, source);
    }
    if let Some(library) = declaration.elsewhere_library {
      self.walk(parsed, library, target.scope, source);
    }
    declared
  }

  /// Puts the name a declaration introduces into its scope. A declaration that
  /// has none — an unnamed return value — introduces nothing.
  fn declare_name(
    &self,
    parsed: &Parsed,
    node: NodeId,
    declaration: &oj_syntax::ast::Declaration,
    target: &mut DataTarget,
    source: SourceId,
  ) -> Option<DeclId> {
    let declaration = declaration.clone();
    let name = declaration.name?;
    let NodeData::Ident(ident) = parsed.ast.data(name) else {
      return None;
    };
    let ident = ident.name;
    let span = parsed.ast.node(name).span;

    let kind = self.declaration_kind(parsed, &declaration, target, source);
    let overloadable = self.is_overloadable(parsed, &declaration, kind);
    // `` `name := … `` inside a macro declares into whichever block the macro
    // expands into (**L§7.13**), so the name is provisional here and expected
    // to turn up unresolved at the call sites until the macro expands.
    let backticked = declaration
      .flags
      .contains(DeclarationFlags::HAS_SCOPE_MODIFIER);
    if backticked {
      self.macro_injected.borrow_mut().insert(ident);
    }
    let destination = target.destination();
    let declared = self.tree.declare(Decl {
      name: ident,
      scope: destination,
      kind,
      visibility: target.visibility,
      flags: declaration.flags,
      source: Some(source),
      span,
      node: Some(node),
      conditional: self.is_conditional(target) || backticked,
      branch: self.branch.get(),
      overloadable,
    });

    if let Err(previous) = declared {
      let previous = self.tree.decl(previous);
      self.error(
        source,
        span,
        format!(
          "Redeclared identifier '{}'.",
          self.interner.resolve_lossy(ident)
        ),
      );
      if let Some(other) = previous.source {
        self.report(Diagnostic::info(
          other,
          previous.span,
          "Here is the previous declaration.",
        ));
      }
    }
    declared.ok()
  }

  /// What a name stands for, judged from the shape of its value alone; a named
  /// `#import` also instantiates the module it binds.
  fn declaration_kind(
    &self,
    parsed: &Parsed,
    declaration: &oj_syntax::ast::Declaration,
    target: &DataTarget,
    source: SourceId,
  ) -> DeclKind {
    let Some(expression) = declaration.expression else {
      return self.plain_kind(declaration.flags);
    };
    match parsed.ast.data(expression) {
      NodeData::ProcedureHeader(_) => DeclKind::Procedure,
      NodeData::Struct(_) => DeclKind::Struct,
      NodeData::Enum(_) => DeclKind::Enum,
      NodeData::Placeholder => DeclKind::Placeholder,
      NodeData::DirectiveImport(_) => {
        match self.instantiate(parsed, expression, target.scope, source) {
          Some(module) => DeclKind::Module(module),
          None => DeclKind::Constant,
        }
      }
      _ => self.plain_kind(declaration.flags),
    }
  }

  /// Whether this name may join an overload set. Procedures do (**L§7.7**), and
  /// so does a constant that merely names one — `print :: print_to_builder;` is
  /// an alias, and only its type would say so, so a constant whose value is a
  /// bare name or a bake is admitted rather than called a redeclaration.
  fn is_overloadable(
    &self,
    parsed: &Parsed,
    declaration: &oj_syntax::ast::Declaration,
    kind: DeclKind,
  ) -> bool {
    if kind == DeclKind::Procedure {
      return true;
    }
    if kind != DeclKind::Constant {
      return false;
    }
    declaration.expression.is_some_and(|expression| {
      matches!(
        parsed.ast.data(expression),
        NodeData::Ident(_) | NodeData::DirectiveBake { .. }
      )
    })
  }

  /// A declaration is conditional when only one of several `#if` branches will
  /// survive, so it neither collides with a sibling nor with a real one. A
  /// backticked declaration inside a macro is the same case seen from the other
  /// side: it belongs to whichever block the macro expands into (**L§7.13**),
  /// so its presence here is provisional.
  fn is_conditional(&self, target: &DataTarget) -> bool {
    target.conditional || self.conditional.get() > 0
  }

  fn plain_kind(&self, flags: DeclarationFlags) -> DeclKind {
    if flags.contains(DeclarationFlags::IS_CONSTANT) {
      DeclKind::Constant
    } else {
      DeclKind::Variable
    }
  }

  /// `a, b := 1, 2;` and `a, d:, c = 4, 5, 6;` (**L§4.5**). Which names are
  /// declared depends on the statement's own kind and each name's modifier:
  /// in a declaration `b=` assigns to an existing name, and in an assignment
  /// `d:` declares a new one.
  fn compound_declaration(
    &self,
    parsed: &Parsed,
    node: NodeId,
    target: &mut DataTarget,
    source: SourceId,
  ) {
    let NodeData::CompoundDeclaration(compound) = parsed.ast.data(node) else {
      return;
    };
    let (properties, operator) = (compound.declaration_properties, compound.operator_type);
    let NodeData::CommaSeparatedArguments { arguments } =
      parsed.ast.data(compound.comma_separated_assignment)
    else {
      return;
    };
    let entries: Vec<(NodeId, oj_syntax::ast::CommaModifier)> = arguments
      .iter()
      .map(|argument| (argument.node, argument.modifier))
      .collect();
    let (flags, is_declaration) = match parsed.ast.data(properties) {
      NodeData::Declaration(declaration) => (declaration.flags, true),
      _ => (DeclarationFlags::empty(), false),
    };
    // `Code_Compound_Declaration.operator_type` is 0 for a declaration and the
    // assignment operator otherwise (**L§4.5**).
    let is_declaration = is_declaration && operator.is_none();

    for (entry, modifier) in entries {
      let declares = match modifier {
        oj_syntax::ast::CommaModifier::Declare => true,
        oj_syntax::ast::CommaModifier::Assign => false,
        oj_syntax::ast::CommaModifier::None => is_declaration,
      };
      match parsed.ast.data(entry) {
        NodeData::Ident(ident) if declares => {
          let (name, span) = (ident.name, parsed.ast.node(entry).span);
          let destination = target.destination();
          let kind = self.plain_kind(flags);
          let declared = self.tree.declare(Decl {
            name,
            scope: destination,
            kind,
            visibility: target.visibility,
            flags,
            source: Some(source),
            span,
            node: Some(entry),
            conditional: self.is_conditional(target),
            branch: self.branch.get(),
            overloadable: false,
          });
          if declared.is_err() {
            self.error(
              source,
              span,
              format!(
                "Redeclared identifier '{}'.",
                self.interner.resolve_lossy(name)
              ),
            );
          }
        }
        _ => self.walk(parsed, entry, target.scope, source),
      }
    }
    if let NodeData::Declaration(declaration) = parsed.ast.data(properties) {
      let (type_inst, expression) = (declaration.type_inst, declaration.expression);
      if let Some(type_inst) = type_inst {
        self.walk(parsed, type_inst, target.scope, source);
      }
      if let Some(expression) = expression {
        self.walk(parsed, expression, target.scope, source);
      }
    }
  }

  #[allow(clippy::too_many_arguments)]
  fn using(
    &self,
    parsed: &Parsed,
    statement: NodeId,
    expression: NodeId,
    filter_type: FilterType,
    filter: Option<NodeId>,
    target: &mut DataTarget,
    source: SourceId,
  ) {
    let destination = target.destination();
    let span = parsed.ast.node(statement).span;

    // `using X :: #import "X";` and `using #import "X";` widen the scope with
    // the module's exports; anything else needs the type of the used value, so
    // the scope records that names may still arrive (**L§4.3**).
    let import = match parsed.ast.data(expression) {
      NodeData::DirectiveImport(_) => Some(expression),
      NodeData::Declaration(declaration) => declaration
        .expression
        .filter(|value| matches!(parsed.ast.data(*value), NodeData::DirectiveImport(_))),
      _ => None,
    };

    let declared = match parsed.ast.data(expression) {
      NodeData::Declaration(_) => self.declare(parsed, expression, target, source),
      _ => None,
    };

    match import {
      Some(import) => {
        let filter = self.name_filter(parsed, filter_type, filter);
        if let Some(module) = self.instantiate(parsed, import, target.scope, source) {
          let (only, except) = filter;
          self.tree.add_import(
            target.import_destination(),
            ImportEdge {
              target: module,
              only,
              except,
              span,
              source: Some(source),
              transitive: false,
            },
          );
        }
      }
      None => {
        if !matches!(parsed.ast.data(expression), NodeData::Declaration(_)) {
          self.walk(parsed, expression, target.scope, source);
        }
        // `using E :: enum { … };` and `using S :: struct { … };` widen the
        // scope with names the parser already knows; a `using` of a *value*
        // needs its type, so the scope only records that names may still
        // arrive (**L§4.3**, **L§6.8**).
        match self.used_aggregate(parsed, expression, source) {
          Some(aggregate) => {
            let (only, except) = self.name_filter(parsed, filter_type, filter);
            self.tree.add_import(
              destination,
              ImportEdge {
                target: aggregate,
                only,
                except,
                span,
                source: Some(source),
                transitive: true,
              },
            );
          }
          // A `using` of a value: its members are names in this scope, but
          // only the typechecker can say which (**L§6.8**), so the scope
          // records the value and that names may still arrive (**L§4.3**).
          None => {
            let (only, except) = self.name_filter(parsed, filter_type, filter);
            self.tree.add_used_value(
              destination,
              UsedValue {
                source,
                decl: declared,
                expression,
                only,
                except,
              },
            );
            self.tree.add_pending(destination, PendingProvider::Using);
          }
        }
      }
    }
  }

  /// The member scope of `using E :: enum { … }` / `using S :: struct { … }`.
  fn used_aggregate(
    &self,
    parsed: &Parsed,
    expression: NodeId,
    source: SourceId,
  ) -> Option<ScopeId> {
    let NodeData::Declaration(declaration) = parsed.ast.data(expression) else {
      return None;
    };
    let value = declaration.expression?;
    if !matches!(
      parsed.ast.data(value),
      NodeData::Enum(_) | NodeData::Struct(_)
    ) {
      return None;
    }
    self
      .aggregate_scopes
      .borrow()
      .get(&(source, value))
      .copied()
  }

  /// `using,only(a, b)` / `using,except(Node)` name lists (**L§6.8**).
  fn name_filter(
    &self,
    parsed: &Parsed,
    filter_type: FilterType,
    filter: Option<NodeId>,
  ) -> (Vec<Symbol>, Vec<Symbol>) {
    let Some(filter) = filter else {
      return (Vec::new(), Vec::new());
    };
    let mut names = Vec::new();
    collect_names(parsed, filter, &mut names);
    match filter_type {
      FilterType::Only => (names, Vec::new()),
      FilterType::Except => (Vec::new(), names),
      FilterType::None | FilterType::Map => (Vec::new(), Vec::new()),
    }
  }

  fn load(&self, name: &[u8], statement: NodeId, target: &mut DataTarget, source: SourceId) {
    let span = self.span_of(source, statement);
    let Some(module) = target
      .module
      .or_else(|| self.tree.enclosing_module(target.scope))
    else {
      return;
    };
    if !self.options.follow_imports {
      self.tree.add_pending(target.scope, PendingProvider::Insert);
      return;
    }
    let Ok(relative) = std::str::from_utf8(name) else {
      self.error(source, span, "A #load path must be valid UTF-8.");
      return;
    };
    let from = self.path_of(source);
    let path = oj_source::resolve_load(&from, relative);
    self.load_file_into(module, &path, Some((source, span)));
  }

  fn span_of(&self, source: SourceId, node: NodeId) -> Span {
    self
      .ast_of(source)
      .map(|ast| ast.node(node).span)
      .unwrap_or_else(|| Span::at(0))
  }

  // -------------------------------------------------------------- imports ---

  fn import(
    &self,
    parsed: &Parsed,
    node: NodeId,
    destination: ScopeId,
    filter: Option<(Vec<Symbol>, Vec<Symbol>)>,
    source: SourceId,
  ) {
    let span = parsed.ast.node(node).span;
    let Some(module) = self.instantiate(parsed, node, destination, source) else {
      return;
    };
    let (only, except) = filter.unwrap_or_default();
    self.tree.add_import(
      destination,
      ImportEdge {
        target: module,
        only,
        except,
        span,
        source: Some(source),
        transitive: false,
      },
    );
  }

  /// Finds or creates the module scope a `#import` names. Instantiations are
  /// shared by (entry file, textual parameter list) so that two imports with
  /// the same arguments see the same globals (**L§11.2**).
  fn instantiate(
    &self,
    parsed: &Parsed,
    node: NodeId,
    scope: ScopeId,
    source: SourceId,
  ) -> Option<ScopeId> {
    let NodeData::DirectiveImport(import) = parsed.ast.data(node) else {
      return None;
    };
    let import = import.clone();
    let span = parsed.ast.node(node).span;

    if let Some(parameters) = &import.module_parameters {
      for argument in parameters {
        self.walk(parsed, argument.expression, scope, source);
      }
    }
    if let Some(parameters) = &import.program_parameters {
      for argument in parameters {
        self.walk(parsed, argument.expression, scope, source);
      }
    }

    if !self.options.follow_imports {
      self.tree.add_pending(scope, PendingProvider::FailedImport);
      return None;
    }

    let from = self.path_of(source);
    let mut name = String::from_utf8_lossy(&import.name).into_owned();
    // A metaprogram may have said this import is somebody else, or is nobody
    // at all (**C§3.3**).
    if import.import_type == ImportType::ShortName {
      match self.remapped(scope, &name) {
        Some(Some(replacement)) => name = replacement,
        Some(None) => {
          self.tree.add_pending(scope, PendingProvider::FailedImport);
          return None;
        }
        None => {}
      }
    }
    let resolved = match import.import_type {
      ImportType::ShortName => match oj_source::resolve_module(&name, &self.import_path) {
        Ok(resolved) => resolved,
        Err(error) => {
          self.error(source, span, error.to_string());
          if matches!(error, ModuleError::NotFound { .. }) {
            self.tree.add_pending(scope, PendingProvider::FailedImport);
          }
          return None;
        }
      },
      ImportType::PathToFile => oj_source::resolve_file_module(&from, &name),
      ImportType::PathToDirectory => oj_source::resolve_directory_module(&from, &name),
      ImportType::FullText => {
        // `#import,string "code"` has no file to resolve; M6 compiles the text
        // once compile-time execution can produce it.
        self.tree.add_pending(scope, PendingProvider::FailedImport);
        return None;
      }
    };

    let unshared = import.flags.contains(oj_syntax::ast::ImportFlags::UNSHARED);
    let instance = if unshared {
      self
        .unshared_instances
        .set(self.unshared_instances.get() + 1);
      self.unshared_instances.get()
    } else {
      0
    };
    let key = ModuleKey {
      entry: resolved.entry.clone(),
      parameters: self.parameter_text(parsed, &import, source),
      instance,
    };
    if let Some(existing) = self.modules.borrow().get(&key).copied() {
      return Some(existing);
    }

    let module = self.tree.push_scope(ScopeKind::Module, Some(self.preload));
    self.modules.borrow_mut().insert(key, module);
    self.record_module(name, resolved.entry.clone(), module, ModuleKind::File);
    self.load_module_files(module, &resolved.entry, Some((source, span)));
    Some(module)
  }

  /// What `#import "name"` written in `scope` should import instead, if a
  /// metaprogram remapped it (**C§3.3**). `Some(None)` means the import is
  /// blocked: the compiler does not even look for it.
  fn remapped(&self, scope: ScopeId, name: &str) -> Option<Option<String>> {
    if self.options.import_remaps.is_empty() {
      return None;
    }
    let host = self
      .tree
      .enclosing_module(scope)
      .and_then(|module| {
        self
          .module_records
          .borrow()
          .iter()
          .find(|record| record.scope == module)
          .map(|record| record.name.clone())
      })
      .unwrap_or_default();
    let remap = self
      .options
      .import_remaps
      .iter()
      .find(|remap| remap.matches(&host, name))?;
    Some((!remap.replacement.is_empty()).then(|| remap.replacement.clone()))
  }

  /// The textual argument list that keys an instantiation, taken from the
  /// source so that two spellings of the same value stay distinct exactly as
  /// they do in the reference.
  fn parameter_text(
    &self,
    parsed: &Parsed,
    import: &oj_syntax::ast::DirectiveImport,
    source: SourceId,
  ) -> String {
    let file = self.sources.file(source);
    let mut text = String::new();
    let mut write = |arguments: &Vec<Argument>| {
      text.push('(');
      for argument in arguments {
        let span = parsed.ast.node(argument.expression).span;
        text.push_str(&String::from_utf8_lossy(file.text_of(span)));
        text.push(',');
      }
      text.push(')');
    };
    // Only the *module* parameters distinguish instantiations, and an empty
    // list supplies nothing, so `#import "Basic"()(…)` is the same
    // instantiation as a bare `#import "Basic"`. The program parameters are
    // set once by the main program and shared by every import of the module,
    // wherever it comes from (**L§11.3**).
    if let Some(parameters) = import
      .module_parameters
      .as_ref()
      .filter(|parameters| !parameters.is_empty())
    {
      write(parameters);
    }
    text
  }

  fn module_parameters(
    &self,
    parsed: &Parsed,
    node: NodeId,
    target: &mut DataTarget,
    source: SourceId,
  ) {
    let NodeData::DirectiveModuleParameters(parameters) = parsed.ast.data(node) else {
      return;
    };
    let (module_parameters, program_parameters, common_code) = (
      parameters.module_parameters,
      parameters.program_parameters,
      parameters.common_code,
    );

    // Module and program parameters are constants of the module scope, not
    // exported to importers (**L§11.3**).
    let destination = target.module.unwrap_or(target.scope);
    let mut inner = DataTarget {
      scope: destination,
      module: None,
      visibility: Visibility::Module,
      conditional: target.conditional,
    };
    for header in [Some(module_parameters), program_parameters]
      .into_iter()
      .flatten()
    {
      let NodeData::ProcedureHeader(header) = parsed.ast.data(header) else {
        continue;
      };
      let arguments = header.arguments.clone();
      for argument in arguments {
        self.declare_parameter(parsed, argument, &mut inner, source);
      }
    }
    if let Some(common_code) = common_code {
      self.walk(parsed, common_code, destination, source);
    }
  }

  fn poke_name(
    &self,
    parsed: &Parsed,
    module: NodeId,
    name: NodeId,
    target: &mut DataTarget,
    source: SourceId,
  ) {
    // `#poke_name Module ident;` injects `ident` into the module bound by the
    // named import (**L§11.9**).
    let (NodeData::Ident(module_name), NodeData::Ident(poked)) =
      (parsed.ast.data(module), parsed.ast.data(name))
    else {
      return;
    };
    let (module_name, poked) = (module_name.name, poked.name);
    let span = parsed.ast.node(name).span;

    let destination = match self.tree.lookup(target.scope, module_name) {
      crate::tree::Resolution::Found(candidates) => {
        candidates
          .iter()
          .find_map(|id| match self.tree.decl(*id).kind {
            DeclKind::Module(scope) => Some(scope),
            _ => None,
          })
      }
      _ => None,
    };
    let Some(destination) = destination else {
      self.tree.add_pending(target.scope, PendingProvider::Insert);
      return;
    };
    let _ = self.tree.declare(Decl {
      name: poked,
      scope: destination,
      kind: DeclKind::Placeholder,
      visibility: Visibility::Export,
      flags: DeclarationFlags::empty(),
      source: Some(source),
      span,
      node: Some(name),
      conditional: true,
      branch: None,
      overloadable: false,
    });
  }

  // ------------------------------------------------------------ static if ---

  fn static_if(&self, parsed: &Parsed, node: NodeId, target: &mut DataTarget, source: SourceId) {
    let NodeData::If(payload) = parsed.ast.data(node) else {
      return;
    };
    if !payload
      .if_flags
      .contains(oj_syntax::ast::IfFlags::IS_STATIC)
    {
      self.walk(parsed, node, target.scope, source);
      return;
    }
    let condition = payload.condition;
    self.walk(parsed, condition, target.scope, source);

    match self.static_branch(parsed, node, target.scope, source) {
      Some(statements) => self.data_statements(parsed, &statements, target, source),
      None => self.pending_ifs.borrow_mut().push(PendingIf::Data {
        node,
        target: *target,
        source,
      }),
    }
  }

  /// The statements a static `#if` selects, or `None` while its condition is
  /// still undecidable. A `#if` branch does not create a scope: what it
  /// declares lands in the enclosing one (**L§4.2**).
  fn static_branch(
    &self,
    parsed: &Parsed,
    node: NodeId,
    scope: ScopeId,
    source: SourceId,
  ) -> Option<Vec<NodeId>> {
    let NodeData::If(payload) = parsed.ast.data(node) else {
      return Some(Vec::new());
    };
    if payload
      .if_flags
      .contains(oj_syntax::ast::IfFlags::IS_SWITCH_STATEMENT)
    {
      return self.static_switch_branch(parsed, &payload.clone(), scope, source);
    }

    let taken = self.fold_condition(scope, source, payload.condition)?;
    let branch = if taken {
      payload.then_block
    } else {
      payload.else_block
    };
    Some(
      branch
        .map(|branch| self.branch_statements(parsed, branch))
        .unwrap_or_default(),
    )
  }

  /// `#if x == { case a; … case; … }` (**L§6.10**): the first case whose value
  /// equals the subject is taken, `#through` continues into the next one, and a
  /// case without a value is the default.
  fn static_switch_branch(
    &self,
    parsed: &Parsed,
    payload: &oj_syntax::ast::IfNode,
    scope: ScopeId,
    source: SourceId,
  ) -> Option<Vec<NodeId>> {
    let subject = self.fold(scope, source, payload.condition)?;
    let cases = payload
      .then_block
      .map(|block| self.branch_statements(parsed, block))
      .unwrap_or_default();

    let mut taken = Vec::new();
    let mut matching = false;
    let mut default = None;

    for case in cases {
      let NodeData::Case(entry) = parsed.ast.data(case) else {
        continue;
      };
      if !matching {
        match entry.condition {
          Some(condition) => {
            if self.fold(scope, source, condition)? != subject {
              continue;
            }
            matching = true;
          }
          None => {
            default = Some(entry.then_block);
            continue;
          }
        }
      }
      taken.extend(self.branch_statements(parsed, entry.then_block));
      if !entry.marked_as_fallthrough {
        return Some(taken);
      }
    }

    if matching {
      return Some(taken);
    }
    Some(
      default
        .map(|block| self.branch_statements(parsed, block))
        .unwrap_or_default(),
    )
  }

  /// Every statement of every branch, for a `#if` that stayed undecidable.
  fn all_branch_statements(&self, parsed: &Parsed, node: NodeId) -> Vec<Vec<NodeId>> {
    let NodeData::If(payload) = parsed.ast.data(node) else {
      return Vec::new();
    };
    let mut branches = Vec::new();
    for block in [payload.then_block, payload.else_block]
      .into_iter()
      .flatten()
    {
      let statements = self.branch_statements(parsed, block);
      if payload
        .if_flags
        .contains(oj_syntax::ast::IfFlags::IS_SWITCH_STATEMENT)
      {
        for case in statements {
          if let NodeData::Case(entry) = parsed.ast.data(case) {
            branches.push(self.branch_statements(parsed, entry.then_block));
          }
        }
      } else {
        branches.push(statements);
      }
    }
    branches
  }

  fn branch_statements(&self, parsed: &Parsed, branch: NodeId) -> Vec<NodeId> {
    match parsed.ast.data(branch) {
      NodeData::Block(block) => block.statements.clone(),
      _ => vec![branch],
    }
  }

  /// Retries the `#if`s whose conditions were not yet decidable until nothing
  /// more folds, then admits every branch of what is left.
  fn settle_pending_ifs(&self) {
    loop {
      let pending = std::mem::take(&mut *self.pending_ifs.borrow_mut());
      if pending.is_empty() {
        return;
      }
      let before = pending.len();
      let mut still_pending = Vec::new();

      for item in pending {
        let (node, scope, source) = match &item {
          PendingIf::Data {
            node,
            target,
            source,
          } => (*node, target.scope, *source),
          PendingIf::Imperative {
            node,
            scope,
            source,
          } => (*node, *scope, *source),
        };
        let Some(parsed) = self.parsed_of(source) else {
          continue;
        };
        match self.static_branch(&parsed, node, scope, source) {
          Some(statements) => self.resume_static_if(&parsed, item, &statements),
          None => still_pending.push(item),
        }
      }

      if still_pending.len() == before {
        for item in still_pending {
          self.admit_all_branches(item);
        }
        continue;
      }
      self.pending_ifs.borrow_mut().extend(still_pending);
    }
  }

  fn resume_static_if(&self, parsed: &Parsed, item: PendingIf, statements: &[NodeId]) {
    match item {
      PendingIf::Data {
        mut target, source, ..
      } => self.data_statements(parsed, statements, &mut target, source),
      PendingIf::Imperative { scope, source, .. } => {
        for statement in statements {
          self.imperative_statement(parsed, *statement, scope, source);
        }
      }
    }
  }

  /// A `#if` M3 cannot decide still declares names, and the scope has to see
  /// all of them: every branch is admitted as conditional declarations, which
  /// neither collide with each other nor hide a real declaration, and the scope
  /// is marked as still able to gain names (**L§4.3**).
  fn admit_all_branches(&self, item: PendingIf) {
    let (node, source) = match &item {
      PendingIf::Data { node, source, .. } => (*node, *source),
      PendingIf::Imperative { node, source, .. } => (*node, *source),
    };
    let Some(parsed) = self.parsed_of(source) else {
      return;
    };
    let branches = self.labelled_branches(&parsed, node, source);
    self.speculative.set(self.speculative.get() + 1);
    self.conditional.set(self.conditional.get() + 1);
    let outer = self.branch.get();

    match item {
      PendingIf::Data {
        mut target, source, ..
      } => {
        self
          .tree
          .add_pending(target.destination(), PendingProvider::StaticIf);
        target.conditional = true;
        for (branch, statements) in branches {
          self.branch.set(branch);
          let mut branch_target = target;
          self.data_statements(&parsed, &statements, &mut branch_target, source);
        }
      }
      PendingIf::Imperative { scope, source, .. } => {
        self.tree.add_pending(scope, PendingProvider::StaticIf);
        for (branch, statements) in branches {
          self.branch.set(branch);
          for statement in statements {
            self.imperative_statement(&parsed, statement, scope, source);
          }
        }
      }
    }
    self.branch.set(outer);
    self.speculative.set(self.speculative.get() - 1);
    self.conditional.set(self.conditional.get() - 1);
  }

  /// The statements of each branch, each labelled with the branch it is — so
  /// that a declaration remembers which one admitted it (**L§6.10**). A `#if
  /// x == { case … }` has no two-way block to name, so its cases go unlabelled
  /// and stay admitted.
  fn labelled_branches(
    &self,
    parsed: &Parsed,
    node: NodeId,
    source: SourceId,
  ) -> Vec<(Option<Branch>, Vec<NodeId>)> {
    let NodeData::If(payload) = parsed.ast.data(node) else {
      return Vec::new();
    };
    if payload
      .if_flags
      .contains(oj_syntax::ast::IfFlags::IS_SWITCH_STATEMENT)
    {
      return self
        .all_branch_statements(parsed, node)
        .into_iter()
        .map(|statements| (None, statements))
        .collect();
    }
    [payload.then_block, payload.else_block]
      .into_iter()
      .flatten()
      .map(|block| {
        (
          Some(Branch {
            source,
            node,
            block,
          }),
          self.branch_statements(parsed, block),
        )
      })
      .collect()
  }

  fn parsed_of(&self, source: SourceId) -> Option<Arc<Parsed>> {
    if let Some(index) = self.unit_of_source.borrow().get(&source).copied() {
      return Some(Arc::clone(&self.units[index].parsed));
    }
    let index = *self.inserted_of_source.borrow().get(&source)?;
    Some(Arc::clone(&self.inserted[index].parsed))
  }

  fn fold_condition(&self, scope: ScopeId, source: SourceId, node: NodeId) -> Option<bool> {
    self.fold(scope, source, node)?.truth()
  }

  fn fold(&self, scope: ScopeId, source: SourceId, node: NodeId) -> Option<ConstValue> {
    Evaluator::new(&self.tree, self, &self.builtins.borrow(), self.interner)
      .eval(scope, source, node)
  }

  // --------------------------------------------------------------- insert ---

  /// Every module instantiation the program made, in order (**C§3.2**).
  pub fn modules(&self) -> Vec<Module> {
    self.module_records.borrow().clone()
  }

  fn record_module(&self, name: String, entry: PathBuf, scope: ScopeId, kind: ModuleKind) {
    self.module_records.borrow_mut().push(Module {
      name,
      entry,
      scope,
      kind,
    });
  }

  /// The `#insert`s still waiting for the typechecker to say what their text
  /// is (**L§13.2**).
  pub fn pending_inserts(&self) -> Vec<PendingInsert> {
    self.pending_inserts.borrow().clone()
  }

  /// The program an `#insert` expanded to, when one has expanded (**L§13.2**).
  pub fn expansion_of(&self, source: SourceId, node: NodeId) -> Option<Expansion> {
    self.expansions.borrow().get(&(source, node)).copied()
  }

  /// Parses `text` as the statements an `#insert` stands for and admits its
  /// declarations into `scope`, which is the mutable program of **L§13.2**: the
  /// names it declares are visible from the insertion point on, and the block
  /// it returns is what the back end lowers in the `#insert`'s place.
  ///
  /// The string is lexed under the path of the file the `#insert` was written
  /// in, so a `#load` inside it resolves the way one written there would.
  /// Expanding the same `#insert` twice is not possible: the first expansion is
  /// remembered and handed back.
  pub fn insert_source(
    &self,
    scope: ScopeId,
    kind: InsertKind,
    at: (SourceId, NodeId),
    text: &[u8],
  ) -> Option<Expansion> {
    if let Some(existing) = self.expansion_of(at.0, at.1) {
      return Some(existing);
    }

    let path = self.path_of(at.0);
    let source = self.sources.add_bytes(path.clone(), text.to_vec());
    let file = self.sources.file(source);
    let parsed = Arc::new(oj_syntax::parse(file.bytes(), source, self.interner));
    self
      .diagnostics
      .borrow_mut()
      .extend(parsed.diagnostics.iter().cloned());

    self
      .inserted_of_source
      .borrow_mut()
      .insert(source, self.inserted.count());
    self.inserted.push(Insertion {
      path,
      scope,
      parsed: Arc::clone(&parsed),
    });

    let expansion = Expansion {
      source,
      root: parsed.root,
      scope,
    };
    self.expansions.borrow_mut().insert(at, expansion);

    let NodeData::Block(block) = parsed.ast.data(parsed.root) else {
      return Some(expansion);
    };
    let statements = block.statements.clone();
    match kind {
      InsertKind::Data(visibility) => {
        let mut target = DataTarget {
          scope,
          module: self.tree.enclosing_module(scope),
          visibility,
          conditional: self.conditional.get() > 0,
        };
        self.data_statements(&parsed, &statements, &mut target, source);
      }
      InsertKind::Members => {
        let mut target = DataTarget::nested(scope);
        self.data_statements(&parsed, &statements, &mut target, source);
      }
      InsertKind::Imperative => {
        for statement in &statements {
          self.imperative_statement(&parsed, *statement, scope, source);
        }
      }
      InsertKind::Expression => {
        if statements.len() != 1 {
          self.error(
            at.0,
            self.span_of(at.0, at.1),
            "An #insert in expression position must be one expression statement.",
          );
        }
        for statement in &statements {
          self.walk(&parsed, *statement, scope, source);
        }
      }
    }
    Some(expansion)
  }

  /// Expands an `#insert` whose operand already folds to a string, which is
  /// every one whose text does not have to wait for a type or a `#run`. The
  /// rest keep the scope waiting (**L§4.3**) until the typechecker gets to
  /// them.
  fn insert(
    &self,
    parsed: &Parsed,
    node: NodeId,
    scope: ScopeId,
    kind: InsertKind,
    source: SourceId,
  ) {
    let NodeData::DirectiveInsert(insert) = parsed.ast.data(node) else {
      return;
    };
    let expression = insert.expression;
    if let Some(ConstValue::String(text)) = self.fold(scope, source, expression) {
      self.insert_source(scope, kind, (source, node), &text);
      return;
    }
    // An `#insert` may declare anything; without a `#placeholder` the
    // reference does not wait for it, but it may still provide names
    // (**L§11.7**).
    self.tree.add_pending(scope, PendingProvider::Insert);
    self.pending_inserts.borrow_mut().push(PendingInsert {
      source,
      node,
      scope,
      kind,
    });
    self.walk(parsed, expression, scope, source);
  }
  // ------------------------------------------------------- diagnostics ------

  fn error(&self, source: SourceId, span: Span, message: impl Into<String>) {
    self.report(Diagnostic::error(source, span, message));
  }

  fn report(&self, diagnostic: Diagnostic) {
    if self.speculative.get() == 0 {
      self.diagnostics.borrow_mut().push(diagnostic);
    }
  }
}

fn collect_names(parsed: &Parsed, node: NodeId, names: &mut Vec<Symbol>) {
  match parsed.ast.data(node) {
    NodeData::Ident(ident) => names.push(ident.name),
    NodeData::BinaryOperator { left, right, .. } => {
      collect_names(parsed, *left, names);
      collect_names(parsed, *right, names);
    }
    NodeData::CommaSeparatedArguments { arguments } => {
      for argument in arguments {
        collect_names(parsed, argument.node, names);
      }
    }
    NodeData::ProcedureCall(call) => {
      for argument in &call.arguments {
        collect_names(parsed, argument.expression, names);
      }
    }
    _ => {}
  }
}

// ---------------------------------------------------------------------------
// Nested scopes and the expression walk
// ---------------------------------------------------------------------------

impl Program<'_> {
  /// Walks one node, opening the scopes it introduces and recording every
  /// identifier that has to resolve.
  fn walk(&self, parsed: &Parsed, node: NodeId, scope: ScopeId, source: SourceId) {
    match parsed.ast.data(node) {
      NodeData::Block(block) => {
        let (block_type, statements) = (block.block_type, block.statements.clone());
        if block_type == oj_syntax::ast::BlockType::Imperative {
          let inner = self.tree.push_scope(ScopeKind::Imperative, Some(scope));
          for statement in statements {
            self.imperative_statement(parsed, statement, inner, source);
          }
        } else {
          let mut target = DataTarget::nested(scope);
          self.data_statements(parsed, &statements, &mut target, source);
        }
      }
      NodeData::Literal(literal) => match &literal.value {
        oj_syntax::ast::LiteralValue::Array(array) => {
          let (element_type, members) = (array.element_type, array.members.clone());
          self.walk_all(
            parsed,
            element_type.into_iter().chain(members),
            scope,
            source,
          );
        }
        oj_syntax::ast::LiteralValue::Struct(structure) => {
          let type_expression = structure.type_expression;
          let arguments: Vec<NodeId> = structure
            .arguments
            .iter()
            .map(|argument| argument.expression)
            .collect();
          self.walk_all(parsed, type_expression, scope, source);
          // `Body.{name = "Ginger", values[1] = 7}`: the left of a field
          // assignment names a member of the literal's type (**L§5.7**).
          for argument in arguments {
            self.assignment_value(parsed, argument, scope, source);
          }
        }
        _ => {}
      },
      NodeData::Ident(ident) => {
        let (name, flags) = (ident.name, ident.flags);
        self.reference(parsed, node, name, flags, scope, source);
      }
      NodeData::UnaryOperator { operator, operand } => {
        // `.NAME` is an enum member of a type only the typechecker knows
        // (**L§5.12**), so its operand never resolves as an identifier.
        if *operator != oj_syntax::ast::OperatorType::DOT {
          let operand = *operand;
          self.walk(parsed, operand, scope, source);
        }
      }
      NodeData::BinaryOperator {
        operator,
        left,
        right,
        ..
      } => {
        let (operator, left, right) = (*operator, *left, *right);
        self.walk(parsed, left, scope, source);
        // The right side of `.` is a member name, resolved against the left
        // side's type rather than against any scope (**L§4.3**).
        if operator != oj_syntax::ast::OperatorType::DOT {
          self.walk(parsed, right, scope, source);
        }
      }
      NodeData::ProcedureHeader(_) => self.procedure(parsed, node, scope, source),
      NodeData::ProcedureBody { header, .. } => {
        let header = *header;
        self.procedure(parsed, header, scope, source);
      }
      NodeData::ProcedureCall(call) => {
        let procedure = call.procedure_expression;
        let arguments: Vec<NodeId> = call
          .arguments
          .iter()
          .map(|argument| argument.expression)
          .collect();
        let context = call.context_modification.clone().unwrap_or_default();
        self.walk_all(
          parsed,
          [procedure].into_iter().chain(arguments),
          scope,
          source,
        );
        for entry in context {
          self.assignment_value(parsed, entry, scope, source);
        }
      }
      NodeData::While { condition, block } => {
        let (condition, block) = (*condition, *block);
        self.walk(parsed, condition, scope, source);
        self.body(parsed, block, scope, source);
      }
      NodeData::If(payload) => {
        let payload = payload.clone();
        if payload
          .if_flags
          .contains(oj_syntax::ast::IfFlags::IS_STATIC)
        {
          self.walk(parsed, payload.condition, scope, source);
          match self.static_branch(parsed, node, scope, source) {
            Some(statements) => {
              for statement in statements {
                self.imperative_statement(parsed, statement, scope, source);
              }
            }
            None => self.pending_ifs.borrow_mut().push(PendingIf::Imperative {
              node,
              scope,
              source,
            }),
          }
          return;
        }
        self.walk(parsed, payload.condition, scope, source);
        for branch in [payload.then_block, payload.else_block]
          .into_iter()
          .flatten()
        {
          self.body(parsed, branch, scope, source);
        }
      }
      NodeData::LoopControl { .. } => {}
      NodeData::Case(case) => {
        let (condition, then_block) = (case.condition, case.then_block);
        if let Some(condition) = condition {
          self.walk(parsed, condition, scope, source);
        }
        self.body(parsed, then_block, scope, source);
      }
      NodeData::Return { arguments, .. } => {
        let arguments: Vec<NodeId> = arguments
          .iter()
          .map(|argument| argument.expression)
          .collect();
        self.walk_all(parsed, arguments, scope, source);
      }
      NodeData::For(payload) => {
        let payload = payload.clone();
        self.for_loop(parsed, node, &payload, scope, source);
      }
      NodeData::TypeInstantiation(inst) => {
        let inst = inst.clone();
        self.walk_all(
          parsed,
          [
            inst.type_valued_expression,
            inst.must_implement,
            inst.pointer_to,
            inst.type_directive_target,
            inst.array_element_type,
            inst.array_dimension,
          ]
          .into_iter()
          .flatten(),
          scope,
          source,
        );
      }
      NodeData::Enum(_) => self.enumeration(parsed, node, scope, source),
      NodeData::Struct(_) => self.structure(parsed, node, scope, source),
      NodeData::CommaSeparatedArguments { arguments } => {
        let arguments: Vec<NodeId> = arguments.iter().map(|argument| argument.node).collect();
        self.walk_all(parsed, arguments, scope, source);
      }
      NodeData::DirectiveBytes { expression } => {
        let expression = *expression;
        self.walk(parsed, expression, scope, source);
      }
      NodeData::Declaration(_) => {
        let mut target = DataTarget::nested(scope);
        self.declare(parsed, node, &mut target, source);
      }
      NodeData::CompoundDeclaration(_) => {
        let mut target = DataTarget::nested(scope);
        self.compound_declaration(parsed, node, &mut target, source);
      }
      NodeData::Cast(cast) => {
        let (target_type, expression) = (cast.target_type, cast.expression);
        self.walk_all(
          parsed,
          target_type.into_iter().chain([expression]),
          scope,
          source,
        );
      }
      NodeData::DirectiveImport(_) => self.import(parsed, node, scope, None, source),
      NodeData::DirectiveRun(run) => {
        let (procedure, assertion) = (run.procedure, run.assertion_string);
        self.walk_all(
          parsed,
          [procedure].into_iter().chain(assertion),
          scope,
          source,
        );
      }
      NodeData::DirectiveCode { .. } => {
        // Quoted code is typechecked where it is inserted, not here
        // (**L§13.1**).
      }
      NodeData::DirectiveBake { procedure_call, .. } => {
        let procedure_call = *procedure_call;
        self.walk(parsed, procedure_call, scope, source);
      }
      NodeData::DirectiveModify { block } => {
        let block = *block;
        self.body(parsed, block, scope, source);
      }
      NodeData::ExpressionQuery {
        expression_to_query,
        ..
      } => {
        let expression = *expression_to_query;
        self.walk(parsed, expression, scope, source);
      }
      NodeData::PushContext { to_push, block, .. } => {
        let (to_push, block) = (*to_push, *block);
        if let Some(to_push) = to_push {
          self.walk(parsed, to_push, scope, source);
        }
        if let Some(block) = block {
          self.body(parsed, block, scope, source);
        }
      }
      NodeData::TypeQuery { type_to_query, .. } => {
        let type_to_query = *type_to_query;
        self.walk(parsed, type_to_query, scope, source);
      }
      NodeData::DirectiveLocation(location) => {
        if let Some(expression) = location.expression {
          self.walk(parsed, expression, scope, source);
        }
      }
      NodeData::DirectiveAddContext { expression } => {
        let expression = *expression;
        self.walk(parsed, expression, scope, source);
      }
      NodeData::Defer { block, .. } => {
        let block = *block;
        self.body(parsed, block, scope, source);
      }
      NodeData::Using(using) => {
        let (expression, filter_type, filter) =
          (using.expression, using.filter_type, using.filter_expression);
        let mut target = DataTarget::nested(scope);
        self.using(
          parsed,
          node,
          expression,
          filter_type,
          filter,
          &mut target,
          source,
        );
      }
      // Reaching an `#insert` through the expression walk means it stands where
      // a value goes; a statement one is admitted by whoever walks statements.
      NodeData::DirectiveInsert(_) => {
        self.insert(parsed, node, scope, InsertKind::Expression, source);
      }
      NodeData::DirectiveProcedureName { argument } => {
        if let Some(argument) = *argument {
          self.walk(parsed, argument, scope, source);
        }
      }
      NodeData::DirectiveModuleParameters(_) => {
        let mut target = DataTarget::nested(scope);
        self.module_parameters(parsed, node, &mut target, source);
      }
      NodeData::DirectivePokeName { module, name } => {
        let (module, name) = (*module, *name);
        let mut target = DataTarget::nested(scope);
        self.poke_name(parsed, module, name, &mut target, source);
      }
      NodeData::Asm(_) => self.asm(parsed, node, scope, source),
      // `#exists` asks whether a name resolves, so a miss is its answer rather
      // than an error (**L§5.14**); a `#place` target names a struct member,
      // not a scope entry.
      NodeData::DirectiveExists(_)
      | NodeData::DirectivePlace { .. }
      | NodeData::Note { .. }
      | NodeData::DirectiveLoad { .. }
      | NodeData::DirectiveScope { .. }
      | NodeData::DirectiveLibrary { .. }
      | NodeData::Context
      | NodeData::DirectiveThis
      | NodeData::DirectiveThrough
      | NodeData::DirectiveCompileTime
      | NodeData::DirectiveContextType
      | NodeData::DirectiveWildcard { .. }
      | NodeData::DirectiveFileInfo { .. }
      | NodeData::DirectiveCallerCode
      | NodeData::Placeholder => {}
    }
  }

  fn walk_all(
    &self,
    parsed: &Parsed,
    nodes: impl IntoIterator<Item = NodeId>,
    scope: ScopeId,
    source: SourceId,
  ) {
    for node in nodes {
      self.walk(parsed, node, scope, source);
    }
  }

  /// The body of an `if`, `while`, `for`, `defer` or `case`: a child scope even
  /// when it was written without braces (**L§4.2**).
  fn body(&self, parsed: &Parsed, node: NodeId, scope: ScopeId, source: SourceId) {
    match parsed.ast.data(node) {
      NodeData::Block(_) => self.walk(parsed, node, scope, source),
      _ => {
        let inner = self.tree.push_scope(ScopeKind::Imperative, Some(scope));
        self.imperative_statement(parsed, node, inner, source);
      }
    }
  }

  fn imperative_statement(
    &self,
    parsed: &Parsed,
    statement: NodeId,
    scope: ScopeId,
    source: SourceId,
  ) {
    if matches!(parsed.ast.data(statement), NodeData::Struct(_)) {
      let mut target = DataTarget::nested(scope);
      self.anonymous_aggregate(parsed, statement, &mut target, source);
      return;
    }
    if matches!(parsed.ast.data(statement), NodeData::DirectiveInsert(_)) {
      self.insert(parsed, statement, scope, InsertKind::Imperative, source);
      return;
    }
    self.walk(parsed, statement, scope, source);
  }

  /// `f(x,, allocator = temp)`: the left of a `,,` assignment names a member of
  /// the `Context` type, not anything in this scope (**L§10.1**). A struct
  /// literal field works the same way, so both walk only the value.
  fn assignment_value(&self, parsed: &Parsed, entry: NodeId, scope: ScopeId, source: SourceId) {
    match parsed.ast.data(entry) {
      NodeData::BinaryOperator {
        operator, right, ..
      } if *operator == oj_syntax::ast::OperatorType::ASSIGN => {
        let right = *right;
        self.walk(parsed, right, scope, source);
      }
      _ => self.walk(parsed, entry, scope, source),
    }
  }

  /// `union { … }` or `struct { … }` written as a statement declares no name of
  /// its own, so its members join the scope around it (**L§8.1**).
  fn anonymous_aggregate(
    &self,
    parsed: &Parsed,
    node: NodeId,
    target: &mut DataTarget,
    source: SourceId,
  ) {
    let NodeData::Struct(payload) = parsed.ast.data(node) else {
      return;
    };
    let Some(block) = payload.block else {
      return;
    };
    let statements = self.branch_statements(parsed, block);
    self.data_statements(parsed, &statements, target, source);
  }

  fn reference(
    &self,
    parsed: &Parsed,
    node: NodeId,
    name: Symbol,
    flags: oj_syntax::ast::IdentFlags,
    scope: ScopeId,
    source: SourceId,
  ) {
    use oj_syntax::ast::IdentFlags;
    // A member name, a `$T` being defined, and a backticked name (which
    // resolves in the macro's caller) are not lookups in this scope.
    if flags.intersects(
      IdentFlags::IS_RHS_OF_DOT_DEREFERENCE
        | IdentFlags::DEFINES_POLYMORPH_VARIABLE
        | IdentFlags::HAS_SCOPE_MODIFIER
        | IdentFlags::DO_NOT_RESOLVE_DUE_TO_PARSER,
    ) {
      return;
    }
    self.references.borrow_mut().push(Reference {
      name,
      scope,
      source,
      span: parsed.ast.node(node).span,
      node,
      speculative: self.speculative.get() > 0,
    });
  }

  // ----------------------------------------------------------- procedures ---

  fn procedure(&self, parsed: &Parsed, header: NodeId, parent: ScopeId, source: SourceId) {
    let NodeData::ProcedureHeader(payload) = parsed.ast.data(header) else {
      return;
    };
    let payload = payload.clone();

    let constants = self
      .tree
      .push_scope(ScopeKind::ProcedureConstants, Some(parent));
    let arguments = self
      .tree
      .push_scope(ScopeKind::ProcedureArguments, Some(constants));

    let mut argument_target = DataTarget::nested(arguments);
    for parameter in &payload.arguments {
      self.declare_polymorph_variables(parsed, *parameter, constants, source);
      self.declare_parameter(parsed, *parameter, &mut argument_target, source);
    }

    // Named return values are their own scope, which the body is not inside:
    // a return may be named after a parameter (`(oh: float64, ol: float64) ->
    // (oh: float64, ol: float64)`) without redeclaring it, and the name in the
    // body is still the parameter, because a named return is not a name the
    // body can read at all (**L§7.2**).
    let returns = self
      .tree
      .push_scope(ScopeKind::ProcedureReturns, Some(arguments));
    let mut return_target = DataTarget::nested(returns);
    for parameter in &payload.returns {
      self.declare_polymorph_variables(parsed, *parameter, constants, source);
      self.declare_parameter(parsed, *parameter, &mut return_target, source);
    }

    // A polymorphic procedure or a macro has no body until something
    // instantiates or expands it, so nothing written inside one is checked or
    // executed until then (**L§7.8**, **L§7.13**). A `$` in a parameter's type
    // declares a variable in the constants scope; a `$` on its *name* makes
    // the parameter itself a constant of the instantiation.
    let polymorphic = payload
      .procedure_flags
      .intersects(ProcedureFlags::POLYMORPHIC | ProcedureFlags::MACRO)
      || !self.tree.declarations(constants).is_empty()
      || payload
        .arguments
        .iter()
        .chain(&payload.returns)
        .any(|parameter| declares_a_constant(parsed, *parameter));
    if polymorphic {
      self.uninstantiated_scopes.borrow_mut().insert(constants);
    }
    if payload.procedure_flags.contains(ProcedureFlags::MACRO) {
      self.macro_scopes.borrow_mut().insert(constants);
    }
    self.procedure_scopes.borrow_mut().insert(
      (source, header),
      ProcedureScopes {
        constants,
        arguments,
        returns,
        polymorphic,
      },
    );

    // `#modify` runs with the `$` parameters bound as mutable values
    // (**C§6.3**), so it reads them where they are declared.
    for directive in &payload.modify_directives {
      self.walk(parsed, *directive, arguments, source);
    }
    if let Some(library) = payload.library_identifier {
      self.walk(parsed, library, parent, source);
    }

    if let Some(body) = payload.body_or_null {
      let NodeData::ProcedureBody { block, .. } = parsed.ast.data(body) else {
        return;
      };
      let block = *block;
      let inner = self.tree.push_scope(ScopeKind::Imperative, Some(arguments));
      let statements = match parsed.ast.data(block) {
        NodeData::Block(block) => block.statements.clone(),
        _ => vec![block],
      };
      for statement in statements {
        self.imperative_statement(parsed, statement, inner, source);
      }
    }
  }

  /// One parameter or named return value. `using p: Player` also widens the
  /// scope, which needs `Player`'s members and therefore waits (**L§4.3**).
  /// An `#asm` block declares its registers in the scope it stands in, not in
  /// one of its own, so they are still visible to the blocks that follow
  /// (**L§15**). Everything else an operand names — a high-level variable, a
  /// constant, the type behind a `?T` size — resolves where it was written.
  fn asm(&self, parsed: &Parsed, node: NodeId, scope: ScopeId, source: SourceId) {
    let NodeData::Asm(block) = parsed.ast.data(node) else {
      return;
    };
    let block = block.clone();
    for instruction in &block.instructions {
      if let AsmSize::Of(expression) = instruction.size {
        self.walk(parsed, expression, scope, source);
      }
      for operand in &instruction.operands {
        match &operand.kind {
          AsmOperandKind::Declaration(declaration) => {
            self.declare_asm_register(parsed, declaration.name, scope, source);
          }
          AsmOperandKind::Pin { name, .. } => self.walk(parsed, *name, scope, source),
          AsmOperandKind::Expression(expression) => self.walk(parsed, *expression, scope, source),
          AsmOperandKind::Memory(memory) => {
            self.walk(parsed, memory.base, scope, source);
            for part in [memory.index, memory.scale, memory.displacement]
              .into_iter()
              .flatten()
            {
              self.walk(parsed, part, scope, source);
            }
          }
        }
        if let Some(mask) = &operand.mask {
          self.walk(parsed, mask.register, scope, source);
        }
      }
    }
  }

  /// `mov apple:, 10` declares `apple` the way `apple := 10` would, except
  /// that what it names is a register rather than storage (**L§15**). A name
  /// an earlier block already declared is the same register, not a second one.
  fn declare_asm_register(&self, parsed: &Parsed, name: NodeId, scope: ScopeId, source: SourceId) {
    let NodeData::Ident(ident) = parsed.ast.data(name) else {
      return;
    };
    let symbol = ident.name;
    if self.tree.declares(scope, symbol) {
      return;
    }
    let _ = self.tree.declare(Decl {
      name: symbol,
      scope,
      kind: DeclKind::AsmRegister,
      visibility: Visibility::File,
      flags: DeclarationFlags::empty(),
      source: Some(source),
      span: parsed.ast.node(name).span,
      node: Some(name),
      conditional: false,
      branch: self.branch.get(),
      overloadable: false,
    });
  }

  fn declare_parameter(
    &self,
    parsed: &Parsed,
    parameter: NodeId,
    target: &mut DataTarget,
    source: SourceId,
  ) {
    match parsed.ast.data(parameter) {
      NodeData::Declaration(_) => {
        if let Some(id) = self.declare(parsed, parameter, target, source) {
          self.set_parameter_kind(id);
        }
      }
      NodeData::Using(using) => {
        let (expression, filter_type, filter) =
          (using.expression, using.filter_type, using.filter_expression);
        self.using(
          parsed,
          parameter,
          expression,
          filter_type,
          filter,
          target,
          source,
        );
      }
      _ => self.walk(parsed, parameter, target.scope, source),
    }
  }

  /// A parameter is a constant of the call, not a global or a local: the kind
  /// is fixed after the declaration is made so that `declare` stays the one
  /// place a name enters a scope.
  fn set_parameter_kind(&self, id: DeclId) {
    if self.tree.decl(id).kind == DeclKind::Variable {
      self.tree.set_decl_kind(id, DeclKind::Parameter);
    }
  }

  /// `$T` and `$$T` introduce constants of the procedure's constants block
  /// (**L§7.8**); they are written inside the parameter's type expression.
  fn declare_polymorph_variables(
    &self,
    parsed: &Parsed,
    node: NodeId,
    constants: ScopeId,
    source: SourceId,
  ) {
    let mut found = Vec::new();
    collect_polymorph_variables(parsed, node, &mut found);
    for (name, span, node) in found {
      let _ = self.tree.declare(Decl {
        name,
        scope: constants,
        kind: DeclKind::Constant,
        visibility: Visibility::Export,
        flags: DeclarationFlags::IS_CONSTANT,
        source: Some(source),
        span,
        node: Some(node),
        conditional: true,
        branch: None,
        overloadable: false,
      });
    }
  }

  // ------------------------------------------------------ structs and enums ---

  fn structure(&self, parsed: &Parsed, node: NodeId, parent: ScopeId, source: SourceId) {
    let NodeData::Struct(payload) = parsed.ast.data(node) else {
      return;
    };
    let payload = payload.clone();

    let arguments = self
      .tree
      .push_scope(ScopeKind::StructArguments, Some(parent));
    let mut argument_target = DataTarget::nested(arguments);
    for parameter in &payload.arguments {
      self.declare_polymorph_variables(parsed, *parameter, arguments, source);
      self.declare_parameter(parsed, *parameter, &mut argument_target, source);
    }

    // A polymorphic struct is a family, not a type: nothing written in its
    // body exists until an instantiation supplies the arguments (**L§8.5**).
    if payload.has_argument_list && !payload.arguments.is_empty() {
      self.uninstantiated_scopes.borrow_mut().insert(arguments);
    }
    let members = self
      .tree
      .push_scope(ScopeKind::StructMembers, Some(arguments));
    self
      .aggregate_scopes
      .borrow_mut()
      .insert((source, node), members);
    for directive in &payload.modify_directives {
      self.walk(parsed, *directive, arguments, source);
    }
    if let Some(alignment) = payload.alignment_expression {
      self.walk(parsed, alignment, parent, source);
    }
    if let Some(block) = payload.block {
      let statements = match parsed.ast.data(block) {
        NodeData::Block(block) => block.statements.clone(),
        _ => vec![block],
      };
      let mut target = DataTarget::nested(members);
      self.data_statements(parsed, &statements, &mut target, source);
    }
  }

  fn enumeration(&self, parsed: &Parsed, node: NodeId, parent: ScopeId, source: SourceId) {
    let NodeData::Enum(payload) = parsed.ast.data(node) else {
      return;
    };
    let payload = payload.clone();
    if let Some(internal) = payload.internal_type_inst {
      self.walk(parsed, internal, parent, source);
    }
    let scope = self.tree.push_scope(ScopeKind::Enum, Some(parent));
    self
      .aggregate_scopes
      .borrow_mut()
      .insert((source, node), scope);
    if let Some(block) = payload.block {
      let statements = match parsed.ast.data(block) {
        NodeData::Block(block) => block.statements.clone(),
        _ => vec![block],
      };
      let mut target = DataTarget::nested(scope);
      self.data_statements(parsed, &statements, &mut target, source);
    }
  }

  fn for_loop(
    &self,
    parsed: &Parsed,
    node: NodeId,
    payload: &oj_syntax::ast::ForNode,
    parent: ScopeId,
    source: SourceId,
  ) {
    self.walk(parsed, payload.iteration_expression, parent, source);
    if let Some(right) = payload.iteration_expression_right {
      self.walk(parsed, right, parent, source);
    }
    for modifier in [
      payload.want_replacement_for_expansion,
      payload.want_pointer_expression,
      payload.want_reverse_expression,
    ]
    .into_iter()
    .flatten()
    {
      self.walk(parsed, modifier, parent, source);
    }

    // `it` and `it_index` live in the loop's own scope and shadow outer ones
    // (**L§4.3**, **L§6.6**).
    let scope = self.tree.push_scope(ScopeKind::Imperative, Some(parent));
    self.loop_scopes.borrow_mut().insert((source, node), scope);
    let span = parsed.ast.node(payload.block).span;
    let implicit = [b"it".as_slice(), b"it_index".as_slice()];
    for (written, fallback) in [payload.ident_it, payload.ident_it_index]
      .into_iter()
      .zip(implicit)
    {
      // `it` and `it_index` are ordinary identifiers that the loop declares
      // implicitly unless it names them itself (**L§6.6**).
      let (symbol, span, node) = match written.map(|name| (parsed.ast.data(name), name)) {
        Some((NodeData::Ident(ident), name)) => {
          (ident.name, parsed.ast.node(name).span, Some(name))
        }
        _ => (self.interner.intern(fallback), span, None),
      };
      let _ = self.tree.declare(Decl {
        name: symbol,
        scope,
        kind: DeclKind::Iterator,
        visibility: Visibility::File,
        flags: DeclarationFlags::IS_ITERATOR,
        source: Some(source),
        span,
        node,
        conditional: false,
        branch: None,
        overloadable: false,
      });
    }
    // The body is that same scope: `it` is not in a scope outside the body.
    let statements = match parsed.ast.data(payload.block) {
      NodeData::Block(block) => block.statements.clone(),
      _ => vec![payload.block],
    };
    for statement in statements {
      self.imperative_statement(parsed, statement, scope, source);
    }
  }
}

fn collect_polymorph_variables(
  parsed: &Parsed,
  node: NodeId,
  found: &mut Vec<(Symbol, Span, NodeId)>,
) {
  match parsed.ast.data(node) {
    NodeData::Ident(ident) => {
      if ident
        .flags
        .contains(oj_syntax::ast::IdentFlags::DEFINES_POLYMORPH_VARIABLE)
      {
        found.push((ident.name, parsed.ast.node(node).span, node));
      }
    }
    NodeData::Declaration(declaration) => {
      for child in [
        declaration.name,
        declaration.type_inst,
        declaration.expression,
      ]
      .into_iter()
      .flatten()
      {
        collect_polymorph_variables(parsed, child, found);
      }
    }
    NodeData::Using(using) => collect_polymorph_variables(parsed, using.expression, found),
    NodeData::TypeInstantiation(inst) => {
      for child in [
        inst.type_valued_expression,
        inst.must_implement,
        inst.pointer_to,
        inst.type_directive_target,
        inst.array_element_type,
        inst.array_dimension,
      ]
      .into_iter()
      .flatten()
      {
        collect_polymorph_variables(parsed, child, found);
      }
    }
    NodeData::UnaryOperator { operand, .. } => collect_polymorph_variables(parsed, *operand, found),
    NodeData::BinaryOperator { left, right, .. } => {
      collect_polymorph_variables(parsed, *left, found);
      collect_polymorph_variables(parsed, *right, found);
    }
    NodeData::ProcedureCall(call) => {
      for argument in &call.arguments {
        collect_polymorph_variables(parsed, argument.expression, found);
      }
    }
    NodeData::ProcedureHeader(header) => {
      for parameter in header.arguments.iter().chain(header.returns.iter()) {
        collect_polymorph_variables(parsed, *parameter, found);
      }
    }
    _ => {}
  }
}

/// Whether a parameter is written `$name: T` — a constant of the instantiation
/// rather than an argument, which the parser records as a required bake
/// (**L§7.8**).
fn declares_a_constant(parsed: &Parsed, parameter: NodeId) -> bool {
  match parsed.ast.data(parameter) {
    NodeData::Declaration(declaration) => declaration
      .flags
      .contains(DeclarationFlags::AUTO_VALUE_BAKE_IS_REQUIRED),
    _ => false,
  }
}
