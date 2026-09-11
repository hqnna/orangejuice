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
