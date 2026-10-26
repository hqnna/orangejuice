// Emits Jai enums from clang's `-ast-print`.
//
// An anonymous C enum is named after the common prefix of its members, and
// that prefix comes off each member — `DT_DIR` becomes `DT.DIR`. A tagged one
// keeps its members' full names, since the tag says nothing about them. Two
// anonymous enums that share a prefix are told apart by a `_1`, `_2` suffix.

use std::collections::{BTreeSet, HashMap};
use std::io::Read;

fn main() {
  let args: Vec<String> = std::env::args().collect();
  let ast = std::fs::read_to_string(&args[1]).expect("ast file");
  let aliases = args.get(2).is_some_and(|style| style == "aliases");
  let mut wanted = String::new();
  std::io::stdin().read_to_string(&mut wanted).unwrap();
  let wanted: BTreeSet<String> = wanted.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();

  let lines: Vec<&str> = ast.lines().collect();
  let mut seen: HashMap<String, usize> = HashMap::new();
  let mut emitted: BTreeSet<String> = BTreeSet::new();

  let mut at = 0;
  while at < lines.len() {
    let line = lines[at].trim_end();
    let tagged = line.starts_with("enum ") && line.ends_with('{');
    let typedefed = line.starts_with("typedef enum ") && line.ends_with('{');
    if !tagged && !typedefed {
      at += 1;
      continue;
    }

    let mut end = at + 1;
    while end < lines.len() && !lines[end].trim_start().starts_with('}') {
      end += 1;
    }
    if end >= lines.len() {
      break;
    }

    // `NAME = VALUE` or just `NAME`, one per line or comma-separated.
    let body: String = lines[at + 1..end].join(" ");
    let members: Vec<(String, Option<String>)> = body
      .split(',')
      .map(str::trim)
      .filter(|p| !p.is_empty())
      .map(|p| match p.split_once('=') {
        Some((n, v)) => (n.trim().to_string(), Some(v.trim().to_string())),
        None => (p.to_string(), None),
      })
      .filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_'))
      .collect();
    if members.is_empty() {
      at = end + 1;
      continue;
    }

    let head = if tagged { line["enum ".len()..line.len() - 1].trim() } else { line["typedef enum ".len()..line.len() - 1].trim() };
    let tail = lines[end].trim().trim_start_matches('}').trim().trim_end_matches(';').trim();

    let prefix = common_prefix(&members);
    // A tagged enum is known by its tag; an anonymous one by its prefix.
    let mut candidates: Vec<String> = Vec::new();
    if !head.is_empty() {
      candidates.push(head.to_string());
      candidates.push(head.trim_start_matches('_').to_uppercase());
    }
    if !tail.is_empty() {
      candidates.push(tail.to_string());
    }
    if !prefix.is_empty() {
      candidates.push(prefix.clone());
      // `_SC_*` names an enum called `SC`, not `_SC`.
      candidates.push(prefix.trim_start_matches('_').to_string());
      candidates.push(format!("{}_definitions", prefix.trim_start_matches('_')));
      candidates.push(format!("{prefix}_definitions"));
      // The reference suffixes some groups to say what they are, and spells
      // io_uring `IO_URING` where the header says `IORING`.
      let spaced = prefix.replacen("IORING", "IO_URING", 1);
      for base in [prefix.as_str(), spaced.as_str()] {
        for suffix in ["_Flag_Bits", "_Bits", "_Categories", "_OP", "_Flags"] {
          candidates.push(format!("{base}{suffix}"));
        }
        candidates.push(base.to_string());
      }
    }

    let free = |c: &String| wanted.contains(c) && !emitted.contains(c);
    let Some(name) = candidates.iter().find(|c| free(c)).cloned().or_else(|| {
      // A second anonymous enum sharing a prefix takes the `_1` spelling.
      let base = prefix.trim_start_matches('_').to_string();
      (1..4).flat_map(|n| [format!("{prefix}_{n}"), format!("{base}_{n}")]).find(|c| free(c))
    }) else {
      at = end + 1;
      continue;
    };
    if !emitted.insert(name.clone()) {
      at = end + 1;
      continue;
    }
    *seen.entry(prefix.clone()).or_insert(0) += 1;

    // The prefix only comes off when it is what named the enum.
    // The prefix only comes off when it is what named the enum, and only for
    // an anonymous one: a tagged enum is known by its tag, which says nothing
    // about its members, so they keep the names C gave them.
    let from_prefix = name == prefix
      || (name.starts_with(&format!("{prefix}_")) && name[prefix.len() + 1..].chars().all(|c| c.is_ascii_digit()));
    let strip = head.is_empty() && from_prefix;
    // In the alias style a named enum loses its prefix too, and the C spelling
    // stays reachable inside it — unless a member would be left starting with
    // a digit, which no Jai name can.
    let alias = aliases
      && !strip
      && !prefix.is_empty()
      && members.iter().all(|(member, _)| {
        member.len() > prefix.len() + 1 && !member.as_bytes()[prefix.len() + 1].is_ascii_digit()
      });
    let strip = strip || alias;

    println!("{name} :: enum s32 {{");
    for (member, value) in &members {
      let shown = if strip && member.len() > prefix.len() + 1 { &member[prefix.len() + 1..] } else { member.as_str() };
      match value {
        Some(v) => println!("    {shown} :: {};", rewrite(v, &prefix, strip)),
        None => println!("    {shown};"),
      }
    }
    if alias {
      println!();
      for (member, _) in &members {
        println!("    {member} :: {};", &member[prefix.len() + 1..]);
      }
    }
    println!("}}");
    // The C tag is what other declarations name the type by, so it stays
    // reachable as an alias when the enum took a different name.
    if !head.is_empty() && head != name {
      println!("{head} :: {name};");
    }
    if !tail.is_empty() && tail != name && tail != head {
      println!("{tail} :: {name};");
    }
    println!();
    at = end + 1;
  }

  eprintln!("emitted {} of {}", emitted.len(), wanted.len());
  for name in &wanted {
    if !emitted.contains(name) {
      eprintln!("  missed {name}");
    }
  }
}

/// A value that names a sibling has to name it by its new spelling.
fn rewrite(value: &str, prefix: &str, strip: bool) -> String {
  // C integer suffixes — `1U << 0`, `4096UL` — mean nothing in Jai.
  let stripped: String = {
    let mut out = String::new();
    let chars: Vec<char> = value.chars().collect();
    let mut i = 0;
    while i < chars.len() {
      out.push(chars[i]);
      if chars[i].is_ascii_digit() {
        let mut j = i + 1;
        while j < chars.len() && chars[j].is_ascii_digit() { out.push(chars[j]); j += 1; }
        while j < chars.len() && matches!(chars[j], 'u' | 'U' | 'l' | 'L') { j += 1; }
        i = j;
        continue;
      }
      i += 1;
    }
    out
  };
  let uncast = c_casts_removed(&stripped);
  let value = uncast.as_str();
  let trimmed = value.trim();
  if strip && trimmed.starts_with(prefix) && trimmed.len() > prefix.len() + 1 {
    return trimmed[prefix.len() + 1..].to_string();
  }
  trimmed.to_string()
}

/// The longest `_`-separated prefix every member shares.
fn common_prefix(members: &[(String, Option<String>)]) -> String {
  let first: Vec<&str> = members[0].0.split('_').collect();
  let mut take = first.len().saturating_sub(1);
  while take > 0 {
    let candidate = first[..take].join("_");
    if members.iter().all(|(m, _)| m.starts_with(&format!("{candidate}_"))) {
      return candidate;
    }
    take -= 1;
  }
  String::new()
}

/// Takes off the C casts a value is spelled with and turns a character
/// literal into its code, which is what a four-character tag such as
/// FreeType's `FT_IMAGE_TAG` expands to: `((unsigned long)(unsigned char)('c')
/// << 24) | ...` reads as `(((99) << 24) | ...` and folds the same.
fn c_casts_removed(value: &str) -> String {
  let chars: Vec<char> = value.chars().collect();
  let mut out = String::new();
  let mut i = 0;
  while i < chars.len() {
    if chars[i] == '\'' && i + 2 < chars.len() && chars[i + 2] == '\'' {
      out.push_str(&(chars[i + 1] as u32).to_string());
      i += 3;
      continue;
    }
    if chars[i] == '(' {
      if let Some(close) = chars[i + 1..].iter().position(|&c| c == ')') {
        let inner: String = chars[i + 1..i + 1 + close].iter().collect();
        let after = chars.get(i + close + 2).copied();
        let words: Vec<&str> = inner.split_whitespace().collect();
        let is_type = !words.is_empty()
          && words.iter().all(|w| w.chars().all(|c| c.is_alphanumeric() || c == '_') && !w.chars().next().unwrap().is_ascii_digit())
          && (words.len() > 1 || matches!(words[0], "char" | "short" | "int" | "long" | "unsigned" | "signed") || words[0].ends_with("_t") || words[0].starts_with("FT_"));
        if is_type && matches!(after, Some('(') | Some('\'')) {
          i += close + 2;
          continue;
        }
      }
    }
    out.push(chars[i]);
    i += 1;
  }
  out
}
