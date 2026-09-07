{ pkgs, craneLib, llvm }:

let
  # cleanCargoSource keeps only Rust sources, dropping rustfmt.toml and
  # clippy.toml, which the fmt and clippy checks need in order to see the
  # workspace style rules, and the .jai fixtures and insta snapshots the tests
  # read.
  src = pkgs.lib.cleanSourceWith {
    src = ../.;
    name = "orangejuice-source";
    filter = path: type:
      craneLib.filterCargoSources path type
      || builtins.match ".*/(rustfmt|clippy)\\.toml$" path != null
      || builtins.match ".*/tests/.*\\.(jai|snap)$" path != null;
  };

  commonArgs = {
    inherit src;
    pname = "orangejuice";
    version = "0.1.0";
    strictDeps = true;

    nativeBuildInputs = [ pkgs.pkg-config ];

    buildInputs = [
      llvm.llvm
      pkgs.zlib
      pkgs.libffi
      pkgs.libxml2
      pkgs.ncurses
    ];

    LLVM_SYS_191_PREFIX = "${llvm.llvm.dev}";
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
craneLib.buildPackage (commonArgs // {
  inherit cargoArtifacts;
  doCheck = true;

  passthru = { inherit commonArgs cargoArtifacts; };

  meta = {
    description = "A cleanroom implementation of the Jai programming language";
    license = pkgs.lib.licenses.mit;
    mainProgram = "oj";
    platforms = [ "x86_64-linux" ];
  };
})
