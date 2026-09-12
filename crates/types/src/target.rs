//! What a compilation emits and links code for (`docs/spec.md` §2.1).
//!
//! A target is an operating system and a processor, and everything that is not
//! the same on all of them is asked of one of these: the LLVM triple and cpu
//! name `oj-codegen` builds a target machine from, the `#c_call` convention
//! `oj-types`' `abi` module classifies with, the link line `oj-link` builds,
//! and the `OS`/`CPU`/`IS_CROSS_COMPILING` constants the checker injects
//! (**L§17**).
//!
//! The numbers are `Operating_System_Tag`'s and `CPU_Tag`'s, because a
//! metaprogram sets `Build_Options.os_target`/`cpu_target` with them (**C§4**)
//! and the checker gives `OS` and `CPU` the members they name.

/// The operating system half of a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Os {
  Linux,
  Macos,
}

/// The processor half of a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cpu {
  X64,
  Arm64,
}

impl Os {
  /// The `Operating_System_Tag` value (**L§17**).
  pub fn tag(self) -> u32 {
    match self {
      Self::Linux => 3,
      Self::Macos => 6,
    }
  }

  pub fn from_tag(tag: u32) -> Option<Self> {
    match tag {
      3 => Some(Self::Linux),
      6 => Some(Self::Macos),
      _ => None,
    }
  }

  /// The `Operating_System_Tag` member `OS` is given.
  pub fn member(self) -> &'static [u8] {
    match self {
      Self::Linux => b"LINUX",
      Self::Macos => b"MACOS",
    }
  }

  /// How a diagnostic names it.
  pub fn name(self) -> &'static str {
    match self {
      Self::Linux => "Linux",
      Self::Macos => "MacOS",
    }
  }
}

impl Cpu {
  /// The `CPU_Tag` value (**L§17**).
  pub fn tag(self) -> u32 {
    match self {
      Self::X64 => 3,
      Self::Arm64 => 4,
    }
  }

  pub fn from_tag(tag: u32) -> Option<Self> {
    match tag {
      3 => Some(Self::X64),
      4 => Some(Self::Arm64),
      _ => None,
    }
  }

  /// The `CPU_Tag` member `CPU` is given.
  pub fn member(self) -> &'static [u8] {
    match self {
      Self::X64 => b"X64",
      Self::Arm64 => b"ARM64",
    }
  }

  /// How a diagnostic names it.
  pub fn name(self) -> &'static str {
    match self {
      Self::X64 => "x64",
      Self::Arm64 => "arm64",
    }
  }
}

/// A machine code is produced for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Target {
  pub os: Os,
  pub cpu: Cpu,
}

impl Target {
  pub const LINUX_X64: Self = Self {
    os: Os::Linux,
    cpu: Cpu::X64,
  };
  pub const LINUX_ARM64: Self = Self {
    os: Os::Linux,
    cpu: Cpu::Arm64,
  };
  pub const MACOS_ARM64: Self = Self {
    os: Os::Macos,
    cpu: Cpu::Arm64,
  };

  /// The machine the compiler is running on, which is what a build targets
  /// unless a metaprogram says otherwise (**C§4**).
  pub const HOST: Self = {
    let os = if cfg!(target_os = "macos") {
      Os::Macos
    } else {
      Os::Linux
    };
    let cpu = if cfg!(target_arch = "aarch64") {
      Cpu::Arm64
    } else {
      Cpu::X64
    };
    Self { os, cpu }
  };

  pub fn new(os: Os, cpu: Cpu) -> Self {
    Self { os, cpu }
  }

  /// The LLVM triple a target machine is built from.
  pub fn triple(self) -> &'static str {
    match (self.os, self.cpu) {
      (Os::Linux, Cpu::X64) => "x86_64-unknown-linux-gnu",
      (Os::Linux, Cpu::Arm64) => "aarch64-unknown-linux-gnu",
      (Os::Macos, Cpu::X64) => "x86_64-apple-macosx",
      (Os::Macos, Cpu::Arm64) => "arm64-apple-macosx",
    }
  }

  /// The triple in the spelling the Rust ecosystem uses, which is what the
  /// `cc` crate parses to find a compiler. It is not always LLVM's: Darwin is
  /// `aarch64-apple-darwin` here and `arm64-apple-macosx` there, and handing
  /// `cc` the LLVM spelling makes it panic rather than return an error.
  pub fn cc_triple(self) -> &'static str {
    match (self.os, self.cpu) {
      (Os::Linux, Cpu::X64) => "x86_64-unknown-linux-gnu",
      (Os::Linux, Cpu::Arm64) => "aarch64-unknown-linux-gnu",
      (Os::Macos, Cpu::X64) => "x86_64-apple-darwin",
      (Os::Macos, Cpu::Arm64) => "aarch64-apple-darwin",
    }
  }

  /// The LLVM cpu name, which is the baseline of the architecture rather than
  /// whatever the host happens to be: an executable is not built for this
  /// machine alone.
  pub fn cpu_name(self) -> &'static str {
    match self.cpu {
      Cpu::X64 => "x86-64",
      // Every arm64 Apple silicon machine is at least this, and it is what a
      // Darwin toolchain builds for by default.
      Cpu::Arm64 if self.os == Os::Macos => "apple-m1",
      Cpu::Arm64 => "generic",
    }
  }

  /// Whether the target is x86-64, which is the one architecture the language
  /// has an `#asm` for (**L§15**).
  pub fn is_x64(self) -> bool {
    self.cpu == Cpu::X64
  }

  /// Whether the `#c_call` convention is AAPCS64 rather than System V, which
  /// is also what says an aggregate is coerced whole rather than split into
  /// eightbytes (**L§7.11**).
  pub fn is_aarch64(self) -> bool {
    self.cpu == Cpu::Arm64
  }

  /// Whether the platform makes widening a narrow integer argument the
  /// caller's job, so that the callee may read the whole register. Darwin's
  /// AArch64 ABI does; System V and AAPCS64 proper leave the high bits
  /// unspecified.
  pub fn extends_narrow_arguments(self) -> bool {
    self.os == Os::Macos
  }

  /// Whether the object format is Mach-O, which is what the link line and the
  /// symbol mangling turn on.
  pub fn is_darwin(self) -> bool {
    self.os == Os::Macos
  }

  /// Whether a target is not the machine the compiler runs on, which is what
  /// `IS_CROSS_COMPILING` says (**L§17**) — and what says a `#run` cannot
  /// execute, since the JIT runs in this process.
  pub fn is_cross_compiling(self) -> bool {
    self != Self::HOST
  }
}

impl Default for Target {
  fn default() -> Self {
    Self::HOST
  }
}

impl std::fmt::Display for Target {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(formatter, "{} {}", self.os.name(), self.cpu.name())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_tag_round_trips_through_the_preload_numbering() {
    for os in [Os::Linux, Os::Macos] {
      assert_eq!(Os::from_tag(os.tag()), Some(os));
    }
    for cpu in [Cpu::X64, Cpu::Arm64] {
      assert_eq!(Cpu::from_tag(cpu.tag()), Some(cpu));
    }
  }

  #[test]
  fn a_tag_the_compiler_produces_no_output_for_is_not_a_target() {
    assert_eq!(Os::from_tag(2), None);
    assert_eq!(Cpu::from_tag(11), None);
  }

  #[test]
  fn the_host_is_not_cross_compiling_and_everything_else_may_be() {
    assert!(!Target::HOST.is_cross_compiling());
    assert_eq!(Target::default(), Target::HOST);
  }

  #[test]
  fn the_cc_triple_is_the_rust_spelling_rather_than_llvms() {
    // The `cc` crate panics on a triple it cannot parse, and Darwin is where
    // the two spellings differ.
    assert_eq!(Target::MACOS_ARM64.triple(), "arm64-apple-macosx");
    assert_eq!(Target::MACOS_ARM64.cc_triple(), "aarch64-apple-darwin");
    for target in [Target::LINUX_X64, Target::LINUX_ARM64] {
      assert_eq!(target.triple(), target.cc_triple(), "{target}");
    }
  }

  #[test]
  fn each_target_names_a_triple_of_its_own() {
    let triples = [
      Target::LINUX_X64.triple(),
      Target::LINUX_ARM64.triple(),
      Target::MACOS_ARM64.triple(),
    ];
    for (index, triple) in triples.iter().enumerate() {
      assert!(!triples[..index].contains(triple), "{triple} repeats");
    }
    assert!(Target::MACOS_ARM64.is_darwin());
    assert!(!Target::LINUX_ARM64.is_darwin());
  }
}
