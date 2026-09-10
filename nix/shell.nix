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
    # What the vendor tree's graphical programs link against: `invaders`,
    # `treemap`, `skeletal-animation` and `codex_view` name `libGL`, `libX11`,
    # `libasound` and `freetype` with `#library,system`, which the linker has to
    # be able to find.
    pkgs.libGL
    pkgs.libx11
    pkgs.alsa-lib
    pkgs.freetype
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
