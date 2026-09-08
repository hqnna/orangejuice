{ pkgs, llvm, toolchain, rust-analyzer }:

let
  # `llvm-sys` links LLVM, libffi, zlib, libxml2, ncurses and libstdc++
  # dynamically; nothing puts them on a runtime search path, so `oj` is built
  # with an rpath that names them and runs outside the shell as well as in it.
  runtimeLibraries = [
    llvm.llvm.lib
    pkgs.zlib
    pkgs.libffi
    pkgs.libxml2
    pkgs.ncurses
    pkgs.stdenv.cc.cc.lib
  ];
in
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
    RUSTFLAGS = "-C link-arg=-Wl,-rpath,${pkgs.lib.makeLibraryPath runtimeLibraries}";
  };

  shellHook = ''
    export OJ_JAI_DIR="''${OJ_JAI_DIR:-$PWD/vendor/jai}"
  '';
}
