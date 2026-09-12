{ pkgs, llvm, toolchain, rust-analyzer }:

let
  inherit (pkgs) lib stdenv;

  # `llvm-sys` links LLVM, libffi, zlib, libxml2, ncurses and libstdc++
  # dynamically. On Linux nothing puts them on a runtime search path, so `oj`
  # is built with an rpath that names them and runs outside the shell as well
  # as in it. On Darwin a nix dylib carries its own absolute store path as its
  # install name, so the link records it and no rpath is needed.
  runtimeLibraries = [
    llvm.llvm.lib
    pkgs.zlib
    pkgs.libffi
    pkgs.libxml2
    pkgs.ncurses
    stdenv.cc.cc.lib
  ];

  # What a graphical program links against: a `#library,system "libGL"` and its
  # neighbours have to be findable by the linker `oj` drives. They are the
  # Linux graphics stack; on Darwin the equivalents are system frameworks that
  # come with the SDK rather than packages.
  graphicsLibraries = [
    pkgs.libGL
    pkgs.libx11
    pkgs.alsa-lib
    pkgs.freetype
  ];

  # `gdb` and `valgrind` do not support Darwin on Apple silicon; `lldb`, which
  # LLVM ships, is the debugger there.
  debuggers =
    if stdenv.hostPlatform.isDarwin
    then [ llvm.lldb ]
    else [ pkgs.gdb pkgs.valgrind ];
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
  ]
  ++ debuggers
  ++ lib.optionals stdenv.hostPlatform.isLinux graphicsLibraries;

  env = {
    LLVM_SYS_191_PREFIX = "${llvm.llvm.dev}";
    RUST_BACKTRACE = "1";
  } // lib.optionalAttrs stdenv.hostPlatform.isLinux {
    RUSTFLAGS = "-C link-arg=-Wl,-rpath,${lib.makeLibraryPath runtimeLibraries}";
  };
}
