// Emits a C program that prints each wanted macro as a Jai constant.
//
// A `#define` is text, and its value is whatever the *compiler* makes of it —
// `ACCESSPERMS` is `(S_IRWXU|S_IRWXG|S_IRWXO)` and `O_CREAT` is `0100`. So
// nothing here parses a value: the program this writes is compiled against the
// same headers and prints what each one came to, which is the only way to be
// right about an octal constant or an expression over three other macros.
//
// Function-like macros are skipped — there is no value to print — and so is
// anything the compiler then refuses, which `generate.sh` feeds back in.

use std::collections::BTreeSet;
use std::io::Read;

fn main() {
  let args: Vec<String> = std::env::args().collect();
  let macros = std::fs::read_to_string(&args[1]).expect("macros file");
  let headers = &args[2];
  // Names the caller already knows the compiler rejects, one per line.
  let rejected: BTreeSet<String> = match args.get(3) {
    Some(path) => std::fs::read_to_string(path)
      .unwrap_or_default()
      .lines()
      .map(|line| line.trim().to_string())
      .filter(|line| !line.is_empty())
      .collect(),
    None => BTreeSet::new(),
  };

  let mut wanted = String::new();
  std::io::stdin().read_to_string(&mut wanted).unwrap();
  let wanted: BTreeSet<&str> = wanted
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .collect();

  // An object-like macro is `NAME value`; a function-like one is `NAME(args)`,
  // where the parenthesis follows the name with nothing between.
  let mut defined: BTreeSet<&str> = BTreeSet::new();
  for line in macros.lines() {
    let (name, value) = match line.find(|c: char| c == ' ' || c == '(') {
      Some(at) if line.as_bytes()[at] == b' ' => (&line[..at], &line[at + 1..]),
      Some(_) => continue,
      // `#define NAME` with no value at all is not a constant either.
      None => continue,
    };
    // A macro whose value is a string is not a number, and casting it to one
    // yields whatever address the literal landed at — which is how `P_tmpdir`
    // came out as 93824992259364. A pointer built out of an integer is fine:
    // `MAP_FAILED` is `((void *) -1)` and -1 is what the module wants.
    if value.contains('"') {
      continue;
    }
    if wanted.contains(name) && !rejected.contains(name) {
      defined.insert(name);
    }
  }

  println!("#include \"{headers}\"");
  println!("#include <stdio.h>");
  println!("int main(void) {{");
  // One per line and in order, so that a compiler error's line number is the
  // macro that caused it, which is what `generate.sh` feeds back in.
  for name in &defined {
    println!("  printf(\"{name} :: %lld;\\n\", (long long)({name})); /* {name} */");
  }
  println!("  return 0;");
  println!("}}");
  eprintln!("{} of {} wanted names are object-like macros", defined.len(), wanted.len());
}
