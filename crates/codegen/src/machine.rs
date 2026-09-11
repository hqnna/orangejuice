use inkwell::OptimizationLevel;
use inkwell::targets::{
  CodeModel, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};

/// What the back end is asked to produce.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
  /// The target triple. orangejuice targets Linux x86-64 only
  /// (`docs/spec.md` §2), so this is the default unless a test overrides it.
  pub triple: String,
  pub cpu: String,
  pub features: String,
  /// `Llvm_Options.machine_code_optimization_setting` (**C§4**), `0`…`3`:
  /// what the target machine is built with, which is instruction selection and
  /// scheduling rather than the IR.
  pub optimization: u8,
  /// `Llvm_Options.bitcode_optimization_setting` (**C§4**): the pass pipeline
  /// the module is run through before anything is generated from it.
  pub bitcode: Bitcode,
  /// `Llvm_Options.enable_loop_unrolling`, `enable_slp_vectorization`,
  /// `enable_loop_vectorization` and `merge_functions` (**C§4**), which the
  /// pass builder is told about directly.
  pub passes: PassOptions,
  /// The name the module carries, which shows up in the listings.
  pub module_name: String,
  /// `Build_Options.emit_debug_info` (**C§4**): whether the module carries a
  /// DWARF description of the program.
  pub debug_info: bool,
  /// Whether the module is an executable's or a compile-time run's.
  pub purpose: crate::Purpose,
  /// Which of the program's codegen units this module is, and how many there
  /// are. LLVM's own work — instruction selection, register allocation, the
  /// pass pipeline — is most of a build, and it is per-module, so the program
  /// is split across modules that are built on separate threads and linked
  /// together.
  ///
  /// A count of one is the whole program in one module, which is what a
  /// compile-time run and every textual output get.
  pub unit_index: usize,
  pub unit_count: usize,
}

impl Options {
  /// Whether this module is the one that defines what there may only be one
  /// of: the program's globals and its entry point.
  pub(crate) fn is_primary_unit(&self) -> bool {
    self.unit_count <= 1 || self.unit_index == 0
  }

  /// Whether a procedure belongs to this module. Round-robin rather than
  /// contiguous, so that a run of large procedures next to one another in the
  /// program does not land in one unit.
  pub(crate) fn owns_procedure(&self, index: usize) -> bool {
    self.unit_count <= 1 || index % self.unit_count == self.unit_index
  }

  /// Whether a procedure a unit does not own may still be called from it,
  /// which is what stops a split program giving anything internal linkage.
  pub(crate) fn is_split(&self) -> bool {
    self.unit_count > 1
  }
}

/// `Llvm_Bitcode_Optimization_Setting` (**C§4**): which of LLVM's own
/// pipelines the module is run through.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Bitcode {
  #[default]
  O0,
  O1,
  O2,
  O3,
  Os,
  Oz,
}

impl Bitcode {
  /// The pass pipeline LLVM parses, or `None` when there is nothing to run —
  /// `default<O0>` still costs a walk over the module for no gain.
  pub fn pipeline(self) -> Option<&'static str> {
    match self {
      Self::O0 => None,
      Self::O1 => Some("default<O1>"),
      Self::O2 => Some("default<O2>"),
      Self::O3 => Some("default<O3>"),
      Self::Os => Some("default<Os>"),
      Self::Oz => Some("default<Oz>"),
    }
  }

  /// The value the distribution's enum gives each member (**C§4**), which is
  /// what a metaprogram writes into `Build_Options`.
  pub fn from_value(value: u32) -> Self {
    match value {
      2 => Self::O1,
      3 => Self::O2,
      4 => Self::O3,
      5 => Self::Os,
      6 => Self::Oz,
      _ => Self::O0,
    }
  }
}

/// What `set_optimization` turns on beside the pipeline itself (**C§4**).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PassOptions {
  pub loop_unrolling: bool,
  pub loop_vectorization: bool,
  pub slp_vectorization: bool,
  pub merge_functions: bool,
}

impl Default for Options {
  fn default() -> Self {
    Self {
      triple: String::from(DEFAULT_TRIPLE),
      cpu: String::from("x86-64"),
      features: String::new(),
      optimization: 0,
      bitcode: Bitcode::O0,
      passes: PassOptions::default(),
      module_name: String::from("oj"),
      debug_info: true,
      purpose: crate::Purpose::Executable,
      unit_index: 0,
      unit_count: 1,
    }
  }
}

pub const DEFAULT_TRIPLE: &str = "x86_64-unknown-linux-gnu";

impl Options {
  fn level(&self) -> OptimizationLevel {
    match self.optimization {
      0 => OptimizationLevel::None,
      1 => OptimizationLevel::Less,
      2 => OptimizationLevel::Default,
      _ => OptimizationLevel::Aggressive,
    }
  }
}

/// The target machine every module is built against: its data layout is what
/// `oj-types` computed the layouts for.
pub fn target_machine(options: &Options) -> Result<TargetMachine, String> {
  Target::initialize_x86(&InitializationConfig::default());
  let triple = TargetTriple::create(&options.triple);
  let target = Target::from_triple(&triple).map_err(|error| error.to_string())?;
  target
    .create_target_machine(
      &triple,
      &options.cpu,
      &options.features,
      options.level(),
      // A position-independent executable is what the reference links
      // (**C§11**).
      RelocMode::PIC,
      CodeModel::Default,
    )
    .ok_or_else(|| format!("could not create a target machine for {}", options.triple))
}
