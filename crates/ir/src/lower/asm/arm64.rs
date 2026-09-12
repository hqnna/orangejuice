//! The AArch64 half of `#asm` (**L§15.9**).
//!
//! The reference compiler's `#asm` is x86-64 and nothing else, so none of this
//! is compatibility: it is orangejuice's own, and `docs/language.md` §15.9 is
//! where it is specified. What it keeps from the x86-64 side is everything
//! that is not the instruction set — a block declares its registers into the
//! scope around it, the allocator here places them, and a variable of the
//! program stays the back end's to place.
//!
//! What it does not keep is the shape of an instruction. x86-64 is two-operand
//! and destructive, where `add a, b` means `a += b`; AArch64 is three-operand,
//! where `add d, n, m` means `d = n + m`. A block written for one does not
//! mean anything on the other, which is why a program that wants both writes
//! `#if CPU == .X64`.
//!
//! Where a NEON mnemonic collides with a scalar one — `add` is both — the Jai
//! spelling takes a `v`, and the assembler still sees the real mnemonic:
//! `vadd` is written here and `add` is emitted, with vector registers.

use super::{AsmClass, Form, Ops, form, reads, sized};

/// `x0`–`x30`. `x31` is the stack pointer or the zero register depending on
/// the instruction and is never handed out; `x18` is the platform register,
/// reserved on Darwin; `x16` and `x17` are the linker's veneer scratch, and it
/// may overwrite them across any call. `x29` and `x30` are the frame pointer
/// and the link register, which belong to the procedure around the block.
const GPR_POOL: [u32; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15];

/// `v0`–`v31`, less `v8`–`v15`, whose low halves a callee must preserve.
const VEC_POOL: [u32; 24] = [
  0, 1, 2, 3, 4, 5, 6, 7, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
];

pub(super) fn pool(class: AsmClass) -> &'static [u32] {
  match class {
    AsmClass::Vec => &VEC_POOL,
    _ => &GPR_POOL,
  }
}

/// What LLVM is told to put a value in. AArch64 names a general-purpose
/// register `x<n>` and a vector one `v<n>` in a constraint, whatever width the
/// instruction then uses it at.
pub(super) fn constraint_name(class: AsmClass, index: Option<u32>) -> String {
  let index = index.unwrap_or(0);
  match class {
    AsmClass::Vec => format!("{{v{index}}}"),
    _ => format!("{{x{index}}}"),
  }
}

/// How a register is written in the text. A general-purpose register is `w<n>`
/// at 32 bits and below and `x<n>` above; a vector register takes the letter
/// of the width it is being used at, and the arrangement form — `v0.4s` — is
/// what a lane-wise instruction asks for.
pub(super) fn register_name(class: AsmClass, index: Option<u32>, bits: u32) -> String {
  let index = index.unwrap_or(0);
  match class {
    AsmClass::Vec => match bits {
      8 => format!("b{index}"),
      16 => format!("h{index}"),
      32 => format!("s{index}"),
      64 => format!("d{index}"),
      _ => format!("q{index}"),
    },
    _ => match bits {
      0..=32 => format!("w{index}"),
      _ => format!("x{index}"),
    },
  }
}

/// A vector register written as lanes rather than as a whole: `v3.4s` is the
/// 128 bits of `v3` read as four 32-bit lanes.
pub(super) fn lane_name(index: Option<u32>, element_bits: u32, total_bits: u32) -> String {
  let index = index.unwrap_or(0);
  let lanes = (total_bits / element_bits.max(1)).max(1);
  let letter = match element_bits {
    8 => 'b',
    16 => 'h',
    64 => 'd',
    _ => 's',
  };
  format!("v{index}.{lanes}{letter}")
}

/// `#asm` lets a block pin a declaration to a named register (**L§15**). On
/// AArch64 the name is the register's own, since they have no other.
pub(super) fn pinned_register(name: &str) -> Option<u32> {
  let digits = name.strip_prefix('x').or_else(|| name.strip_prefix('v'))?;
  let index: u32 = digits.parse().ok()?;
  (index <= 31).then_some(index)
}

/// AArch64 scales an index by shifting it, so only a power of two is an
/// addressing mode at all.
pub(super) fn scale_shift(scale: u64) -> Option<u32> {
  match scale {
    1 => None,
    2 => Some(1),
    4 => Some(2),
    8 => Some(3),
    16 => Some(4),
    _ => None,
  }
}

pub(super) fn form_of(written: &str) -> Option<(&'static Form, bool)> {
  FORMS
    .iter()
    .find(|(text, _)| *text == written)
    .map(|(_, form)| (form, false))
}

/// An instruction that writes no register: a comparison, a store, a barrier.
const fn nothing(text: &'static str, class: AsmClass) -> Form {
  reads(text, Ops::All, class)
}

/// An instruction whose condition is spelled into the mnemonic, because a
/// condition is not a name a program could have declared.
const fn conditional(text: &'static str, condition: &'static str) -> Form {
  Form {
    suffix: condition,
    ..form(text, Ops::All, AsmClass::Gpr)
  }
}

/// A NEON instruction, naming which of its operands are read as lanes.
const fn lanewise(text: &'static str, lanes: &'static [usize]) -> Form {
  Form {
    lanes,
    ..form(text, Ops::All, AsmClass::Vec)
  }
}

/// The three-operand NEON shape, every operand an arrangement.
const THREE: &[usize] = &[0, 1, 2];
/// The two-operand one.
const TWO: &[usize] = &[0, 1];

static FORMS: &[(&str, Form)] = &[
  // ---------------------------------------------------------------- moves ---
  ("mov", form("mov", Ops::All, AsmClass::Gpr)),
  ("movz", form("movz", Ops::All, AsmClass::Gpr)),
  ("movk", form("movk", Ops::All, AsmClass::Gpr)),
  ("movn", form("movn", Ops::All, AsmClass::Gpr)),
  ("mvn", form("mvn", Ops::All, AsmClass::Gpr)),
  // ----------------------------------------------------------- arithmetic ---
  ("add", form("add", Ops::All, AsmClass::Gpr)),
  ("adds", form("adds", Ops::All, AsmClass::Gpr)),
  ("adc", form("adc", Ops::All, AsmClass::Gpr)),
  ("sub", form("sub", Ops::All, AsmClass::Gpr)),
  ("subs", form("subs", Ops::All, AsmClass::Gpr)),
  ("sbc", form("sbc", Ops::All, AsmClass::Gpr)),
  ("neg", form("neg", Ops::All, AsmClass::Gpr)),
  ("mul", form("mul", Ops::All, AsmClass::Gpr)),
  ("madd", form("madd", Ops::All, AsmClass::Gpr)),
  ("msub", form("msub", Ops::All, AsmClass::Gpr)),
  ("smulh", form("smulh", Ops::All, AsmClass::Gpr)),
  ("umulh", form("umulh", Ops::All, AsmClass::Gpr)),
  ("sdiv", form("sdiv", Ops::All, AsmClass::Gpr)),
  ("udiv", form("udiv", Ops::All, AsmClass::Gpr)),
  // -------------------------------------------------------------- logical ---
  ("and", form("and", Ops::All, AsmClass::Gpr)),
  ("ands", form("ands", Ops::All, AsmClass::Gpr)),
  ("orr", form("orr", Ops::All, AsmClass::Gpr)),
  ("orn", form("orn", Ops::All, AsmClass::Gpr)),
  ("eor", form("eor", Ops::All, AsmClass::Gpr)),
  ("eon", form("eon", Ops::All, AsmClass::Gpr)),
  ("bic", form("bic", Ops::All, AsmClass::Gpr)),
  // --------------------------------------------------------------- shifts ---
  ("lsl", form("lsl", Ops::All, AsmClass::Gpr)),
  ("lsr", form("lsr", Ops::All, AsmClass::Gpr)),
  ("asr", form("asr", Ops::All, AsmClass::Gpr)),
  ("ror", form("ror", Ops::All, AsmClass::Gpr)),
  // ------------------------------------------------------------- bit work ---
  ("clz", form("clz", Ops::All, AsmClass::Gpr)),
  ("rbit", form("rbit", Ops::All, AsmClass::Gpr)),
  ("rev", form("rev", Ops::All, AsmClass::Gpr)),
  ("rev16", form("rev16", Ops::All, AsmClass::Gpr)),
  ("ubfx", form("ubfx", Ops::All, AsmClass::Gpr)),
  ("sbfx", form("sbfx", Ops::All, AsmClass::Gpr)),
  ("extr", form("extr", Ops::All, AsmClass::Gpr)),
  // ------------------------------------------------------------ extending ---
  ("sxtb", form("sxtb", Ops::All, AsmClass::Gpr)),
  ("sxth", form("sxth", Ops::All, AsmClass::Gpr)),
  ("sxtw", form("sxtw", Ops::All, AsmClass::Gpr)),
  ("uxtb", form("uxtb", Ops::All, AsmClass::Gpr)),
  ("uxth", form("uxth", Ops::All, AsmClass::Gpr)),
  // ------------------------------------------------------------ compares ---
  ("cmp", nothing("cmp", AsmClass::Gpr)),
  ("cmn", nothing("cmn", AsmClass::Gpr)),
  ("tst", nothing("tst", AsmClass::Gpr)),
  // A condition is written into the mnemonic rather than passed as an
  // operand: `cset_eq x` is `cset x, eq`, because `eq` is not a name a program
  // could have declared. These are the conditions the table carries.
  ("cset_eq", conditional("cset", "eq")),
  ("cset_ne", conditional("cset", "ne")),
  ("cset_lt", conditional("cset", "lt")),
  ("cset_le", conditional("cset", "le")),
  ("cset_gt", conditional("cset", "gt")),
  ("cset_ge", conditional("cset", "ge")),
  ("cset_lo", conditional("cset", "lo")),
  ("cset_ls", conditional("cset", "ls")),
  ("cset_hi", conditional("cset", "hi")),
  ("cset_hs", conditional("cset", "hs")),
  ("cset_cs", conditional("cset", "cs")),
  ("cset_cc", conditional("cset", "cc")),
  ("cset_mi", conditional("cset", "mi")),
  ("cset_pl", conditional("cset", "pl")),
  ("csel_eq", conditional("csel", "eq")),
  ("csel_ne", conditional("csel", "ne")),
  ("csel_lt", conditional("csel", "lt")),
  ("csel_le", conditional("csel", "le")),
  ("csel_gt", conditional("csel", "gt")),
  ("csel_ge", conditional("csel", "ge")),
  ("csel_lo", conditional("csel", "lo")),
  ("csel_hi", conditional("csel", "hi")),
  ("csel_cs", conditional("csel", "cs")),
  ("csel_cc", conditional("csel", "cc")),
  // ---------------------------------------------------------------- memory ---
  ("ldr", form("ldr", Ops::All, AsmClass::Gpr)),
  ("ldrb", sized("ldrb", Ops::All, AsmClass::Gpr, 32)),
  ("ldrh", sized("ldrh", Ops::All, AsmClass::Gpr, 32)),
  ("ldrsb", sized("ldrsb", Ops::All, AsmClass::Gpr, 32)),
  ("ldrsh", sized("ldrsh", Ops::All, AsmClass::Gpr, 32)),
  ("ldrsw", form("ldrsw", Ops::All, AsmClass::Gpr)),
  ("str", nothing("str", AsmClass::Gpr)),
  ("strb", reads("strb", Ops::All, AsmClass::Gpr)),
  ("strh", reads("strh", Ops::All, AsmClass::Gpr)),
  (
    "ldp",
    Form {
      writes: &[0, 1],
      ..form("ldp", Ops::All, AsmClass::Gpr)
    },
  ),
  ("stp", nothing("stp", AsmClass::Gpr)),
  // A load-exclusive writes its destination; a store-exclusive writes the
  // status register it is given first.
  ("ldxr", form("ldxr", Ops::All, AsmClass::Gpr)),
  ("ldaxr", form("ldaxr", Ops::All, AsmClass::Gpr)),
  ("stxr", form("stxr", Ops::All, AsmClass::Gpr)),
  ("stlxr", form("stlxr", Ops::All, AsmClass::Gpr)),
  // ------------------------------------------------------------- barriers ---
  ("dmb", nothing("dmb", AsmClass::Gpr)),
  ("dsb", nothing("dsb", AsmClass::Gpr)),
  ("isb", nothing("isb", AsmClass::Gpr)),
  ("nop", reads("nop", Ops::None, AsmClass::Gpr)),
  ("brk", nothing("brk", AsmClass::Gpr)),
  ("yield", reads("yield", Ops::None, AsmClass::Gpr)),
  // ------------------------------------------------------- scalar floating ---
  ("fmov", form("fmov", Ops::All, AsmClass::Vec)),
  ("fadd", form("fadd", Ops::All, AsmClass::Vec)),
  ("fsub", form("fsub", Ops::All, AsmClass::Vec)),
  ("fmul", form("fmul", Ops::All, AsmClass::Vec)),
  ("fdiv", form("fdiv", Ops::All, AsmClass::Vec)),
  ("fneg", form("fneg", Ops::All, AsmClass::Vec)),
  ("fabs", form("fabs", Ops::All, AsmClass::Vec)),
  ("fsqrt", form("fsqrt", Ops::All, AsmClass::Vec)),
  ("fmadd", form("fmadd", Ops::All, AsmClass::Vec)),
  ("fcmp", nothing("fcmp", AsmClass::Vec)),
  ("fcvt", form("fcvt", Ops::All, AsmClass::Vec)),
  // A conversion crosses the register files, so its destination is not its
  // own class.
  (
    "scvtf",
    Form {
      destination_class: Some(AsmClass::Vec),
      ..form("scvtf", Ops::All, AsmClass::Gpr)
    },
  ),
  (
    "ucvtf",
    Form {
      destination_class: Some(AsmClass::Vec),
      ..form("ucvtf", Ops::All, AsmClass::Gpr)
    },
  ),
  (
    "fcvtzs",
    Form {
      destination_class: Some(AsmClass::Gpr),
      ..form("fcvtzs", Ops::All, AsmClass::Vec)
    },
  ),
  (
    "fcvtzu",
    Form {
      destination_class: Some(AsmClass::Gpr),
      ..form("fcvtzu", Ops::All, AsmClass::Vec)
    },
  ),
  // ------------------------------------------------------------ neon lanes ---
  //
  // Written with a `v` where the mnemonic would collide with the scalar one;
  // what reaches the assembler is the real name either way.
  ("vadd", lanewise("add", THREE)),
  ("vsub", lanewise("sub", THREE)),
  ("vmul", lanewise("mul", THREE)),
  ("vand", lanewise("and", THREE)),
  ("vorr", lanewise("orr", THREE)),
  ("veor", lanewise("eor", THREE)),
  ("vmvn", lanewise("mvn", TWO)),
  ("vneg", lanewise("neg", TWO)),
  ("vabs", lanewise("abs", TWO)),
  ("vshl", lanewise("shl", THREE)),
  ("vushr", lanewise("ushr", THREE)),
  ("vsshr", lanewise("sshr", THREE)),
  ("vfadd", lanewise("fadd", THREE)),
  ("vfsub", lanewise("fsub", THREE)),
  ("vfmul", lanewise("fmul", THREE)),
  ("vfdiv", lanewise("fdiv", THREE)),
  (
    "vdup",
    Form {
      destination_class: Some(AsmClass::Vec),
      ..lanewise("dup", &[0])
    },
  ),
  ("vaddv", lanewise("addv", &[1])),
  ("vcmeq", lanewise("cmeq", THREE)),
];
