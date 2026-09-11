// Turns clang's `-ast-print` output into Jai `#foreign` declarations.
//
// Only the names listed on stdin are emitted, so the output covers exactly the
// surface a module is meant to export. Types are mapped the way the reference's
// own generator maps them, which is what keeps the two bindings interchangeable.


fn unused_main() {
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

/// Drops every `__attribute__((...))`, matching nested parentheses.
fn strip_attributes(decl: &str) -> String {
  let decl = &strip_asm(decl);
  let mut result = String::new();
  let bytes: Vec<char> = decl.chars().collect();
  let mut at = 0;
  while at < bytes.len() {
    if bytes[at..].starts_with(&['_', '_', 'a', 't', 't', 'r']) && decl[char_index(&bytes, at)..].starts_with("__attribute__") {
      let mut depth = 0;
      let mut scan = at;
      while scan < bytes.len() {
        if bytes[scan] == '(' {
          depth += 1;
        } else if bytes[scan] == ')' {
          depth -= 1;
          if depth == 0 {
            scan += 1;
            break;
          }
        }
        scan += 1;
      }
      at = scan;
      continue;
    }
    result.push(bytes[at]);
    at += 1;
  }
  result.trim().to_string()
}

fn char_index(chars: &[char], at: usize) -> usize {
  chars[..at].iter().map(|c| c.len_utf8()).sum()
}

/// `RET name(params)` -> the Jai declaration, or None when the shape is one we
/// do not translate (a function returning a function pointer, say).
fn function(decl: &str) -> Option<(String, String)> {
  let open = decl.find('(')?;
  if !decl.ends_with(')') {
    return None;
  }
  let head = decl[..open].trim();
  // A function pointer return type puts a `*` before the name; those are rare
  // enough to hand-write.
  if head.ends_with('*') && head.contains('(') {
    return None;
  }
  let params = &decl[open + 1..decl.len() - 1];

  let name_start = head.rfind(|c: char| !(c.is_alphanumeric() || c == '_'))? + 1;
  let name = head[name_start..].to_string();
  if name.is_empty() {
    return None;
  }
  let return_type = head[..name_start].trim();

  let mut rendered = Vec::new();
  for (index, part) in split_top_level(params).iter().enumerate() {
    let part = part.trim();
    if part.is_empty() || part == "void" {
      continue;
    }
    if part == "..." {
      rendered.push("__args: ..Any".to_string());
      continue;
    }
    let (parameter_name, jai) = parameter(part, index)?;
    rendered.push(format!("{parameter_name}: {jai}"));
  }

  let jai_return = jai_type(return_type)?;
  let arrow = format!(" -> {jai_return}");
  Some((
    name.clone(),
    format!("{name} :: ({}){arrow} #foreign libc;", rendered.join(", ")),
  ))
}

/// Splits a parameter list on commas that are not inside parentheses or
/// brackets, so a function-pointer parameter stays in one piece.
fn split_top_level(params: &str) -> Vec<String> {
  let mut parts = Vec::new();
  let mut depth = 0;
  let mut current = String::new();
  for c in params.chars() {
    match c {
      '(' | '[' => {
        depth += 1;
        current.push(c);
      }
      ')' | ']' => {
        depth -= 1;
        current.push(c);
      }
      ',' if depth == 0 => {
        parts.push(std::mem::take(&mut current));
      }
      _ => current.push(c),
    }
  }
  if !current.trim().is_empty() {
    parts.push(current);
  }
  parts
}

/// A C type to its Jai spelling. `const`, `restrict` and the struct/union/enum
/// keywords carry no information here and come off.
fn jai_type(text: &str) -> Option<String> {
  let mut text = text.trim().to_string();
  for noise in ["const ", "restrict ", "__restrict ", "volatile ", "struct ", "union ", "enum "] {
    while let Some(at) = text.find(noise) {
      text.replace_range(at..at + noise.len(), "");
    }
  }
  let text = text.replace("const", " ").replace("restrict", " ");
  let text = text.trim();

  let stars = text.chars().filter(|c| *c == '*').count();
  let base = text.replace('*', "");
  let base = base.split_whitespace().collect::<Vec<_>>().join(" ");

  let mapped = match base.as_str() {
    "void" => {
      if stars == 0 {
        "void".to_string()
      } else {
        "void".to_string()
      }
    }
    "char" | "signed char" => "u8".to_string(),
    "unsigned char" => "u8".to_string(),
    "short" | "short int" | "signed short" => "s16".to_string(),
    "unsigned short" | "unsigned short int" => "u16".to_string(),
    "int" | "signed" | "signed int" => "s32".to_string(),
    "unsigned" | "unsigned int" => "u32".to_string(),
    "long" | "long int" | "signed long" | "long long" | "long long int" => "s64".to_string(),
    "unsigned long" | "unsigned long int" | "unsigned long long" | "unsigned long long int" => "u64".to_string(),
    "float" => "float32".to_string(),
    "double" | "long double" => "float64".to_string(),
    "_Bool" => "bool".to_string(),
    "" => return None,
    other => other.to_string(),
  };
  Some(format!("{}{}", "*".repeat(stars), mapped))
}
/// One parameter: its name (invented when the header gave none) and its type.
fn parameter(part: &str, index: usize) -> Option<(String, String)> {
  let cleaned = part.trim();

  // A function-pointer parameter: `RET (*name)(args)`.
  if let Some(star) = cleaned.find("(*") {
    let close = cleaned[star..].find(')')? + star;
    let name = cleaned[star + 2..close].trim().to_string();
    let ret = cleaned[..star].trim();
    let inner = cleaned[close + 1..].trim();
    let inner = inner.strip_prefix('(')?.strip_suffix(')')?;

    let mut args = Vec::new();
    for (i, a) in split_top_level(inner).iter().enumerate() {
      let a = a.trim();
      if a.is_empty() || a == "void" {
        continue;
      }
      if a == "..." {
        args.push("..Any".to_string());
        continue;
      }
      let (_, jai) = parameter(a, i)?;
      args.push(jai);
    }
    let ret = jai_type(ret)?;
    let name = if name.is_empty() { format!("__arg{index}") } else { name };
    return Some((name, format!("#type ({}) -> {ret} #c_call", args.join(", "))));
  }

  // An array parameter decays to a pointer: `T name[n]` is `*T`.
  if let Some(bracket) = cleaned.find('[') {
    let head = cleaned[..bracket].trim();
    let (name, type_text) = split_name(head, index);
    let jai = jai_type(&format!("{type_text} *"))?;
    return Some((name, jai));
  }

  let (name, type_text) = split_name(cleaned, index);
  Some((name, jai_type(&type_text)?))
}

/// Splits `int __fd` into ("__fd", "int"). A declaration with no name — `int`,
/// `unsigned long`, `struct aiocb *` — keeps all of it as the type and gets a
/// made-up name, since Jai has no unnamed parameters.
fn split_name(text: &str, index: usize) -> (String, String) {
  let text = text.trim();
  let unnamed = || (format!("__arg{index}"), text.to_string());

  // A trailing `*` or `)` means the declarator ended without a name.
  let Some(end) = text.rfind(|c: char| c.is_alphanumeric() || c == '_') else {
    return unnamed();
  };
  if end + 1 != text.len() {
    return unnamed();
  }

  let start = text[..=end].rfind(|c: char| !(c.is_alphanumeric() || c == '_')).map_or(0, |i| i + 1);
  let candidate = &text[start..];
  if is_type_word(candidate) {
    return unnamed();
  }

  let rest = text[..start].trim();
  // Nothing before it, so the token was the type rather than a name.
  if rest.is_empty() || rest.chars().all(|c| c == '*') {
    return unnamed();
  }
  (candidate.to_string(), rest.to_string())
}

fn is_type_word(word: &str) -> bool {
  matches!(
    word,
    "void" | "char" | "short" | "int" | "long" | "float" | "double" | "signed" | "unsigned" | "const" | "struct" | "union" | "enum" | "restrict" | "_Bool"
  )
}

/// `asm("symbol")` renames the symbol a declaration binds to; it carries no
/// type information, so it comes off with the attributes.
fn strip_asm(decl: &str) -> String {
  let Some(at) = decl.find(" asm(") else { return decl.to_string() };
  let Some(close) = decl[at..].find(')') else { return decl.to_string() };
  format!("{}{}", &decl[..at], &decl[at + close + 1..])
}
