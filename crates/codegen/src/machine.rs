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
  /// `0`…`3`, mapped from `Build_Options.llvm_options` (**C§4**).
  pub optimization: u8,
  /// The name the module carries, which shows up in the listings.
  pub module_name: String,
  /// Whether the module is an executable's or a compile-time run's.
  pub purpose: crate::Purpose,
}

impl Default for Options {
  fn default() -> Self {
    Self {
      triple: String::from(DEFAULT_TRIPLE),
      cpu: String::from("x86-64"),
      features: String::new(),
      optimization: 0,
      module_name: String::from("oj"),
      purpose: crate::Purpose::Executable,
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
