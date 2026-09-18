//! User-space stack traces (**C§13**).
//!
//! `Build_Options.stack_trace` asks every Jai-convention procedure to keep a
//! `Stack_Trace_Node` of its own on the stack, linked into the one the context
//! points at, and to leave the line of the call it is making in that node. A
//! trace is then a linked list in ordinary memory, which is what `Basic`'s
//! `pack_stack_trace` walks and what `context.stack_trace` hands a program.
//!
//! The `Stack_Trace_Procedure_Info` a node points at is static: a global per
//! procedure, filled once by a generated `__oj_stack_trace_init` that the
//! entry point calls before anything else, since a procedure's address is only
//! known to the module that generates it (**L§5.11**).

use super::*;
use crate::ir::{AbiParameter, ParameterKind};

/// What the generated initializer writes into one procedure's info record.
#[derive(Clone, Debug)]
pub(super) struct TraceInfo {
  pub(super) procedure: ProcId,
  pub(super) global: GlobalId,
  pub(super) name: String,
  pub(super) path: String,
  pub(super) line: u32,
  pub(super) column: u32,
}

/// The node the procedure being lowered keeps, and what it has to put back.
#[derive(Clone, Copy, Debug)]
pub(super) struct TraceFrame {
  /// The node's storage.
  pub(super) local: LocalId,
  /// The `#Context` the node was linked into, which is the one the epilogue
  /// unlinks it from even inside a `push_context` (**L§6.9**).
  pub(super) context: ValueId,
  /// What `context.stack_trace` held on entry.
  pub(super) previous: ValueId,
}

/// The mixing constant the node hash is built from — a fast hash of the path
/// that got here rather than a cryptographic one, which is all the reference
/// promises (**C§13**).
const HASH_MULTIPLIER: i128 = 0x0100_0000_01b3;

impl Lowering<'_, '_> {
  /// The Preload types a trace is built out of, or `None` when the program
  /// does not have them — a file checked on its own has no Preload.
  fn trace_types(&mut self) -> Option<(TypeId, TypeId)> {
    if let Some(pair) = self.trace_types {
      return Some(pair);
    }
    let node = self.checker.preload_named_type("Stack_Trace_Node");
    let info = self
      .checker
      .preload_named_type("Stack_Trace_Procedure_Info");
    self.checker.layout(node);
    self.checker.layout(info);
    if self.checker.types().struct_of(node).is_none()
      || self.checker.types().struct_of(info).is_none()
    {
      return None;
    }
    self.trace_types = Some((node, info));
    Some((node, info))
  }

  /// The offset and type of one member of a struct type.
  fn member_of(&mut self, type_id: TypeId, name: &str) -> Option<(u64, TypeId)> {
    let underlying = self.checker.types().underlying(type_id);
    let definition = self.checker.types().struct_of(underlying)?;
    let symbol = self.checker.interner().intern(name.as_bytes());
    self
      .checker
      .types()
      .struct_info(definition)
      .member(symbol)
      .map(|member| (member.offset, member.type_id))
  }

  /// Whether the procedure in hand keeps a node at all: a `#c_call` or a
  /// `#no_context` has no context to link one into, and a `#foreign` body is
  /// not ours to write (**C§13**).
  fn keeps_a_trace(&mut self, flags: ProcedureFlags) -> bool {
    self.stack_trace
      && !flags
        .intersects(ProcedureFlags::FOREIGN | ProcedureFlags::NO_CONTEXT | ProcedureFlags::C_CALL)
      && self.context_value.is_some()
  }

  /// Links a node for the procedure being lowered into `context.stack_trace`.
  /// The linking is one call to a generated procedure rather than written out
  /// in every prologue: it is the same few dozen instructions for every
  /// procedure but the info record and the seed, and at `-O0` written out it
  /// was a seventh of a small program's code.
  pub(super) fn push_trace_node(
    &mut self,
    id: ProcId,
    flags: ProcedureFlags,
    at: (SourceId, Span),
  ) {
    self.trace_frame = None;
    if !self.keeps_a_trace(flags) {
      return;
    }
    let Some((node_type, info_type)) = self.trace_types() else {
      return;
    };
    let Some(context) = self.context_value else {
      return;
    };
    let Some((slot_offset, slot_type)) = self.member_of(self.context_type, "stack_trace") else {
      return;
    };
    let Some((next_offset, next_type)) = self.member_of(node_type, "next") else {
      return;
    };

    let global = self.trace_info_global(id, at);
    let local = self.new_local(String::from("stack_trace_node"), node_type);
    let address = self.local_address(local);
    let slot = self.offset(context, slot_offset, slot_type);
    let info_pointer = self.pointer_to(info_type);
    let info = self.value(info_pointer);
    self.emit(Inst::GlobalAddress { dest: info, global });
    let seed_type = self.trace_seed_type(node_type);
    let seed = self.constant(Constant::Int(trace_seed(id)), seed_type);
    let seed = self.scalar(seed);

    let push = self.reserve_trace_push(node_type, info_type, slot_type, seed_type);
    let signature = self.procedures[push.0 as usize].type_id;
    self.emit(Inst::Call {
      dest: None,
      callee: Callee::Direct(push),
      signature,
      arguments: vec![address, info, slot, seed],
    });

    let link = self.offset(address, next_offset, next_type);
    let previous = self.value(next_type);
    self.emit(Inst::Load {
      dest: previous,
      address: link,
    });
    self.trace_frame = Some(TraceFrame {
      local,
      context,
      previous,
    });
  }

  /// A procedure that makes no calls of its own keeps no node, the way the
  /// reference compiles one (**C§13**): nothing it runs can walk the list, so
  /// the node would only ever be read by the procedure itself, and it reads
  /// its caller's. Whether it calls anything is only known once the body is
  /// lowered, so the link it was given is taken back out here. `previous` is
  /// then read from the context instead of from the node, which makes the
  /// store every `return` does put back what was already there.
  pub(super) fn drop_leaf_trace_node(&mut self, id: ProcId) {
    let (Some(frame), Some(push)) = (self.trace_frame, self.trace_push) else {
      return;
    };
    let is_push = |instruction: &Inst| matches!(instruction, Inst::Call { callee: Callee::Direct(callee), .. } if *callee == push);
    let calls = self
      .blocks
      .iter()
      .flat_map(|block| &block.instructions)
      .filter(|instruction| matches!(instruction, Inst::Call { .. }) && !is_push(instruction))
      .count();
    if calls > 0 {
      return;
    }
    let Some(arguments) = self
      .blocks
      .iter()
      .flat_map(|block| &block.instructions)
      .find_map(|instruction| match instruction {
        Inst::Call { arguments, .. } if is_push(instruction) => Some(arguments.clone()),
        _ => None,
      })
    else {
      return;
    };
    let &[node, info, slot, seed] = arguments.as_slice() else {
      return;
    };
    let mut dead = vec![node, info, seed];
    for block in &mut self.blocks {
      for instruction in &mut block.instructions {
        if let Inst::Load { dest, address } = instruction
          && *dest == frame.previous
        {
          dead.push(*address);
          *address = slot;
        }
      }
    }
    for block in &mut self.blocks {
      block.instructions.retain(|instruction| {
        !is_push(instruction) && instruction.dest().is_none_or(|dest| !dead.contains(&dest))
      });
    }
    self.trace_infos.retain(|info| info.procedure != id);
  }

  /// The type the node's hash is kept in, which is what the seed is passed as.
  fn trace_seed_type(&mut self, node_type: TypeId) -> TypeId {
    self
      .member_of(node_type, "hash")
      .map_or(TypeId::U64, |(_, type_id)| type_id)
  }

  /// Reserves `__oj_trace_push(node, info, slot, seed)`, whose body is written
  /// once every procedure has been lowered.
  fn reserve_trace_push(
    &mut self,
    node_type: TypeId,
    info_type: TypeId,
    slot_type: TypeId,
    seed_type: TypeId,
  ) -> ProcId {
    if let Some(id) = self.trace_push {
      return id;
    }
    let node_pointer = self.pointer_to(node_type);
    let info_pointer = self.pointer_to(info_type);
    let slot_pointer = self.pointer_to(slot_type);
    let arguments = vec![node_pointer, info_pointer, slot_pointer, seed_type];
    let mut signature = oj_types::ProcedureType::new(arguments.clone(), Vec::new());
    signature.flags |= oj_types::ProcedureFlags::HAS_NO_CONTEXT;
    let type_id = self.checker.types_table_mut().procedure(signature);
    let parameters = arguments
      .iter()
      .map(|type_id| AbiParameter {
        type_id: *type_id,
        kind: ParameterKind::Value,
        class: None,
      })
      .collect();
    let id = ProcId(self.procedures.len() as u32);
    self.procedures.push(Procedure {
      symbol: String::from(TRACE_PUSH_SYMBOL),
      name: String::from(TRACE_PUSH_SYMBOL),
      type_id,
      parameters: arguments,
      returns: Vec::new(),
      flags: ProcedureFlags::COMPILER_GENERATED | ProcedureFlags::NO_CONTEXT,
      library: None,
      abi: Abi {
        parameters,
        direct_return: None,
        return_class: None,
        variadic: false,
        c_call: false,
      },
      locals: Vec::new(),
      blocks: Vec::new(),
      value_types: Vec::new(),
      entry: BlockId(0),
      location: None,
    });
    self.trace_push = Some(id);
    id
  }

  /// The body of `__oj_trace_push`: clear the node, point it at its info
  /// record and at the node it hides, count and hash the path, and make it the
  /// context's (**C§13**).
  pub(super) fn emit_trace_push(&mut self) {
    let Some(id) = self.trace_push else {
      return;
    };
    let Some((node_type, _)) = self.trace_types() else {
      return;
    };
    let Some((_, slot_type)) = self.member_of(self.context_type, "stack_trace") else {
      return;
    };
    let parameters = self.procedures[id.0 as usize].abi.parameters.clone();
    self.start_procedure();
    let incoming: Vec<ValueId> = parameters
      .iter()
      .map(|parameter| self.value(parameter.type_id))
      .collect();
    let &[address, info, slot, seed] = incoming.as_slice() else {
      return;
    };

    self.clear(address, node_type);
    let previous = self.value(slot_type);
    self.emit(Inst::Load {
      dest: previous,
      address: slot,
    });
    if let Some((offset, type_id)) = self.member_of(node_type, "next") {
      let field = self.offset(address, offset, type_id);
      self.emit(Inst::Store {
        address: field,
        value: previous,
      });
    }
    if let Some((offset, type_id)) = self.member_of(node_type, "info") {
      let field = self.offset(address, offset, type_id);
      self.emit(Inst::Store {
        address: field,
        value: info,
      });
    }
    self.fill_depth_and_hash(node_type, address, previous, slot_type, seed);
    self.emit(Inst::Store {
      address: slot,
      value: address,
    });
    self.terminate(Terminator::Return(Vec::new()));

    let procedure = &mut self.procedures[id.0 as usize];
    procedure.locals = std::mem::take(&mut self.locals);
    procedure.blocks = std::mem::take(&mut self.blocks);
    procedure.value_types = std::mem::take(&mut self.value_types);
    procedure.entry = BlockId(0);
  }

  /// `call_depth` counts from the outermost node and `hash` mixes the caller's
  /// in, so two calls of one procedure from two places hash differently
  /// (**C§13**). Both need to know whether there was a caller, which is one
  /// branch on the link.
  fn fill_depth_and_hash(
    &mut self,
    node_type: TypeId,
    address: ValueId,
    previous: ValueId,
    pointer_type: TypeId,
    seed: ValueId,
  ) {
    let depth = self.member_of(node_type, "call_depth");
    let hash = self.member_of(node_type, "hash");
    if depth.is_none() && hash.is_none() {
      return;
    }
    let null = self.constant(Constant::Null, pointer_type);
    let null = self.scalar(null);
    let condition = self.value(TypeId::BOOL);
    self.emit(Inst::Binary {
      dest: condition,
      operator: BinaryOp::NotEqual,
      left: previous,
      right: null,
    });
    let linked = self.new_block();
    let first = self.new_block();
    let join = self.new_block();
    self.terminate(Terminator::Branch {
      condition,
      then_block: linked,
      else_block: first,
    });

    self.current = linked;
    if let Some((offset, type_id)) = depth {
      let outer = self.offset(previous, offset, type_id);
      let loaded = self.value(type_id);
      self.emit(Inst::Load {
        dest: loaded,
        address: outer,
      });
      let one = self.constant(Constant::Int(1), type_id);
      let one = self.scalar(one);
      let sum = self.value(type_id);
      self.emit(Inst::Binary {
        dest: sum,
        operator: BinaryOp::Add,
        left: loaded,
        right: one,
      });
      let field = self.offset(address, offset, type_id);
      self.emit(Inst::Store {
        address: field,
        value: sum,
      });
    }
    if let Some((offset, type_id)) = hash {
      let outer = self.offset(previous, offset, type_id);
      let loaded = self.value(type_id);
      self.emit(Inst::Load {
        dest: loaded,
        address: outer,
      });
      let mixed = self.mix_hash(loaded, seed, type_id);
      let field = self.offset(address, offset, type_id);
      self.emit(Inst::Store {
        address: field,
        value: mixed,
      });
    }
    self.terminate(Terminator::Jump(join));

    self.current = first;
    if let Some((offset, type_id)) = depth {
      let one = self.constant(Constant::Int(1), type_id);
      let one = self.scalar(one);
      let field = self.offset(address, offset, type_id);
      self.emit(Inst::Store {
        address: field,
        value: one,
      });
    }
    if let Some((offset, type_id)) = hash {
      let field = self.offset(address, offset, type_id);
      self.emit(Inst::Store {
        address: field,
        value: seed,
      });
    }
    self.terminate(Terminator::Jump(join));
    self.current = join;
  }

  /// `(outer ^ seed) * prime`, which is the FNV step: cheap, and different for
  /// every path through the program.
  fn mix_hash(&mut self, outer: ValueId, seed: ValueId, type_id: TypeId) -> ValueId {
    let mixed = self.value(type_id);
    self.emit(Inst::Binary {
      dest: mixed,
      operator: BinaryOp::BitwiseXor,
      left: outer,
      right: seed,
    });
    let prime = self.constant(Constant::Int(HASH_MULTIPLIER), type_id);
    let prime = self.scalar(prime);
    let product = self.value(type_id);
    self.emit(Inst::Binary {
      dest: product,
      operator: BinaryOp::Multiply,
      left: mixed,
      right: prime,
    });
    product
  }

  /// Unlinks the node the procedure in hand pushed. Every `return` does this,
  /// which is why it is the returning that carries it rather than a defer.
  pub(super) fn pop_trace_node(&mut self) {
    let Some(frame) = self.trace_frame else {
      return;
    };
    let Some((offset, type_id)) = self.member_of(self.context_type, "stack_trace") else {
      return;
    };
    let slot = self.offset(frame.context, offset, type_id);
    self.emit(Inst::Store {
      address: slot,
      value: frame.previous,
    });
  }

  /// The line the procedure in hand is calling from, which is what a trace
  /// reports for the frame under it (**C§13**).
  pub(super) fn record_call_line(&mut self, source: SourceId, node: NodeId) {
    let Some(frame) = self.trace_frame else {
      return;
    };
    let Some((node_type, _)) = self.trace_types() else {
      return;
    };
    let Some((offset, type_id)) = self.member_of(node_type, "line_number") else {
      return;
    };
    let Some(line) = self.line_of(source, node) else {
      return;
    };
    let address = self.local_address(frame.local);
    let field = self.offset(address, offset, type_id);
    let value = self.constant(Constant::Int(i128::from(line)), type_id);
    let value = self.scalar(value);
    self.emit(Inst::Store {
      address: field,
      value,
    });
  }

  fn line_of(&mut self, source: SourceId, node: NodeId) -> Option<u32> {
    let span = self.checker.tree_of(source)?.node(node).span;
    Some(
      self
        .checker
        .program()
        .sources()
        .file(source)
        .location(span.start)
        .line,
    )
  }

  /// The static info record one procedure's nodes point at, created the first
  /// time that procedure is given a node.
  fn trace_info_global(&mut self, id: ProcId, at: (SourceId, Span)) -> GlobalId {
    if let Some(entry) = self
      .trace_infos
      .iter()
      .find(|entry| entry.procedure == id)
      .map(|entry| entry.global)
    {
      return entry;
    }
    let (_, info_type) = self.trace_types().unwrap_or((TypeId::VOID, TypeId::VOID));
    let (size, alignment) = self.size_align(info_type);
    let symbol = format!("__oj_trace_info${}", self.procedures[id.0 as usize].symbol);
    let global = GlobalId(self.globals.len() as u32);
    self.globals.push(Global {
      symbol: symbol.clone(),
      name: symbol,
      type_id: info_type,
      init: GlobalInit::Zero,
      size,
      alignment,
      decl: None,
      no_reset: false,
      external: false,
      imported: false,
    });

    let (source, span) = at;
    let (path, line, column) = match source == SourceId::NONE {
      true => (String::new(), 0, 0),
      false => {
        let file = self.checker.program().sources().file(source);
        let position = file.location(span.start);
        let path = std::path::absolute(file.path())
          .unwrap_or_else(|_| file.path().to_path_buf())
          .to_string_lossy()
          .into_owned();
        (path, position.line, position.column)
      }
    };
    let name = self.procedures[id.0 as usize].name.clone();
    // A generated procedure has no name of its own in a trace, which is what
    // the reference reports for a `#run`'s frame (**C§13**).
    let name = match self.procedures[id.0 as usize]
      .flags
      .contains(ProcedureFlags::COMPILER_GENERATED)
    {
      true => String::new(),
      false => name,
    };
    self.trace_infos.push(TraceInfo {
      procedure: id,
      global,
      name,
      path,
      line,
      column,
    });
    global
  }

  /// Reserves the generated initializer so that a caller built before the info
  /// records exist can already name it. A `#run`'s wrapper needs that: the JIT
  /// calls the wrapper directly, so the wrapper is what has to fill the records
  /// (**C§13**).
  pub(super) fn reserve_trace_init(&mut self, symbol: String) -> ProcId {
    if let Some(id) = self.trace_init {
      return id;
    }
    let type_id = self
      .checker
      .types_table_mut()
      .procedure(oj_types::ProcedureType::new(Vec::new(), Vec::new()));
    let id = ProcId(self.procedures.len() as u32);
    self.procedures.push(Procedure {
      symbol: symbol.clone(),
      name: symbol,
      type_id,
      parameters: Vec::new(),
      returns: Vec::new(),
      flags: ProcedureFlags::COMPILER_GENERATED | ProcedureFlags::NO_CONTEXT,
      library: None,
      abi: Abi {
        parameters: Vec::new(),
        direct_return: None,
        return_class: None,
        variadic: false,
        c_call: false,
      },
      locals: Vec::new(),
      blocks: Vec::new(),
      value_types: Vec::new(),
      entry: BlockId(0),
      location: None,
    });
    self.trace_init = Some(id);
    id
  }

  /// Fills every info record, in a generated procedure the entry point calls
  /// before the program (**C§13**).
  pub(super) fn emit_trace_info_init(&mut self) {
    if self.trace_infos.is_empty() && self.trace_init.is_none() {
      return;
    }
    let Some((_, info_type)) = self.trace_types() else {
      return;
    };
    let pointer = self.pointer_to(info_type);
    self.start_procedure();

    let infos = self.trace_infos.clone();
    for info in infos {
      let base = self.value(pointer);
      self.emit(Inst::GlobalAddress {
        dest: base,
        global: info.global,
      });
      if let Some((offset, type_id)) = self.member_of(info_type, "name") {
        let field = self.offset(base, offset, type_id);
        let value = self.string_constant(info.name.clone().into_bytes().into());
        self.store(field, value);
      }
      if let Some((offset, type_id)) = self.member_of(info_type, "procedure_address") {
        let field = self.offset(base, offset, type_id);
        let address = self.value(type_id);
        self.emit(Inst::ProcedureAddress {
          dest: address,
          procedure: info.procedure,
        });
        self.emit(Inst::Store {
          address: field,
          value: address,
        });
      }
      let Some((location_offset, location_type)) = self.member_of(info_type, "location") else {
        continue;
      };
      let location = self.offset(base, location_offset, location_type);
      if let Some((offset, type_id)) = self.member_of(location_type, "fully_pathed_filename") {
        let field = self.offset(location, offset, type_id);
        let value = self.string_constant(info.path.clone().into_bytes().into());
        self.store(field, value);
      }
      for (name, number) in [
        ("line_number", info.line),
        ("character_number", info.column),
      ] {
        let Some((offset, type_id)) = self.member_of(location_type, name) else {
          continue;
        };
        let field = self.offset(location, offset, type_id);
        let value = self.constant(Constant::Int(i128::from(number)), type_id);
        let value = self.scalar(value);
        self.emit(Inst::Store {
          address: field,
          value,
        });
      }
    }
    self.terminate(Terminator::Return(Vec::new()));

    let id = self.reserve_trace_init(String::from(TRACE_INIT_SYMBOL));
    let procedure = &mut self.procedures[id.0 as usize];
    procedure.locals = std::mem::take(&mut self.locals);
    procedure.blocks = std::mem::take(&mut self.blocks);
    procedure.value_types = std::mem::take(&mut self.value_types);
    procedure.entry = BlockId(0);
  }
}

/// What a procedure's node hash starts from, different for every procedure of
/// the program (**C§13**).
fn trace_seed(id: ProcId) -> i128 {
  i128::from(id.0) * 0x9e37_79b9 + 0x632b_e59b
}
