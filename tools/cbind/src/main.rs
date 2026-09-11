use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

fn main() {
  let args: Vec<String> = std::env::args().collect();
  let ast = std::fs::read_to_string(&args[1]).expect("ast file");
  let mut wanted = String::new();
  std::io::stdin().read_to_string(&mut wanted).unwrap();
  let wanted: BTreeSet<&str> = wanted.lines().map(str::trim).filter(|l| !l.is_empty()).collect();

  let mut out: BTreeMap<String, String> = BTreeMap::new();
  for line in ast.lines() {
    let line = line.trim();
    if !line.starts_with("extern ") || !line.ends_with(';') {
      continue;
    }
    let decl = strip_attributes(&line[7..line.len() - 1]);
    let Some((name, rendered)) = function(&decl) else { continue };
    if !wanted.contains(name.as_str()) {
      continue;
    }
    out.entry(name).or_insert(rendered);
  }

  for (_, rendered) in &out {
    println!("{rendered}");
  }
  eprintln!("emitted {} of {} wanted", out.len(), wanted.len());
  for name in &wanted {
    if !out.contains_key(*name) {
      eprintln!("  missed {name}");
    }
  }
}


include!("common.rs");
