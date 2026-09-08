//! A thin, owning wrapper over LLVM's ORC v2 `LLJIT`.
//!
//! inkwell has no ORC bindings, so this is the one place in orangejuice that
//! talks to `llvm-sys` directly. Everything above it sees safe Rust: build a
//! module in [`Orc::context`], hand it to [`Orc::add_module`], and look up a
//! symbol with [`Orc::lookup`].

use std::ffi::{CStr, CString};

use inkwell::context::ContextRef;
use inkwell::module::Module;
use llvm_sys::error::{
  LLVMConsumeError, LLVMDisposeErrorMessage, LLVMErrorRef, LLVMGetErrorMessage,
};
use llvm_sys::orc2::lljit::{
  LLVMOrcCreateLLJIT, LLVMOrcCreateLLJITBuilder, LLVMOrcDisposeLLJIT, LLVMOrcLLJITAddLLVMIRModule,
  LLVMOrcLLJITBuilderSetJITTargetMachineBuilder, LLVMOrcLLJITGetGlobalPrefix,
  LLVMOrcLLJITGetMainJITDylib, LLVMOrcLLJITLookup, LLVMOrcLLJITMangleAndIntern, LLVMOrcLLJITRef,
};
use llvm_sys::orc2::{
  LLVMJITEvaluatedSymbol, LLVMJITSymbolFlags, LLVMOrcAbsoluteSymbols, LLVMOrcCSymbolMapPair,
  LLVMOrcCreateDynamicLibrarySearchGeneratorForProcess, LLVMOrcCreateNewThreadSafeContext,
  LLVMOrcCreateNewThreadSafeModule, LLVMOrcDefinitionGeneratorRef, LLVMOrcDisposeThreadSafeContext,
  LLVMOrcJITDylibAddGenerator, LLVMOrcJITDylibDefine, LLVMOrcJITDylibRef,
  LLVMOrcJITTargetMachineBuilderCreateFromTargetMachine, LLVMOrcThreadSafeContextGetContext,
  LLVMOrcThreadSafeContextRef,
};

/// An `LLJIT` instance with one dylib, into which every compile-time module of
/// a compilation is added.
pub struct Orc {
  jit: LLVMOrcLLJITRef,
  dylib: LLVMOrcJITDylibRef,
  thread_safe_context: LLVMOrcThreadSafeContextRef,
}

impl Orc {
  /// Builds a JIT for the given target machine, which it takes ownership of.
  pub fn new(machine: inkwell::targets::TargetMachine) -> Result<Self, String> {
    unsafe {
      let builder = LLVMOrcCreateLLJITBuilder();
      // `CreateFromTargetMachine` consumes the machine, so inkwell must not
      // dispose it as well.
      let raw = machine.as_mut_ptr();
      std::mem::forget(machine);
      let target_machine_builder = LLVMOrcJITTargetMachineBuilderCreateFromTargetMachine(raw);
      LLVMOrcLLJITBuilderSetJITTargetMachineBuilder(builder, target_machine_builder);

      let mut jit: LLVMOrcLLJITRef = std::ptr::null_mut();
      check(LLVMOrcCreateLLJIT(&mut jit, builder))?;
      let dylib = LLVMOrcLLJITGetMainJITDylib(jit);
      let thread_safe_context = LLVMOrcCreateNewThreadSafeContext();

      // Compile-time code calls into the compiler process for `#foreign`
      // procedures whose library is already loaded (`docs/spec.md` §6.5).
      let mut generator: LLVMOrcDefinitionGeneratorRef = std::ptr::null_mut();
      let prefix = LLVMOrcLLJITGetGlobalPrefix(jit);
      check(LLVMOrcCreateDynamicLibrarySearchGeneratorForProcess(
        &mut generator,
        prefix,
        None,
        std::ptr::null_mut(),
      ))?;
      LLVMOrcJITDylibAddGenerator(dylib, generator);

      Ok(Self {
        jit,
        dylib,
        thread_safe_context,
      })
    }
  }

  /// The context every compile-time module is built in. It belongs to the
  /// JIT's thread-safe context, which is what lets a module be handed over
  /// without copying it.
  pub fn context(&self) -> ContextRef<'_> {
    unsafe { ContextRef::new(LLVMOrcThreadSafeContextGetContext(self.thread_safe_context)) }
  }

  /// Adds a module to the JIT, which takes ownership of it.
  pub fn add_module(&self, module: Module<'_>) -> Result<(), String> {
    unsafe {
      let raw = module.as_mut_ptr();
      std::mem::forget(module);
      let thread_safe = LLVMOrcCreateNewThreadSafeModule(raw, self.thread_safe_context);
      check(LLVMOrcLLJITAddLLVMIRModule(
        self.jit,
        self.dylib,
        thread_safe,
      ))
    }
  }

  /// Binds a symbol to an address the compiler owns, which is how a global in
  /// JIT code reaches the data segments of `oj-runtime`.
  pub fn define(&self, symbol: &str, address: u64) -> Result<(), String> {
    let name =
      CString::new(symbol).map_err(|_| format!("the symbol '{symbol}' has a NUL in it"))?;
    unsafe {
      let interned = LLVMOrcLLJITMangleAndIntern(self.jit, name.as_ptr());
      let mut pair = LLVMOrcCSymbolMapPair {
        Name: interned,
        Sym: LLVMJITEvaluatedSymbol {
          Address: address,
          Flags: LLVMJITSymbolFlags {
            GenericFlags:
              llvm_sys::orc2::LLVMJITSymbolGenericFlags::LLVMJITSymbolGenericFlagsExported as u8,
            TargetFlags: 0,
          },
        },
      };
      let unit = LLVMOrcAbsoluteSymbols(&mut pair, 1);
      check(LLVMOrcJITDylibDefine(self.dylib, unit))
    }
  }

  /// The address a symbol materialized to, compiling whatever it needs.
  pub fn lookup(&self, symbol: &str) -> Result<u64, String> {
    let name =
      CString::new(symbol).map_err(|_| format!("the symbol '{symbol}' has a NUL in it"))?;
    let mut address = 0u64;
    unsafe {
      check(LLVMOrcLLJITLookup(self.jit, &mut address, name.as_ptr()))?;
    }
    Ok(address)
  }
}

impl Drop for Orc {
  fn drop(&mut self) {
    unsafe {
      let error = LLVMOrcDisposeLLJIT(self.jit);
      if !error.is_null() {
        LLVMConsumeError(error);
      }
      LLVMOrcDisposeThreadSafeContext(self.thread_safe_context);
    }
  }
}

/// Turns an `LLVMErrorRef` into a `Result`, consuming it either way.
fn check(error: LLVMErrorRef) -> Result<(), String> {
  if error.is_null() {
    return Ok(());
  }
  unsafe {
    let message = LLVMGetErrorMessage(error);
    let text = CStr::from_ptr(message).to_string_lossy().into_owned();
    LLVMDisposeErrorMessage(message);
    Err(text)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use inkwell::AddressSpace;
  use inkwell::module::Linkage;

  fn jit() -> Orc {
    let machine = oj_codegen::target_machine(&oj_codegen::Options::default())
      .expect("a target machine for the host");
    Orc::new(machine).expect("an LLJIT")
  }

  /// A module built in the JIT's context runs, and a symbol the compiler
  /// defined is the storage it reads — which is the whole contract the data
  /// segments rest on.
  #[test]
  fn a_module_reads_a_global_the_compiler_owns() {
    let jit = jit();
    let storage: Box<i64> = Box::new(41);
    let address = &*storage as *const i64 as u64;
    jit
      .define("oj_test_storage", address)
      .expect("a definition");

    let context = jit.context();
    let module = context.create_module("test");
    let i64_type = context.i64_type();
    let global = module.add_global(i64_type, Some(AddressSpace::default()), "oj_test_storage");
    global.set_linkage(Linkage::External);

    let function = module.add_function("oj_test_run", i64_type.fn_type(&[], false), None);
    let builder = context.create_builder();
    builder.position_at_end(context.append_basic_block(function, "entry"));
    let loaded = builder
      .build_load(i64_type, global.as_pointer_value(), "value")
      .expect("a load")
      .into_int_value();
    let sum = builder
      .build_int_add(loaded, i64_type.const_int(1, false), "sum")
      .expect("an add");
    builder.build_return(Some(&sum)).expect("a return");

    jit.add_module(module).expect("the module is added");
    let entry = jit.lookup("oj_test_run").expect("the entry point");
    let entry: extern "C" fn() -> i64 = unsafe { std::mem::transmute(entry as usize) };
    assert_eq!(entry(), 42);
  }

  /// Two modules in one dylib share the globals the compiler defined, which is
  /// what makes two `#run`s see one `x` (**L§12.3**).
  #[test]
  fn two_modules_share_one_definition() {
    let jit = jit();
    let storage: Box<i64> = Box::new(0);
    let address = &*storage as *const i64 as u64;
    jit.define("oj_test_shared", address).expect("a definition");

    for (name, value) in [("oj_test_write", 7i64), ("oj_test_write_again", 9)] {
      let context = jit.context();
      let module = context.create_module(name);
      let i64_type = context.i64_type();
      let global = module.add_global(i64_type, Some(AddressSpace::default()), "oj_test_shared");
      global.set_linkage(Linkage::External);
      let function = module.add_function(name, context.void_type().fn_type(&[], false), None);
      let builder = context.create_builder();
      builder.position_at_end(context.append_basic_block(function, "entry"));
      builder
        .build_store(
          global.as_pointer_value(),
          i64_type.const_int(value as u64, false),
        )
        .expect("a store");
      builder.build_return(None).expect("a return");
      jit.add_module(module).expect("the module is added");
    }

    for (name, expected) in [("oj_test_write", 7i64), ("oj_test_write_again", 9)] {
      let entry = jit.lookup(name).expect("the entry point");
      let entry: extern "C" fn() = unsafe { std::mem::transmute(entry as usize) };
      entry();
      assert_eq!(*storage, expected);
    }
  }
}
