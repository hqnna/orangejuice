use std::process::ExitCode;

/// The compiler allocates in small pieces and in great numbers — a scope's
/// names, a lookup's answer, every node of every tree — and glibc's allocator
/// was a tenth of a build's instructions on its own.
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
  oj_cli::run(std::env::args_os())
}
