use std::collections::HashMap;
use std::path::{Path, PathBuf};

use oj_diag::{SourceId, Span};
use oj_lexer::Symbol;
use oj_syntax::NodeId;
use oj_syntax::ast::DeclarationFlags;

/// A scope's identity in one [`ScopeTree`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeId(pub u32);

/// A declaration's identity in one [`ScopeTree`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclId(pub u32);

/// The scope kinds of **L§4.2**. `Preload` is the root of the whole tree; every
/// module (including the main program) hangs directly off it, so a module sees
/// Preload, its own files and its imports, and nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeKind {
  Preload,
  Module,
  File,
  StructArguments,
  StructMembers,
  Enum,
  ProcedureConstants,
  ProcedureArguments,
  ProcedureReturns,
  Imperative,
}

impl ScopeKind {
  /// The scopes a `#scope_*` directive chooses between (**L§4.4**).
  pub fn is_program_scope(self) -> bool {
    matches!(self, Self::Preload | Self::Module | Self::File)
  }

  pub fn name(self) -> &'static str {
    match self {
      Self::Preload => "preload",
      Self::Module => "module",
      Self::File => "file",
      Self::StructArguments => "struct arguments",
      Self::StructMembers => "struct members",
      Self::Enum => "enum",
      Self::ProcedureConstants => "procedure constants",
      Self::ProcedureArguments => "procedure arguments",
      Self::ProcedureReturns => "procedure returns",
      Self::Imperative => "block",
    }
  }
}

/// Where a declaration written in a file ends up, chosen by the `#scope_*`
/// directive in effect (**L§4.4**). `Export` and `Module` declarations live in
/// the module scope; only `Export` ones are visible to importers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
  #[default]
  Export,
  Module,
  File,
}

impl Visibility {
  pub fn is_exported(self) -> bool {
    self == Self::Export
  }

  pub fn name(self) -> &'static str {
    match self {
      Self::Export => "export",
      Self::Module => "module",
      Self::File => "file",
    }
  }
}

/// What a name stands for, to the extent M3 can tell without types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclKind {
  Variable,
  Constant,
  Procedure,
  Struct,
  Enum,
  /// A named `#import` binding: `Math :: #import "Math"`.
  Module(ScopeId),
  Parameter,
  /// A `for` loop's `it` / `it_index`, or a `case` binding.
  Iterator,
  /// `#placeholder NAME;` (**L§11.8**) — filled later by a metaprogram.
  Placeholder,
  /// A name the compiler injects: `OS`, `CPU`, the basic types (**L§4.3**).
  Builtin,
}

impl DeclKind {
  pub fn name(self) -> &'static str {
    match self {
      Self::Variable => "variable",
      Self::Constant => "constant",
      Self::Procedure => "procedure",
      Self::Struct => "struct",
      Self::Enum => "enum",
      Self::Module(_) => "module",
      Self::Parameter => "parameter",
      Self::Iterator => "iterator",
      Self::Placeholder => "placeholder",
      Self::Builtin => "builtin",
    }
  }
}

#[derive(Clone, Debug)]
pub struct Decl {
  pub name: Symbol,
  pub scope: ScopeId,
  pub kind: DeclKind,
  pub visibility: Visibility,
  pub flags: DeclarationFlags,
  pub source: Option<SourceId>,
  pub span: Span,
  pub node: Option<NodeId>,
  /// Declared inside a `#if` whose condition M3 could not fold, so it neither
  /// collides with nor is collided with by its sibling branch (**L§4.2**).
  pub conditional: bool,
  /// May share its name with other declarations: a procedure, or a constant
  /// that may name one (**L§7.7**). Whether an alias really is a procedure
  /// needs its type, so a constant that could be one is admitted.
  pub overloadable: bool,
}

/// A `using`/`#import` edge that makes another scope's names visible here
/// (**L§4.3**). Unnamed imports contribute to overload sets, so lookup follows
/// these rather than copying declarations.
#[derive(Clone, Debug)]
pub struct ImportEdge {
  pub target: ScopeId,
  pub only: Vec<Symbol>,
  pub except: Vec<Symbol>,
  pub span: Span,
  pub source: Option<SourceId>,
  /// Whether an importer of *this* scope sees the names too. A `using` of a
  /// type declaration makes its members part of the scope, so `using E :: enum
  /// { … }` at module scope reaches importers; a `#import` never re-exports
  /// what it imported (**L§11.2**).
  pub transitive: bool,
}

impl ImportEdge {
  pub fn admits(&self, name: Symbol) -> bool {
    if !self.only.is_empty() && !self.only.contains(&name) {
      return false;
    }
    !self.except.contains(&name)
  }
}

/// A construct that may still introduce names into a scope: an unresolved
/// `using` of a typed value, an `#insert`, or a `#if` whose condition M3 could
/// not fold. A lookup that misses in a scope holding one of these is not an
/// error — the reference waits for the construct instead (**L§4.3**, **C§6.1**).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingProvider {
  Using,
  Insert,
  StaticIf,
  FailedImport,
}

#[derive(Debug)]
pub struct Scope {
  pub kind: ScopeKind,
  pub parent: Option<ScopeId>,
  pub children: Vec<ScopeId>,
  pub source: Option<SourceId>,
  /// The file a `#load`/relative `#import` inside this scope resolves against.
  pub path: Option<PathBuf>,
  pub names: HashMap<Symbol, Vec<DeclId>>,
  pub declarations: Vec<DeclId>,
  pub imports: Vec<ImportEdge>,
  pub pending: Vec<PendingProvider>,
}

impl Scope {
  fn new(kind: ScopeKind, parent: Option<ScopeId>) -> Self {
    Self {
      kind,
      parent,
      children: Vec::new(),
      source: None,
      path: None,
      names: HashMap::new(),
      declarations: Vec::new(),
      imports: Vec::new(),
      pending: Vec::new(),
    }
  }

  pub fn has_pending_providers(&self) -> bool {
    !self.pending.is_empty()
  }
}

/// The result of looking a name up: the declarations that could satisfy it, or
/// the reason the answer is not yet knowable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
  Found(Vec<DeclId>),
  /// A scope on the lookup chain still has an unfinished name-inserting
  /// construct, so the reference would wait rather than fail (**L§4.3**).
  Pending,
  Undeclared,
}

#[derive(Debug, Default)]
pub struct ScopeTree {
  scopes: Vec<Scope>,
  declarations: Vec<Decl>,
  /// The interned `_`, which may be declared any number of times (**L§4.6**).
  discard: Option<Symbol>,
}

impl ScopeTree {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn push_scope(&mut self, kind: ScopeKind, parent: Option<ScopeId>) -> ScopeId {
    let id = ScopeId(self.scopes.len() as u32);
    self.scopes.push(Scope::new(kind, parent));
    if let Some(parent) = parent {
      self.scope_mut(parent).children.push(id);
      let inherited = self.scope(parent).source;
      self.scope_mut(id).source = inherited;
      let path = self.scope(parent).path.clone();
      self.scope_mut(id).path = path;
    }
    id
  }

  pub fn scope(&self, id: ScopeId) -> &Scope {
    &self.scopes[id.0 as usize]
  }

  pub fn scope_mut(&mut self, id: ScopeId) -> &mut Scope {
    &mut self.scopes[id.0 as usize]
  }

  pub fn decl(&self, id: DeclId) -> &Decl {
    &self.declarations[id.0 as usize]
  }

  pub fn scope_count(&self) -> usize {
    self.scopes.len()
  }

  pub fn declaration_count(&self) -> usize {
    self.declarations.len()
  }

  pub fn scope_ids(&self) -> impl Iterator<Item = ScopeId> {
    (0..self.scopes.len() as u32).map(ScopeId)
  }

  pub fn set_file(&mut self, id: ScopeId, source: SourceId, path: &Path) {
    let scope = self.scope_mut(id);
    scope.source = Some(source);
    scope.path = Some(path.to_path_buf());
  }

  /// Adds `decl` to its scope. Returns the declaration it collides with, if the
  /// scope already holds a non-overloadable name (**L§4.3**).
  pub fn declare(&mut self, decl: Decl) -> Result<DeclId, DeclId> {
    let id = DeclId(self.declarations.len() as u32);
    let scope = decl.scope;
    let name = decl.name;
    let clash = self
      .scope(scope)
      .names
      .get(&name)
      .and_then(|existing| self.first_collision(existing, &decl));

    self.declarations.push(decl);
    let scope = self.scope_mut(scope);
    scope.names.entry(name).or_default().push(id);
    scope.declarations.push(id);

    match clash {
      Some(previous) => Err(previous),
      None => Ok(id),
    }
  }

  /// Procedures overload (**L§7.7**); a declaration from an unfolded `#if`
  /// branch never collides, since only one branch will survive, and neither
  /// does a `#placeholder`, whose whole purpose is to be replaced.
  fn first_collision(&self, existing: &[DeclId], decl: &Decl) -> Option<DeclId> {
    if decl.conditional || decl.kind == DeclKind::Placeholder || self.discard == Some(decl.name) {
      return None;
    }
    existing.iter().copied().find(|previous| {
      let previous = self.decl(*previous);
      !previous.conditional
        && previous.kind != DeclKind::Placeholder
        && !(previous.overloadable && decl.overloadable)
    })
  }

  /// Tells the tree which symbol is the discard identifier `_` (**L§4.6**).
  pub fn set_discard_name(&mut self, name: Symbol) {
    self.discard = Some(name);
  }

  pub fn set_decl_kind(&mut self, id: DeclId, kind: DeclKind) {
    self.declarations[id.0 as usize].kind = kind;
  }

  pub fn add_import(&mut self, scope: ScopeId, edge: ImportEdge) {
    self.scope_mut(scope).imports.push(edge);
  }

  pub fn add_pending(&mut self, scope: ScopeId, provider: PendingProvider) {
    let pending = &mut self.scope_mut(scope).pending;
    if !pending.contains(&provider) {
      pending.push(provider);
    }
  }

  /// Looks `name` up from `scope` outward (**L§4.3**): the lexical chain, and
  /// at each step the exported names of that scope's unnamed imports.
  pub fn lookup(&self, scope: ScopeId, name: Symbol) -> Resolution {
    let mut pending = false;
    let mut current = Some(scope);

    while let Some(id) = current {
      let scope = self.scope(id);
      if let Some(found) = scope.names.get(&name) {
        return Resolution::Found(found.clone());
      }
      if let Some(found) = self.lookup_through_imports(id, name) {
        return Resolution::Found(found);
      }
      pending |= scope.has_pending_providers();
      current = scope.parent;
    }

    if pending {
      Resolution::Pending
    } else {
      Resolution::Undeclared
    }
  }

  /// The exported declarations of `name` in the scopes `scope` imports, plus
  /// whatever those scopes made their own with a transitive `using`. What a
  /// scope merely `#import`ed stops there (**L§11.2**).
  fn lookup_through_imports(&self, scope: ScopeId, name: Symbol) -> Option<Vec<DeclId>> {
    let mut found = Vec::new();
    for edge in &self.scope(scope).imports {
      if edge.admits(name) {
        self.collect_exports(edge.target, name, &mut Vec::new(), &mut found);
      }
    }
    (!found.is_empty()).then_some(found)
  }

  fn collect_exports(
    &self,
    scope: ScopeId,
    name: Symbol,
    visiting: &mut Vec<ScopeId>,
    found: &mut Vec<DeclId>,
  ) {
    if visiting.contains(&scope) {
      return;
    }
    visiting.push(scope);

    if let Some(candidates) = self.scope(scope).names.get(&name) {
      found.extend(
        candidates
          .iter()
          .copied()
          .filter(|id| self.decl(*id).visibility.is_exported()),
      );
    }
    for edge in &self.scope(scope).imports {
      if edge.transitive && edge.admits(name) {
        self.collect_exports(edge.target, name, visiting, found);
      }
    }

    visiting.pop();
  }

  pub fn enclosing_module(&self, scope: ScopeId) -> Option<ScopeId> {
    let mut current = Some(scope);
    while let Some(id) = current {
      if matches!(self.scope(id).kind, ScopeKind::Module | ScopeKind::Preload) {
        return Some(id);
      }
      current = self.scope(id).parent;
    }
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn decl(name: Symbol, scope: ScopeId, kind: DeclKind) -> Decl {
    Decl {
      name,
      scope,
      kind,
      visibility: Visibility::Export,
      flags: DeclarationFlags::empty(),
      source: None,
      span: Span::at(0),
      node: None,
      conditional: false,
      overloadable: kind == DeclKind::Procedure,
    }
  }

  #[test]
  fn lookup_walks_outward_to_preload() {
    let interner = oj_lexer::Interner::new();
    let outer = interner.intern(b"print");
    let inner = interner.intern(b"x");

    let mut tree = ScopeTree::new();
    let preload = tree.push_scope(ScopeKind::Preload, None);
    let module = tree.push_scope(ScopeKind::Module, Some(preload));
    let block = tree.push_scope(ScopeKind::Imperative, Some(module));

    tree
      .declare(decl(outer, preload, DeclKind::Procedure))
      .unwrap();
    let local = tree
      .declare(decl(inner, block, DeclKind::Variable))
      .unwrap();

    assert_eq!(tree.lookup(block, inner), Resolution::Found(vec![local]));
    assert!(matches!(tree.lookup(block, outer), Resolution::Found(_)));
    assert_eq!(tree.lookup(preload, inner), Resolution::Undeclared);
  }

  #[test]
  fn an_inner_declaration_shadows_an_outer_one() {
    let interner = oj_lexer::Interner::new();
    let name = interner.intern(b"length");

    let mut tree = ScopeTree::new();
    let module = tree.push_scope(ScopeKind::Module, None);
    let block = tree.push_scope(ScopeKind::Imperative, Some(module));

    tree
      .declare(decl(name, module, DeclKind::Procedure))
      .unwrap();
    let local = tree.declare(decl(name, block, DeclKind::Variable)).unwrap();

    assert_eq!(tree.lookup(block, name), Resolution::Found(vec![local]));
  }

  #[test]
  fn redeclaring_a_name_in_one_scope_collides_unless_both_are_procedures() {
    let interner = oj_lexer::Interner::new();
    let name = interner.intern(b"draw");

    let mut tree = ScopeTree::new();
    let scope = tree.push_scope(ScopeKind::File, None);

    let first = tree
      .declare(decl(name, scope, DeclKind::Procedure))
      .unwrap();
    assert!(tree.declare(decl(name, scope, DeclKind::Procedure)).is_ok());
    assert_eq!(
      tree.declare(decl(name, scope, DeclKind::Variable)),
      Err(first)
    );
  }

  #[test]
  fn a_declaration_from_an_unfolded_static_if_never_collides() {
    let interner = oj_lexer::Interner::new();
    let name = interner.intern(b"handle");

    let mut tree = ScopeTree::new();
    let scope = tree.push_scope(ScopeKind::File, None);

    let mut conditional = decl(name, scope, DeclKind::Variable);
    conditional.conditional = true;
    assert!(tree.declare(conditional.clone()).is_ok());
    assert!(tree.declare(conditional).is_ok());
    assert!(tree.declare(decl(name, scope, DeclKind::Variable)).is_ok());
  }

  #[test]
  fn an_import_exposes_only_exported_names_and_does_not_re_export_them() {
    let interner = oj_lexer::Interner::new();
    let exported = interner.intern(b"print");
    let private = interner.intern(b"helper");

    let mut tree = ScopeTree::new();
    let preload = tree.push_scope(ScopeKind::Preload, None);
    let basic = tree.push_scope(ScopeKind::Module, Some(preload));
    let string = tree.push_scope(ScopeKind::Module, Some(preload));
    let main = tree.push_scope(ScopeKind::Module, Some(preload));

    let print = tree
      .declare(decl(exported, basic, DeclKind::Procedure))
      .unwrap();
    let mut hidden = decl(private, basic, DeclKind::Procedure);
    hidden.visibility = Visibility::Module;
    tree.declare(hidden).unwrap();

    let edge = |target| ImportEdge {
      target,
      only: Vec::new(),
      except: Vec::new(),
      span: Span::at(0),
      source: None,
      transitive: false,
    };
    tree.add_import(string, edge(basic));
    tree.add_import(main, edge(string));

    assert_eq!(
      tree.lookup(string, exported),
      Resolution::Found(vec![print])
    );
    assert_eq!(tree.lookup(string, private), Resolution::Undeclared);
    assert_eq!(tree.lookup(main, exported), Resolution::Undeclared);
  }

  #[test]
  fn an_except_filter_hides_the_names_it_names() {
    let interner = oj_lexer::Interner::new();
    let hidden = interner.intern(b"Node");
    let shown = interner.intern(b"Tree");

    let mut tree = ScopeTree::new();
    let trees = tree.push_scope(ScopeKind::Module, None);
    let main = tree.push_scope(ScopeKind::Module, None);
    tree.declare(decl(hidden, trees, DeclKind::Struct)).unwrap();
    tree.declare(decl(shown, trees, DeclKind::Struct)).unwrap();
    tree.add_import(
      main,
      ImportEdge {
        target: trees,
        only: Vec::new(),
        except: vec![hidden],
        span: Span::at(0),
        source: None,
        transitive: false,
      },
    );

    assert!(matches!(tree.lookup(main, shown), Resolution::Found(_)));
    assert_eq!(tree.lookup(main, hidden), Resolution::Undeclared);
  }

  #[test]
  fn a_pending_provider_turns_a_miss_into_a_wait() {
    let interner = oj_lexer::Interner::new();
    let name = interner.intern(b"member");

    let mut tree = ScopeTree::new();
    let file = tree.push_scope(ScopeKind::File, None);
    let block = tree.push_scope(ScopeKind::Imperative, Some(file));

    assert_eq!(tree.lookup(block, name), Resolution::Undeclared);
    tree.add_pending(file, PendingProvider::Using);
    assert_eq!(tree.lookup(block, name), Resolution::Pending);
  }

  #[test]
  fn mutually_importing_modules_do_not_borrow_each_other_s_imports() {
    let interner = oj_lexer::Interner::new();
    let name = interner.intern(b"deep");

    let mut tree = ScopeTree::new();
    let a = tree.push_scope(ScopeKind::Module, None);
    let b = tree.push_scope(ScopeKind::Module, None);
    let c = tree.push_scope(ScopeKind::Module, None);
    tree.declare(decl(name, c, DeclKind::Procedure)).unwrap();

    let edge = |target| ImportEdge {
      target,
      only: Vec::new(),
      except: Vec::new(),
      span: Span::at(0),
      source: None,
      transitive: false,
    };
    tree.add_import(a, edge(b));
    tree.add_import(b, edge(a));
    tree.add_import(b, edge(c));

    assert!(matches!(tree.lookup(b, name), Resolution::Found(_)));
    assert_eq!(tree.lookup(a, name), Resolution::Undeclared);
  }
}
