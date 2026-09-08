use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use oj_diag::{Diagnostic, SourceId, SourceMap, Span};
use oj_lexer::{Interner, Symbol};
use oj_source::{ImportPath, ModuleError};
use oj_syntax::Parsed;
use oj_syntax::ast::{
  Argument, DeclarationFlags, FilterType, ImportType, NodeData, NodeId, ProcedureFlags, ScopeType,
};

use crate::constants::{AstSource, ConstValue, Evaluator};
use crate::tree::{
  Decl, DeclId, DeclKind, ImportEdge, PendingProvider, ScopeId, ScopeKind, ScopeTree, Visibility,
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
}

impl Default for Options {
  fn default() -> Self {
    Self {
      jai_dir: None,
      import_dirs: Vec::new(),
      follow_imports: true,
      load_preload: true,
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

/// One parsed file and the file scope it was resolved into.
pub struct Unit {
  pub source: SourceId,
  pub path: PathBuf,
  pub scope: ScopeId,
  pub parsed: Arc<Parsed>,
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
  units: Vec<Unit>,
  unit_of_source: HashMap<SourceId, usize>,
  modules: HashMap<ModuleKey, ScopeId>,
  loaded: HashSet<(ScopeId, PathBuf)>,
  /// The member scope of each struct and enum definition, so that a `using` of
  /// a type declaration can widen the scope around it without waiting for
  /// types (**L§6.8**).
  aggregate_scopes: HashMap<(SourceId, NodeId), ScopeId>,
  /// The scope a `for` loop declares its `it` and `it_index` in, by the loop
  /// node: an implicit one has no identifier to be found through (**L§6.5**).
  loop_scopes: HashMap<(SourceId, NodeId), ScopeId>,
  /// The scopes every procedure header opened, by the header node. An
  /// instantiation binds the `$T`s declared in the constants block, so it
  /// needs a way in that does not go through a name (**L§7.8**).
  procedure_scopes: HashMap<(SourceId, NodeId), ProcedureScopes>,
  /// The constants scope of every polymorphic procedure and macro. Nothing
  /// inside one of those exists until an instantiation makes it exist, so a
  /// `#run` written there waits for M7 rather than executing (**L§12.1**).
  uninstantiated_scopes: HashSet<ScopeId>,
  builtins: HashMap<DeclId, ConstValue>,
  references: Vec<Reference>,
  /// Names some macro declares with a backtick, which land in whatever block
  /// the macro expands into rather than where they are written (**L§7.13**).
  /// A miss on one of these is a wait for macro expansion (M7), not an error.
  macro_injected: HashSet<Symbol>,
  diagnostics: Vec<Diagnostic>,
  pending_ifs: Vec<PendingIf>,
  /// Nonzero while the branches of an undecidable `#if` are being admitted, so
  /// nothing reached from there is known to be real: its diagnostics are held
  /// back until the condition can be decided (M6 runs the `#run`s most of them
  /// wait on).
  speculative: u32,
  /// Nonzero while declarations belong to one branch of an undecidable `#if`.
  /// Unlike `speculative` this stops at a module boundary: a module is loaded
  /// once however it was reached, so its own declarations are not conditional.
  conditional: u32,
  unshared_instances: u32,
  preload: ScopeId,
  main: ScopeId,
}

impl AstSource for Program<'_> {
  fn ast_of(&self, source: SourceId) -> Option<&oj_syntax::ast::Ast> {
    self
      .unit_of_source
      .get(&source)
      .map(|index| &self.units[*index].parsed.ast)
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
    let jai_dir = options.jai_dir.clone().unwrap_or_default();
    let mut import_path = ImportPath::default_for(root, &jai_dir);
    for directory in options.import_dirs.iter().rev() {
      import_path.prepend(directory.clone());
    }

    let mut tree = ScopeTree::new();
    let preload = tree.push_scope(ScopeKind::Preload, None);
    let main = tree.push_scope(ScopeKind::Module, Some(preload));

    let mut program = Self {
      sources,
      interner,
      options,
      import_path,
      tree,
      units: Vec::new(),
      unit_of_source: HashMap::new(),
      modules: HashMap::new(),
      loaded: HashSet::new(),
      aggregate_scopes: HashMap::new(),
      loop_scopes: HashMap::new(),
      procedure_scopes: HashMap::new(),
      uninstantiated_scopes: HashSet::new(),
      builtins: HashMap::new(),
      references: Vec::new(),
      macro_injected: HashSet::new(),
      diagnostics: Vec::new(),
      pending_ifs: Vec::new(),
      speculative: 0,
      conditional: 0,
      unshared_instances: 0,
      preload,
      main,
    };

    program.declare_builtins();
    if program.options.load_preload {
      program.load_preload();
    }
    program.load_file_into(main, root, None);
    program.settle_pending_ifs();
    program
  }

  pub fn tree(&self) -> &ScopeTree {
    &self.tree
  }

  pub fn units(&self) -> &[Unit] {
    &self.units
  }

  pub fn references(&self) -> &[Reference] {
    &self.references
  }

  /// Whether `name` is one a macro injects into its caller's block.
  pub fn is_macro_injected(&self, name: Symbol) -> bool {
    self.macro_injected.contains(&name)
  }

  pub fn diagnostics(&self) -> &[Diagnostic] {
    &self.diagnostics
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
    self.aggregate_scopes.get(&(source, node)).copied()
  }

  /// Every recorded aggregate body, as `(source, node, members scope)`. The
  /// typechecker inverts this to find the definition a member scope belongs to.
  /// The scope a `for` loop declared its `it` and `it_index` in (**L§6.5**). A
  /// loop that did not name them has no identifier to find them through, so
  /// this is the only way in.
  pub fn loop_scope(&self, source: SourceId, node: NodeId) -> Option<ScopeId> {
    self.loop_scopes.get(&(source, node)).copied()
  }

  /// The scopes a procedure header opened (**L§7.8**).
  pub fn procedure_scopes(&self, source: SourceId, header: NodeId) -> Option<ProcedureScopes> {
    self.procedure_scopes.get(&(source, header)).copied()
  }

  /// Whether a scope lies inside a polymorphic procedure or a macro, whose
  /// body only exists once something instantiates or expands it. Nothing
  /// written there is checked or executed until then (**L§7.8**, **L§7.13**).
  pub fn is_uninstantiated(&self, scope: ScopeId) -> bool {
    let mut current = Some(scope);
    while let Some(id) = current {
      if self.uninstantiated_scopes.contains(&id) {
        return true;
      }
      current = self.tree.scope(id).parent;
    }
    false
  }

  pub fn aggregate_scopes(&self) -> impl Iterator<Item = (SourceId, NodeId, ScopeId)> + '_ {
    self
      .aggregate_scopes
      .iter()
      .map(|((source, node), scope)| (*source, *node, *scope))
  }

  pub fn has_errors(&self) -> bool {
    self.diagnostics.iter().any(Diagnostic::is_error)
  }

  fn declare_builtins(&mut self) {
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
  }

  fn declare_builtin(&mut self, name: &[u8], value: Option<ConstValue>) {
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
        overloadable: false,
      })
      .unwrap_or_else(|previous| previous);
    if let Some(value) = value {
      self.builtins.insert(id, value);
    }
  }

  fn load_preload(&mut self) {
    let Some(jai_dir) = self.options.jai_dir.clone() else {
      return;
    };
    let preload = jai_dir.join("modules").join("Preload.jai");
    if preload.is_file() {
      let scope = self.preload;
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
  fn import_module_by_name(&mut self, name: &str, into: ScopeId) {
    let Ok(resolved) = oj_source::resolve_module(name, &self.import_path) else {
      return;
    };
    let key = ModuleKey {
      entry: resolved.entry.clone(),
      parameters: String::new(),
      instance: 0,
    };
    let module = match self.modules.get(&key) {
      Some(existing) => *existing,
      None => {
        let module = self.tree.push_scope(ScopeKind::Module, Some(self.preload));
        self.modules.insert(key, module);
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
  fn load_file_into(&mut self, module: ScopeId, path: &Path, origin: Option<(SourceId, Span)>) {
    let key = (module, path.to_path_buf());
    if !self.loaded.insert(key) {
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

    let file = self.sources.file(source);
    let parsed = Arc::new(oj_syntax::parse(file.bytes(), source, self.interner));
    self.diagnostics.extend(parsed.diagnostics.iter().cloned());

    let scope = self.tree.push_scope(ScopeKind::File, Some(module));
    self.tree.set_file(scope, source, path);
    self.unit_of_source.insert(source, self.units.len());
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
  fn load_module_files(&mut self, module: ScopeId, entry: &Path, origin: Option<(SourceId, Span)>) {
    let conditional = std::mem::take(&mut self.conditional);
    self.load_file_into(module, entry, origin);
    self.conditional = conditional;
  }

  fn path_of(&self, source: SourceId) -> PathBuf {
    self
      .unit_of_source
      .get(&source)
      .map(|index| self.units[*index].path.clone())
      .unwrap_or_default()
  }

  // ------------------------------------------------------------ data scope ---

  fn data_statements(
    &mut self,
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
    &mut self,
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
      NodeData::Struct(_) => self.anonymous_aggregate(parsed, statement, target, source),
      // An enum body's `#if` branches parse as imperative statements, so a bare
      // member name arrives here as an identifier rather than a declaration; in
      // an enum scope it is still a member (**L§6.10**, **L§9**).
      NodeData::Ident(ident) if self.tree.scope(target.scope).kind == ScopeKind::Enum => {
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
    &mut self,
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
    &mut self,
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
    // to turn up unresolved at the call sites until M7 expands them.
    let backticked = declaration
      .flags
      .contains(DeclarationFlags::HAS_SCOPE_MODIFIER);
    if backticked {
      self.macro_injected.insert(ident);
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
      overloadable,
    });

    if let Err(previous) = declared {
      let previous = self.tree.decl(previous).clone();
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
    &mut self,
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
    target.conditional || self.conditional > 0
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
    &mut self,
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
    &mut self,
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

    if let NodeData::Declaration(_) = parsed.ast.data(expression) {
      self.declare(parsed, expression, target, source);
    }

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
          None => self.tree.add_pending(destination, PendingProvider::Using),
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
    self.aggregate_scopes.get(&(source, value)).copied()
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

  fn load(&mut self, name: &[u8], statement: NodeId, target: &mut DataTarget, source: SourceId) {
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
    &mut self,
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
    &mut self,
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
    let name = String::from_utf8_lossy(&import.name).into_owned();
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
      self.unshared_instances += 1;
      self.unshared_instances
    } else {
      0
    };
    let key = ModuleKey {
      entry: resolved.entry.clone(),
      parameters: self.parameter_text(parsed, &import, source),
      instance,
    };
    if let Some(existing) = self.modules.get(&key) {
      return Some(*existing);
    }

    let module = self.tree.push_scope(ScopeKind::Module, Some(self.preload));
    self.modules.insert(key, module);
    self.load_module_files(module, &resolved.entry, Some((source, span)));
    Some(module)
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
    &mut self,
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
    &mut self,
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
      overloadable: false,
    });
  }

  // ------------------------------------------------------------ static if ---

  fn static_if(
    &mut self,
    parsed: &Parsed,
    node: NodeId,
    target: &mut DataTarget,
    source: SourceId,
  ) {
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
      None => self.pending_ifs.push(PendingIf::Data {
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
  fn settle_pending_ifs(&mut self) {
    loop {
      let pending = std::mem::take(&mut self.pending_ifs);
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
      self.pending_ifs.extend(still_pending);
    }
  }

  fn resume_static_if(&mut self, parsed: &Parsed, item: PendingIf, statements: &[NodeId]) {
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
  fn admit_all_branches(&mut self, item: PendingIf) {
    let (node, source) = match &item {
      PendingIf::Data { node, source, .. } => (*node, *source),
      PendingIf::Imperative { node, source, .. } => (*node, *source),
    };
    let Some(parsed) = self.parsed_of(source) else {
      return;
    };
    let branches = self.all_branch_statements(&parsed, node);
    self.speculative += 1;
    self.conditional += 1;

    match item {
      PendingIf::Data {
        mut target, source, ..
      } => {
        self
          .tree
          .add_pending(target.destination(), PendingProvider::StaticIf);
        target.conditional = true;
        for statements in branches {
          let mut branch_target = target;
          self.data_statements(&parsed, &statements, &mut branch_target, source);
        }
      }
      PendingIf::Imperative { scope, source, .. } => {
        self.tree.add_pending(scope, PendingProvider::StaticIf);
        for statements in branches {
          for statement in statements {
            self.imperative_statement(&parsed, statement, scope, source);
          }
        }
      }
    }
    self.speculative -= 1;
    self.conditional -= 1;
  }

  fn parsed_of(&self, source: SourceId) -> Option<Arc<Parsed>> {
    self
      .unit_of_source
      .get(&source)
      .map(|index| Arc::clone(&self.units[*index].parsed))
  }

  fn fold_condition(&self, scope: ScopeId, source: SourceId, node: NodeId) -> Option<bool> {
    self.fold(scope, source, node)?.truth()
  }

  fn fold(&self, scope: ScopeId, source: SourceId, node: NodeId) -> Option<ConstValue> {
    Evaluator::new(&self.tree, self, &self.builtins, self.interner).eval(scope, source, node)
  }

  // ------------------------------------------------------- diagnostics ------

  fn error(&mut self, source: SourceId, span: Span, message: impl Into<String>) {
    self.report(Diagnostic::error(source, span, message));
  }

  fn report(&mut self, diagnostic: Diagnostic) {
    if self.speculative == 0 {
      self.diagnostics.push(diagnostic);
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
  fn walk(&mut self, parsed: &Parsed, node: NodeId, scope: ScopeId, source: SourceId) {
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
            None => self.pending_ifs.push(PendingIf::Imperative {
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
      NodeData::DirectiveInsert(insert) => {
        let expression = insert.expression;
        // An `#insert` may declare anything; without a `#placeholder` the
        // reference does not wait for it, but it may still provide names
        // (**L§11.7**).
        self.tree.add_pending(scope, PendingProvider::Insert);
        self.walk(parsed, expression, scope, source);
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
      // `mov a:, 12` inside an `#asm` block declares the register `a` in the
      // block around it (**L§15**), and the body is a blob until M9 assembles
      // it, so the scope can still gain names.
      NodeData::Asm(_) => self.tree.add_pending(scope, PendingProvider::Insert),
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
    &mut self,
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
  fn body(&mut self, parsed: &Parsed, node: NodeId, scope: ScopeId, source: SourceId) {
    match parsed.ast.data(node) {
      NodeData::Block(_) => self.walk(parsed, node, scope, source),
      _ => {
        let inner = self.tree.push_scope(ScopeKind::Imperative, Some(scope));
        self.imperative_statement(parsed, node, inner, source);
      }
    }
  }

  fn imperative_statement(
    &mut self,
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
    self.walk(parsed, statement, scope, source);
  }

  /// `f(x,, allocator = temp)`: the left of a `,,` assignment names a member of
  /// the `Context` type, not anything in this scope (**L§10.1**). A struct
  /// literal field works the same way, so both walk only the value.
  fn assignment_value(&mut self, parsed: &Parsed, entry: NodeId, scope: ScopeId, source: SourceId) {
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
    &mut self,
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
    &mut self,
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
    self.references.push(Reference {
      name,
      scope,
      source,
      span: parsed.ast.node(node).span,
      node,
      speculative: self.speculative > 0,
    });
  }

  // ----------------------------------------------------------- procedures ---

  fn procedure(&mut self, parsed: &Parsed, header: NodeId, parent: ScopeId, source: SourceId) {
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

    // Named return values are their own scope: a macro may name a return after
    // a parameter (`(count: int, s: *u8) -> (n: int, s: *u8)`), which shadows
    // rather than redeclares.
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
      || !self.tree.scope(constants).declarations.is_empty()
      || payload
        .arguments
        .iter()
        .chain(&payload.returns)
        .any(|parameter| declares_a_constant(parsed, *parameter));
    if polymorphic {
      self.uninstantiated_scopes.insert(constants);
    }
    self.procedure_scopes.insert(
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
      let inner = self.tree.push_scope(ScopeKind::Imperative, Some(returns));
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
  fn declare_parameter(
    &mut self,
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
  fn set_parameter_kind(&mut self, id: DeclId) {
    if self.tree.decl(id).kind == DeclKind::Variable {
      self.tree.set_decl_kind(id, DeclKind::Parameter);
    }
  }

  /// `$T` and `$$T` introduce constants of the procedure's constants block
  /// (**L§7.8**); they are written inside the parameter's type expression.
  fn declare_polymorph_variables(
    &mut self,
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
        overloadable: false,
      });
    }
  }

  // ------------------------------------------------------ structs and enums ---

  fn structure(&mut self, parsed: &Parsed, node: NodeId, parent: ScopeId, source: SourceId) {
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

    let members = self
      .tree
      .push_scope(ScopeKind::StructMembers, Some(arguments));
    self.aggregate_scopes.insert((source, node), members);
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

  fn enumeration(&mut self, parsed: &Parsed, node: NodeId, parent: ScopeId, source: SourceId) {
    let NodeData::Enum(payload) = parsed.ast.data(node) else {
      return;
    };
    let payload = payload.clone();
    if let Some(internal) = payload.internal_type_inst {
      self.walk(parsed, internal, parent, source);
    }
    let scope = self.tree.push_scope(ScopeKind::Enum, Some(parent));
    self.aggregate_scopes.insert((source, node), scope);
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
    &mut self,
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
    self.loop_scopes.insert((source, node), scope);
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
