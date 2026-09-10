//! The compile-time execution engine (**L§12.1**, `docs/spec.md` §6.5).
//!
//! One `#run` becomes one program, one LLVM module and one call:
//!
//! 1. `oj-ir` lowers the run into a program whose entry point is
//!    `void __oj_run_N(void *result)`, reachable from the run rather than from
//!    `main`, so only what the run actually calls is built.
//! 2. Every global that program reached is given storage in `oj-runtime`'s
//!    data segments and bound in the JIT dylib as an absolute symbol — which
//!    is what makes two `#run`s see one `x`.
//! 3. The module goes into the JIT and the entry point is called with a
//!    buffer, which is then read back as a constant the front end can use.
//!
//! A run whose expression already folded never gets here: the checker answers
//! it without executing anything (**C§14**).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use oj_diag::Diagnostic;
use oj_ir::{Constant, Global, GlobalInit, Library, ProcedureFlags, Program};
use oj_runtime::{Segment, Segments};
use oj_scope::DeclId;
use oj_sema::{
  Checker, CompileTime, ModifyOutcome, ModifyRequest, RunBytes, RunOutcome, RunRequest,
};
use oj_types::{FloatKind, IntKind, TypeId, TypeKind, Types};

use crate::orc::Orc;

/// What a compilation's compile-time executions share: the JIT they are added
/// to and the data segments their globals live in.
pub struct Engine {
  orc: Orc,
  options: oj_codegen::Options,
  state: RefCell<State>,
}

#[derive(Default)]
struct State {
  segments: Segments,
  /// The symbols already bound in the JIT dylib.
  defined: HashSet<String>,
  /// Where each declaration's storage went, so that a `#no_reset` global can
  /// be read back at the end (**L§12.3**).
  bound: HashMap<DeclId, String>,
  /// The libraries already loaded into the compiler process, so that a
  /// `#foreign` procedure a run calls resolves (**L§12.1**).
  libraries: Vec<String>,
}

impl Engine {
  pub fn new() -> Result<Self, String> {
    let options = oj_codegen::Options {
      purpose: oj_codegen::Purpose::CompileTime,
      module_name: String::from("oj.compile_time"),
      ..oj_codegen::Options::default()
    };
    let machine = oj_codegen::target_machine(&options)?;
    Ok(Self {
      orc: Orc::new(machine)?,
      options,
      state: RefCell::new(State::default()),
    })
  }

  /// The bytes a `#no_reset` global holds now, which is what the executable
  /// starts it with (**L§12.3**).
  pub fn kept_bytes(&self, decl: DeclId) -> Option<Vec<u8>> {
    let state = self.state.borrow();
    let symbol = state.bound.get(&decl)?;
    state.segments.bytes(symbol).map(<[u8]>::to_vec)
  }

  /// Puts every writable global back the way the front end laid it out, which
  /// is what the reference does before it writes the executable (**L§12.3**).
  pub fn reset_globals(&self) {
    self.state.borrow_mut().segments.reset();
  }

  fn run(&self, checker: &mut Checker, request: &RunRequest) -> Result<RunOutcome, String> {
    let run = oj_ir::Run {
      source: request.source,
      scope: request.scope,
      header: request.header,
      block: request.block,
      result: request.result,
      results: request.results.clone(),
      value: request.value,
      symbol: request.symbol.clone(),
    };
    // Lowering asks the checker its usual questions, and one of those answers
    // may be another `#run` — so nothing of this engine is borrowed here.
    let lowered = oj_ir::lower_run(checker, &run);
    if lowered.has_errors() {
      for diagnostic in lowered.diagnostics {
        checker.push_diagnostic(diagnostic);
      }
      // The complaint is about code somewhere else entirely — whatever the run
      // called — so it needs the run to point back at (**C§12**).
      checker.push_diagnostic(Diagnostic::info(
        request.source,
        request.span,
        "This is the '#run' that needed it.",
      ));
      return Err(String::new());
    }

    self.prepare_data(&lowered.program)?;
    self.bind_intrinsics(&lowered.program)?;
    self.load_libraries(&lowered.program.libraries);

    let module = self.orc.context().create_module(&request.symbol);
    let module = oj_codegen::build_module(module, &lowered.program, &self.options)?;
    self.orc.add_module(module)?;
    let address = self.orc.lookup(&request.symbol)?;

    // A run behind a compound declaration writes every value it produced into
    // one buffer, each where the shared layout says (**L§4.5**).
    let (offsets, size, alignment) = oj_ir::run_result_layout(checker, &request.results);
    // A `Type` a run produced is an address into the type table image, so it
    // is read back through where that image ended up (**L§17**).
    let table = lowered
      .program
      .type_table
      .symbol
      .as_deref()
      .and_then(|symbol| self.state.borrow().segments.address(symbol))
      .map(|base| (base as u64, &lowered.program.type_table));
    // A run that asks where a struct was written needs that worked out before
    // it starts: only this side knows where the image was placed, and the
    // checker is not reachable from inside the call (**C§3.3**).
    if let Some((base, image)) = table {
      install_struct_locations(checker, &lowered.program, base, image);
    }

    let mut buffer = Buffer::new(size, alignment);
    let entry: extern "C" fn(*mut u8) = unsafe { std::mem::transmute(address as usize) };
    entry(buffer.as_mut_ptr());

    if request.result == TypeId::VOID {
      return Ok(RunOutcome::Void);
    }
    // Several values means a compound declaration is receiving them, and each
    // one is read out of the slot the wrapper wrote it to (**L§4.5**).
    if request.results.len() > 1 {
      let mut values = Vec::with_capacity(request.results.len());
      for (type_id, offset) in request.results.iter().zip(&offsets) {
        let size = checker.layout(*type_id).map_or(0, |layout| layout.size) as usize;
        let start = *offset as usize;
        let bytes = buffer
          .as_slice()
          .get(start..start + size)
          .ok_or_else(|| String::from("the run's values did not fit the storage it was given"))?;
        let value = read_value(checker.types(), *type_id, bytes).ok_or_else(|| {
          String::from("orangejuice cannot bring this kind of value back from compile time yet")
        })?;
        values.push(value);
      }
      return Ok(RunOutcome::Values(values));
    }
    if checker.types().underlying(request.result) == TypeId::TYPE {
      return match table.and_then(|(base, image)| read_type(base, image, buffer.as_slice())) {
        Some(type_id) => Ok(RunOutcome::Value(oj_sema::Const::type_value(type_id))),
        None => Err(String::from(
          "orangejuice cannot bring this kind of value back from compile time yet",
        )),
      };
    }
    // A `Code` a run produced is the address of the `Code_Node` the compiler
    // exported, which is the piece of program it names (**L§13.1**).
    if checker.types().underlying(request.result) == TypeId::CODE {
      let bytes = buffer.as_slice();
      let address = match bytes.len() >= 8 {
        true => usize::from_ne_bytes(bytes[..8].try_into().expect("eight bytes")),
        false => 0,
      };
      // A tree the metaprogram edited is handed back as the source it now
      // reads as, which the `#insert` parses and splices where it stands
      // (**C§3.3**); one nothing touched is the program it already is.
      if let Some(text) = checker.rewritten_code_at(address) {
        return Ok(RunOutcome::Value(oj_sema::Const::new(
          request.result,
          oj_sema::Value::String(text.into_bytes().into_boxed_slice()),
        )));
      }
      return match checker.code_at_address(address) {
        Some((source, node, scope)) => Ok(RunOutcome::Value(oj_sema::Const::new(
          request.result,
          oj_sema::Value::Code {
            source,
            node,
            scope,
          },
        ))),
        None if address == 0 => Ok(RunOutcome::Value(oj_sema::Const::new(
          request.result,
          oj_sema::Value::Null,
        ))),
        None => Err(String::from(
          "orangejuice cannot bring this kind of value back from compile time yet",
        )),
      };
    }
    match read_value(checker.types(), request.result, buffer.as_slice()) {
      Some(value) => Ok(RunOutcome::Value(value)),
      None => Err(String::from(
        "orangejuice cannot bring this kind of value back from compile time yet",
      )),
    }
  }

  /// Runs one `#modify` block (**L§7.8**): the polymorph variables go in as
  /// values and whether the candidate stands — and what they became — comes
  /// back.
  fn run_modify(
    &self,
    checker: &mut Checker,
    request: &ModifyRequest,
  ) -> Result<ModifyOutcome, String> {
    let modify = oj_ir::Modify {
      source: request.source,
      scope: request.scope,
      block: request.block,
      instance: request.instance,
      variables: request.variables.clone(),
      symbol: request.symbol.clone(),
    };
    let lowered = oj_ir::lower_modify(checker, &modify);
    if lowered.has_errors() {
      for diagnostic in lowered.diagnostics {
        checker.push_diagnostic(diagnostic);
      }
      checker.push_diagnostic(Diagnostic::info(
        request.source,
        request.span,
        "This is the '#modify' that needed it.",
      ));
      return Err(String::new());
    }

    self.prepare_data(&lowered.program)?;
    self.bind_intrinsics(&lowered.program)?;
    self.load_libraries(&lowered.program.libraries);

    let module = self.orc.context().create_module(&request.symbol);
    let module = oj_codegen::build_module(module, &lowered.program, &self.options)?;
    self.orc.add_module(module)?;
    let address = self.orc.lookup(&request.symbol)?;

    let size = oj_ir::modify_result_size(request.variables.len());
    let mut buffer = Buffer::new(size, 8);
    let entry: extern "C" fn(*mut u8) = unsafe { std::mem::transmute(address as usize) };
    entry(buffer.as_mut_ptr());

    let bytes = buffer.as_slice();
    let accepted =
      read_integer(IntKind::S64, &bytes[oj_ir::MODIFY_ACCEPT as usize..]).unwrap_or(1) != 0;
    if !accepted {
      let reason = read_value(
        checker.types(),
        TypeId::STRING,
        &bytes[oj_ir::MODIFY_REASON as usize..],
      );
      let reason = match reason.map(|value| value.value) {
        Some(oj_sema::Value::String(text)) => String::from_utf8_lossy(&text).into_owned(),
        _ => String::new(),
      };
      return Ok(ModifyOutcome::Rejected(reason));
    }

    let table = lowered
      .program
      .type_table
      .symbol
      .as_deref()
      .and_then(|symbol| self.state.borrow().segments.address(symbol))
      .map(|base| (base as u64, &lowered.program.type_table));
    let mut values = Vec::with_capacity(request.variables.len());
    for (index, variable) in request.variables.iter().enumerate() {
      let at = (oj_ir::MODIFY_VARIABLES + oj_ir::MODIFY_VARIABLE_SIZE * index as u64) as usize;
      let slot = &bytes[at..];
      // A variable the block left alone keeps exactly what it had, whether or
      // not the answer can be read back.
      let value = match checker.types().underlying(variable.type_id) == TypeId::TYPE {
        true => table
          .and_then(|(base, image)| read_type(base, image, slot))
          .map(oj_sema::Const::type_value),
        false => read_value(checker.types(), variable.type_id, slot),
      };
      values.push(value.unwrap_or_else(|| variable.value.clone()));
    }
    Ok(ModifyOutcome::Accepted(values))
  }

  /// Binds the `#compiler` procedures a run reached to the compiler's own
  /// answers (**C§3.3**). A symbol this compiler does not answer is left
  /// undefined, so the run fails naming it rather than calling nothing.
  fn bind_intrinsics(&self, program: &Program) -> Result<(), String> {
    for procedure in &program.procedures {
      if !procedure.flags.contains(ProcedureFlags::COMPILER) {
        continue;
      }
      {
        let mut state = self.state.borrow_mut();
        if !state.defined.insert(procedure.symbol.clone()) {
          continue;
        }
      }
      let Some(address) = oj_meta::intrinsic(&procedure.symbol) else {
        return Err(format!(
          "'{}' is a compiler procedure orangejuice does not answer yet",
          procedure.name
        ));
      };
      self.orc.define(&procedure.symbol, address as u64)?;
    }
    Ok(())
  }
  /// Gives every global the run reached storage the JIT can bind to. A global
  /// already bound keeps the address — and the contents — it had.
  fn prepare_data(&self, program: &Program) -> Result<(), String> {
    let mut state = self.state.borrow_mut();
    for global in &program.globals {
      // An `#elsewhere` global belongs to a library, so the process is where
      // its definition comes from (**L§4.8**).
      if global.imported {
        continue;
      }
      if !state.defined.insert(global.symbol.clone()) {
        continue;
      }
      let segment = match &global.init {
        // The type table is never written to, so it is never reset either
        // (**L§17**).
        GlobalInit::Image { .. } => Segment::ReadOnly,
        _ if global.no_reset => Segment::NoReset,
        _ => Segment::Writable,
      };
      let initial = initial_bytes(&program.types, global);
      let address = state.segments.allocate(
        &global.symbol,
        global.size,
        global.alignment,
        segment,
        &initial,
      );
      // An image's pointers into itself only become addresses once it has one.
      if let GlobalInit::Image { relocations, .. } = &global.init {
        for (at, target) in relocations {
          state
            .segments
            .write_pointer(&global.symbol, *at, address as u64 + *target);
        }
      }
      if let Some(decl) = global.decl {
        state.bound.insert(decl, global.symbol.clone());
      }
      self.orc.define(&global.symbol, address as u64)?;
    }
    Ok(())
  }

  /// `dlopen`s the libraries a run's `#foreign` procedures named, so that the
  /// JIT's process search generator can find their symbols (`docs/spec.md`
  /// §6.5).
  fn load_libraries(&self, libraries: &[Library]) {
    for library in libraries {
      {
        let state = self.state.borrow();
        if state.libraries.contains(&library.name) {
          continue;
        }
      }
      self.state.borrow_mut().libraries.push(library.name.clone());
      let file = format!("lib{}.so", library.name);
      let candidates = library
        .directory
        .iter()
        .map(|directory| directory.join(&file))
        .chain([std::path::PathBuf::from(&file)]);
      for candidate in candidates {
        if let Ok(handle) = unsafe { libloading::Library::new(&candidate) } {
          // The symbols have to stay in the process for as long as anything
          // JIT-compiled might call them, which is until the compiler exits.
          std::mem::forget(handle);
          break;
        }
      }
    }
  }
}

impl CompileTime for Engine {
  fn evaluate(&self, checker: &mut Checker, request: &RunRequest) -> RunOutcome {
    match self.run(checker, request) {
      Ok(outcome) => outcome,
      Err(message) => {
        if !message.is_empty() {
          checker.push_diagnostic(Diagnostic::error(
            request.source,
            request.span,
            format!("This '#run' could not be executed: {message}."),
          ));
        }
        RunOutcome::Failed
      }
    }
  }

  fn modify(&self, checker: &mut Checker, request: &ModifyRequest) -> ModifyOutcome {
    match self.run_modify(checker, request) {
      Ok(outcome) => outcome,
      Err(message) => {
        if !message.is_empty() {
          checker.push_diagnostic(Diagnostic::error(
            request.source,
            request.span,
            format!("This '#modify' could not be executed: {message}."),
          ));
        }
        ModifyOutcome::Failed
      }
    }
  }
}

/// Storage for one run's result, aligned the way the type asks.
struct Buffer {
  pointer: *mut u8,
  layout: std::alloc::Layout,
}

impl Buffer {
  fn new(size: usize, alignment: usize) -> Self {
    let layout = std::alloc::Layout::from_size_align(size, alignment)
      .unwrap_or_else(|_| std::alloc::Layout::from_size_align(size, 1).expect("a byte layout"));
    let pointer = unsafe { std::alloc::alloc_zeroed(layout) };
    assert!(!pointer.is_null(), "out of memory for a '#run' result");
    Self { pointer, layout }
  }

  fn as_mut_ptr(&mut self) -> *mut u8 {
    self.pointer
  }

  fn as_slice(&self) -> &[u8] {
    unsafe { std::slice::from_raw_parts(self.pointer, self.layout.size()) }
  }
}

impl Drop for Buffer {
  fn drop(&mut self) {
    unsafe { std::alloc::dealloc(self.pointer, self.layout) };
  }
}

/// The bytes a global starts compile time with: whatever the front end folded,
/// zeroes otherwise. A global the front end could not fold is left zero, and
/// the reference's own answer — evaluating the initializer at compile time —
/// waits for the same scheduler M8 needs (`docs/spec.md` §10).
fn initial_bytes(types: &Types, global: &Global) -> Vec<u8> {
  let size = global.size.max(1) as usize;
  let mut bytes = vec![0u8; size];
  match &global.init {
    GlobalInit::Constant(constant) => write_constant(types, global.type_id, constant, &mut bytes),
    GlobalInit::Bytes(source) | GlobalInit::Image { bytes: source, .. } => copy(source, &mut bytes),
    GlobalInit::Zero => {}
  }
  bytes
}

fn write_constant(types: &Types, type_id: TypeId, constant: &Constant, bytes: &mut [u8]) {
  let underlying = types.underlying(type_id);
  match constant {
    Constant::Int(value) => write_integer(*value, bytes),
    Constant::Bool(value) => write_integer(i128::from(*value), bytes),
    Constant::Float(value) => match types.float_kind(underlying) {
      Some(FloatKind::F32) => copy(&(*value as f32).to_le_bytes(), bytes),
      _ => copy(&value.to_le_bytes(), bytes),
    },
    Constant::Bytes { bytes: source, .. } => copy(source, bytes),
    Constant::Null | Constant::Zero => {}
    // A string in a global would need its characters somewhere the JIT can
    // point at; the reference puts them in the read-only segment, which is
    // what the type table will need too (`docs/spec.md` §6.5).
    Constant::String(_) => {}
  }
}

fn write_integer(value: i128, bytes: &mut [u8]) {
  copy(&value.to_le_bytes(), bytes);
}

fn copy(source: &[u8], destination: &mut [u8]) {
  let count = source.len().min(destination.len());
  destination[..count].copy_from_slice(&source[..count]);
}

/// Reads a run's result out of the buffer it wrote (**L§12.1**). `None` means
/// the value is of a kind orangejuice cannot turn into a front-end constant.
/// The type whose record a compile-time `Type` value points at (**L§17**).
fn read_type(base: u64, image: &oj_ir::TypeTableImage, bytes: &[u8]) -> Option<TypeId> {
  let address = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
  image.type_at(address.checked_sub(base)?)
}

fn read_value(types: &Types, type_id: TypeId, bytes: &[u8]) -> Option<oj_sema::Const> {
  use oj_sema::{Const, Value};
  let underlying = types.underlying(type_id);
  if let Some(kind) = types.integer_kind(underlying) {
    return Some(Const::new(type_id, Value::Int(read_integer(kind, bytes)?)));
  }
  if let Some(definition) = types.enum_of(underlying) {
    let base = types.enum_info(definition).base;
    let kind = types.integer_kind(types.underlying(base))?;
    return Some(Const::new(type_id, Value::Int(read_integer(kind, bytes)?)));
  }
  match types.kind(underlying) {
    TypeKind::Bool => Some(Const::new(type_id, Value::Bool(*bytes.first()? != 0))),
    TypeKind::Float(FloatKind::F32) => Some(Const::new(
      type_id,
      Value::Float(f64::from(f32::from_le_bytes(
        bytes.get(..4)?.try_into().ok()?,
      ))),
    )),
    TypeKind::Float(FloatKind::F64) => Some(Const::new(
      type_id,
      Value::Float(f64::from_le_bytes(bytes.get(..8)?.try_into().ok()?)),
    )),
    TypeKind::String => {
      let count = i64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
      let data = u64::from_le_bytes(bytes.get(8..16)?.try_into().ok()?);
      if count < 0 {
        return None;
      }
      if count == 0 || data == 0 {
        return Some(Const::string(Box::default()));
      }
      // The characters live in memory the JIT owns, so they are copied out
      // before anything else can reuse it.
      let text = unsafe { std::slice::from_raw_parts(data as *const u8, count as usize) }.to_vec();
      Some(Const::string(text.into_boxed_slice()))
    }
    // Everything else is storage: the run wrote its bytes into the buffer, and
    // those bytes are the constant (**L§12.1**). A pointer among them that
    // names storage `add_global_data` created is carried along with what it
    // names, so that the executable gets both (**C§3.3**).
    TypeKind::Struct(_) | TypeKind::Array { .. } => Some(Const::new(
      type_id,
      Value::Bytes(RunBytes {
        data: bytes.into(),
        links: compile_time_links(bytes),
      }),
    )),
    _ => None,
  }
}

fn read_integer(kind: IntKind, bytes: &[u8]) -> Option<i128> {
  let size = kind.size() as usize;
  let slice = bytes.get(..size)?;
  let mut buffer = [0u8; 16];
  buffer[..size].copy_from_slice(slice);
  let value = u128::from_le_bytes(buffer);
  let bits = size * 8;
  if kind.is_signed() && bits < 128 && (value >> (bits - 1)) & 1 == 1 {
    return Some(value as i128 - (1i128 << bits));
  }
  Some(value as i128)
}

/// The pointers among a run's bytes that name storage `add_global_data`
/// created (**C§3.3**). Each one is carried along with what it points at, so
/// that the back end can lay that data down beside the bytes and point them at
/// it — which is what makes `image :: #run add_global_data(…)` reach the
/// program.
fn compile_time_links(bytes: &[u8]) -> Box<[oj_sema::RunLink]> {
  if bytes.len() < 8 {
    return Box::default();
  }
  let mut links = Vec::new();
  for at in (0..=bytes.len() - 8).step_by(8) {
    let Ok(word) = bytes[at..at + 8].try_into() else {
      continue;
    };
    let address = u64::from_le_bytes(word) as usize;
    if address == 0 {
      continue;
    }
    let found = oj_meta::with(|meta| {
      meta
        .global_data_at(address)
        .map(|(data, offset)| (data.bytes.clone(), offset))
    })
    .flatten();
    if let Some((data, offset)) = found {
      links.push(oj_sema::RunLink {
        at: at as u64,
        target: oj_sema::RunTarget::Data(data.into_boxed_slice()),
        offset,
      });
    }
  }
  links.into_boxed_slice()
}

/// Tells the compile-time code what the addresses in its own type table image
/// mean, but only when it asks (**C§3.3**): a `Type` value *is* one of those
/// addresses, and a `*Type_Info_Struct` names a struct written somewhere.
/// Working either out costs a lookup per type, and almost no `#run` asks.
fn install_struct_locations(
  checker: &mut Checker,
  program: &Program,
  base: u64,
  image: &oj_ir::TypeTableImage,
) {
  let binds = |symbol: &str| {
    program
      .procedures
      .iter()
      .any(|procedure| procedure.symbol == symbol)
  };
  let wants_locations = binds("compiler_get_struct_location");
  let wants_types = binds("compiler_set_type_info_flags");
  if !wants_locations && !wants_types {
    return;
  }
  let mut locations = HashMap::new();
  let mut types = HashMap::new();
  for (type_id, offset) in &image.offsets {
    let address = (base + offset) as usize;
    if wants_types {
      types.insert(address, type_id.0);
    }
    if wants_locations && let Some(location) = checker.struct_location(*type_id) {
      locations.insert(address, location);
    }
  }
  oj_meta::with(|meta| {
    meta.set_struct_locations(locations);
    meta.set_types_at(types);
  });
}
