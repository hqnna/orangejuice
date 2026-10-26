fn main() {
  println!("cargo:rerun-if-changed=csrc/guard.c");
  cc::Build::new()
    .file("csrc/guard.c")
    .opt_level(2)
    .compile("oj_guard");
}
