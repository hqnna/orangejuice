{ pkgs, llvm, toolchain, rust-analyzer }:

pkgs.mkShell {
  name = "orangejuice";

  packages = [
    toolchain
    rust-analyzer
    llvm.llvm
    llvm.lld
    llvm.clang
    pkgs.pkg-config
    pkgs.zlib
    pkgs.libffi
    pkgs.libxml2
    pkgs.ncurses
    pkgs.gdb
    pkgs.valgrind
  ];

  env = {
    LLVM_SYS_191_PREFIX = "${llvm.llvm.dev}";
    RUST_BACKTRACE = "1";
  };

  shellHook = ''
    export OJ_JAI_DIR="''${OJ_JAI_DIR:-$PWD/vendor/jai}"
  '';
}
