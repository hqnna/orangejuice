use std::process::ExitCode;

fn main() -> ExitCode {
  oj_cli::run(std::env::args_os())
}
