// Emits Jai struct, union and typedef declarations from clang's `-ast-print`.

use std::collections::BTreeSet;
use std::io::Read;

include!("common.rs");

fn main() {
  let args: Vec<String> = std::env::args().collect();
  let ast = std::fs::read_to_string(&args[1]).expect("ast file");
  let mut wanted = String::new();
  std::io::stdin().read_to_string(&mut wanted).unwrap();
  let wanted: BTreeSet<String> = wanted.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();

  let lines: Vec<&str> = ast.lines().collect();
  let mut emitted: BTreeSet<String> = BTreeSet::new();

  let mut at = 0;
  while at < lines.len() {
    // An attribute can sit between `struct` and the name, so it comes off
    // before anything else is read.
    let cleaned_line = strip_attributes(lines[at].trim_end());
    let line = cleaned_line.trim();

    // `typedef ...;` on one line.
    if line.starts_with("typedef ") && line.ends_with(';') {
      // `typedef struct _XOC *XOC, *XFontSet;` declares two names over one
      // base type; each is the one-name typedef of that base.
      let body = strip_attributes(&line[8..line.len() - 1]);
      if body.contains(',') && !body.contains('(') {
        let mut parts = body.split(',').map(str::trim);
        let first = parts.next().unwrap_or("");
        let name_start = first.rfind(|c: char| !(c.is_alphanumeric() || c == '_')).map_or(0, |i| i + 1);
        let base = first[..name_start].trim_end_matches(|c: char| c == '*' || c.is_whitespace());
        for declarator in std::iter::once(first[base.len()..].trim()).chain(parts) {
          if let Some((name, rendered)) = typedef(&format!("{base} {declarator}")) {
            if wanted.contains(&name) && emitted.insert(name) {
              println!("{rendered}");
            }
          }
        }
        at += 1;
        continue;
      }
      if let Some((name, rendered)) = typedef(&body) {
        if wanted.contains(&name) && emitted.insert(name) {
          println!("{rendered}");
        }
      }
      at += 1;
      continue;
    }

    // `struct NAME {` ... `};`
    // `typedef struct NAME {` ... `} ALIAS;` — a named aggregate that also
    // gets a typedef. Both names are emitted when both are wanted.
    if (line.starts_with("typedef struct ") || line.starts_with("typedef union ")) && line.ends_with('{') {
      let keyword = if line.starts_with("typedef struct ") { "struct" } else { "union" };
      let tag = line["typedef ".len() + keyword.len() + 1..line.len() - 1].trim().to_string();
      let mut end = at + 1;
      let mut depth = 1;
      while end < lines.len() && depth > 0 {
        depth += lines[end].matches('{').count() as i32;
        depth -= lines[end].matches('}').count() as i32;
        if depth == 0 {
          break;
        }
        end += 1;
      }
      if end < lines.len() {
        let tail = lines[end].trim().trim_start_matches('}').trim().trim_end_matches(';').trim().to_string();
        let (alias, pointers) = split_aliases(&tail);

        let primary = if wanted.contains(&tag) { Some(tag.clone()) } else if wanted.contains(&alias) { Some(alias.clone()) } else { None };
        if let Some(primary) = primary {
          if emitted.insert(primary.clone()) {
            println!("{primary} :: {keyword} {{");
            emit_members(&lines[at + 1..end], 1);
            println!("}}\n");
          }
          // The other spelling is the same type under another name.
          for other in [&tag, &alias] {
            if *other != primary && !other.is_empty() && wanted.contains(other) && emitted.insert(other.clone()) {
              println!("{other} :: {primary};\n");
            }
          }
          emit_pointer_aliases(&pointers, &primary, &wanted, &mut emitted);
        }
      }
      at = end + 1;
      continue;
    }

    // `struct NAME;` — an opaque type a program only ever holds a pointer to.
    if (line.starts_with("struct ") || line.starts_with("union ")) && line.ends_with(';') && !line.contains('{') {
      let keyword = if line.starts_with("struct ") { "struct" } else { "union" };
      let name = line[keyword.len() + 1..line.len() - 1].trim().to_string();
      if !name.is_empty() && wanted.contains(&name) && emitted.insert(name.clone()) {
        println!("// Opaque: the library owns the contents.");
        println!("{name} :: struct {{}}\n");
      }
      at += 1;
      continue;
    }

    // `typedef struct {` ... `} NAME;` — an anonymous aggregate given a name.
    if line.starts_with("typedef struct {") || line.starts_with("typedef union {") {
      let keyword = if line.contains("struct") { "struct" } else { "union" };
      let mut end = at + 1;
      let mut depth = 1;
      while end < lines.len() && depth > 0 {
        depth += lines[end].matches('{').count() as i32;
        depth -= lines[end].matches('}').count() as i32;
        if depth == 0 {
          break;
        }
        end += 1;
      }
      if end < lines.len() {
        let tail = lines[end].trim();
        let tail = tail.trim_start_matches('}').trim().trim_end_matches(';').trim().to_string();
        let (name, pointers) = split_aliases(&tail);
        if !name.is_empty() && wanted.contains(&name) && emitted.insert(name.clone()) {
          println!("{name} :: {keyword} {{");
          emit_members(&lines[at + 1..end], 1);
          println!("}}\n");
        }
        if !name.is_empty() && emitted.contains(&name) {
          emit_pointer_aliases(&pointers, &name, &wanted, &mut emitted);
        }
      }
      at = end + 1;
      continue;
    }

    let aggregate = line.starts_with("struct ") || line.starts_with("union ");
    if aggregate && line.ends_with('{') {
      let keyword = if line.starts_with("struct ") { "struct" } else { "union" };
      let name = line[keyword.len() + 1..line.len() - 1].trim().to_string();
      let mut end = at + 1;
      let mut depth = 1;
      while end < lines.len() && depth > 0 {
        depth += lines[end].matches('{').count() as i32;
        depth -= lines[end].matches('}').count() as i32;
        if depth == 0 {
          break;
        }
        end += 1;
      }
      if !name.is_empty() && wanted.contains(&name) && emitted.insert(name.clone()) {
        println!("{name} :: {keyword} {{");
        emit_members(&lines[at + 1..end], 1);
        println!("}}\n");
      }
      at = end + 1;
      continue;
    }
    at += 1;
  }

  // A tag the headers only ever name — `typedef struct FT_LibraryRec_
  // *FT_Library;` with the struct private to the library — is opaque too.
  for name in &wanted {
    if emitted.contains(name) {
      continue;
    }
    let named = ast.contains(&format!("struct {name} *"))
      || ast.contains(&format!("struct {name};"))
      || ast.contains(&format!("typedef struct {name} "));
    let defined = ast.contains(&format!("struct {name} {{"));
    if named && !defined && emitted.insert(name.clone()) {
      println!("// Opaque: the library owns the contents.");
      println!("{name} :: struct {{}}\n");
    }
  }

  eprintln!("emitted {} of {}", emitted.len(), wanted.len());
  for name in &wanted {
    if !emitted.contains(name) {
      eprintln!("  missed {name}");
    }
  }
}

/// One struct member, already indented in the source. Nested aggregates are
/// left to a later pass; this handles the flat cases, which is nearly all of
/// them.
fn self_member(line: &str) -> Option<String> {
  let text = strip_attributes(line.trim());
  let text = text.strip_suffix(';')?;
  let text = text.trim();
  if text.is_empty() || text.ends_with('{') || text == "}" {
    return None;
  }
  // A bitfield: Jai has none, so the member is dropped and noted.
  if let Some(colon) = text.rfind(':') {
    if !text[colon + 1..].trim().is_empty() && text[colon + 1..].trim().chars().all(|c| c.is_ascii_digit()) {
      return Some(format!("// bitfield dropped: {text}"));
    }
  }

  // `TYPE name[N]` -> `name: [N] TYPE`
  if let Some(bracket) = text.find('[') {
    let close = text.rfind(']')?;
    let count = text[bracket + 1..close].trim();
    let head = text[..bracket].trim();
    let (name, type_text) = split_name(head, 0);
    let name = member_name(&name);
    let jai = jai_type(&type_text)?;
    // A C flexible array member — `char name[];` — has no count. Jai has no
    // zero-length array, so it becomes a one-element one, the way the
    // reference's own bindings write it; the real length is whatever the
    // struct's own count member says.
    if count.is_empty() {
      return Some(format!("{name}: [1] {jai};"));
    }
    return Some(format!("{name}: [{count}] {jai};"));
  }

  let (name, jai) = parameter(text, 0).ok()?;
  Some(format!("{}: {jai};", member_name(&name)))
}

/// A member may not be named after a primitive type, which `epoll_data`'s
/// `u32` and `u64` are, so those take the leading underscore the reference's
/// own bindings give them.
fn member_name(name: &str) -> String {
  const PRIMITIVES: &[&str] = &[
    "bool", "float", "float32", "float64", "int", "s8", "s16", "s32", "s64", "string", "u8", "u16",
    "u32", "u64", "void",
  ];
  match PRIMITIVES.contains(&name) || is_jai_keyword(name) {
    true => format!("_{name}"),
    false => name.to_string(),
  }
}

trait OkExt<T> {
  fn ok(self) -> Option<T>;
}
impl<T> OkExt<T> for Option<T> {
  fn ok(self) -> Option<T> {
    self
  }
}

/// `typedef` in its several shapes. The last identifier is always the new
/// name, even when it looks like a type — `typedef __off_t off_t;` names
/// `off_t`, and nearly every typedef in glibc ends that way.
fn typedef(body: &str) -> Option<(String, String)> {
  let body = body.trim();

  // `RET (*NAME)(args)`
  if body.contains("(*") {
    let (name, jai) = parameter(body, 0)?;
    return Some((name.clone(), format!("{name} :: {jai};")));
  }

  // `TYPE NAME[n]` — an array typedef, which is how glibc declares jmp_buf.
  if let Some(bracket) = body.find('[') {
    let close = body.rfind(']')?;
    let count = body[bracket + 1..close].trim().to_string();
    let head = body[..bracket].trim();
    let end = head.rfind(|c: char| c.is_alphanumeric() || c == '_')? + 1;
    let start = head[..end].rfind(|c: char| !(c.is_alphanumeric() || c == '_')).map_or(0, |i| i + 1);
    let name = head[start..end].to_string();
    let jai = jai_type(head[..start].trim())?;
    let count = if count.is_empty() { "1".to_string() } else { count };
    return Some((name.clone(), format!("{name} :: [{count}] {jai};")));
  }

  let end = body.rfind(|c: char| c.is_alphanumeric() || c == '_')? + 1;
  if end != body.len() {
    return None;
  }
  let start = body[..end].rfind(|c: char| !(c.is_alphanumeric() || c == '_')).map_or(0, |i| i + 1);
  let name = body[start..end].to_string();
  let rest = body[..start].trim();
  if rest.is_empty() {
    return None;
  }
  let jai = jai_type(rest)?;
  // `typedef struct X X;` names the tag after itself, which in Jai would be a
  // declaration that is its own definition.
  if jai == name {
    return None;
  }
  Some((name.clone(), format!("{name} :: {jai};")))
}
/// Emits a struct's members, descending into the anonymous structs and unions
/// glibc nests inside `siginfo_t` and friends. Each nested aggregate becomes a
/// Jai member whose type is written out in place.
fn emit_members(lines: &[&str], indent: usize) {
  let pad = "    ".repeat(indent);
  let mut at = 0;
  while at < lines.len() {
    let text = strip_attributes(lines[at].trim());
    let text = text.trim();

    if text.is_empty() || text == "}" {
      at += 1;
      continue;
    }

    // A nested aggregate: `struct {` or `union {` ... `} name;`
    if text.ends_with('{') && (text.starts_with("struct") || text.starts_with("union")) {
      let keyword = if text.starts_with("struct") { "struct" } else { "union" };
      let mut end = at + 1;
      let mut depth = 1;
      while end < lines.len() && depth > 0 {
        depth += lines[end].matches('{').count() as i32;
        depth -= lines[end].matches('}').count() as i32;
        if depth == 0 {
          break;
        }
        end += 1;
      }
      if end >= lines.len() {
        return;
      }
      let tail = lines[end].trim().trim_start_matches('}').trim().trim_end_matches(';').trim();
      // An aggregate with no member name is placed inline, the way C does it.
      if tail.is_empty() {
        println!("{pad}using _anonymous_{at}: {keyword} {{");
        emit_members(&lines[at + 1..end], indent + 1);
        println!("{pad}}}");
      } else {
        // `struct { ... } min_aspect, max_aspect;` declares two members of
        // one anonymous type, which Jai spells as two members.
        for name in tail.split(',').map(str::trim).filter(|name| !name.is_empty()) {
          println!("{pad}{}: {keyword} {{", member_name(name));
          emit_members(&lines[at + 1..end], indent + 1);
          println!("{pad}}}");
        }
      }
      at = end + 1;
      continue;
    }

    if let Some(rendered) = self_member(lines[at]) {
      println!("{pad}{rendered}");
    }
    at += 1;
  }
}

/// `} XkbDescRec, *XkbDescPtr;` names an aggregate and a pointer to it: the
/// first plain name, and the names that carry stars with how many.
fn split_aliases(tail: &str) -> (String, Vec<(String, usize)>) {
  let mut plain = String::new();
  let mut pointers = Vec::new();
  for part in tail.split(',').map(str::trim).filter(|part| !part.is_empty()) {
    let stars = part.chars().take_while(|&c| c == '*' || c.is_whitespace()).filter(|&c| c == '*').count();
    let name = part.trim_start_matches(|c: char| c == '*' || c.is_whitespace()).to_string();
    if stars == 0 && plain.is_empty() {
      plain = name;
    } else if stars > 0 {
      pointers.push((name, stars));
    }
  }
  (plain, pointers)
}

fn emit_pointer_aliases(pointers: &[(String, usize)], target: &str, wanted: &BTreeSet<String>, emitted: &mut BTreeSet<String>) {
  for (name, stars) in pointers {
    if wanted.contains(name) && emitted.insert(name.clone()) {
      println!("{name} :: {}{target};\n", "*".repeat(*stars));
    }
  }
}
