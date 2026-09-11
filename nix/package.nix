{ pkgs, craneLib, llvm }:

let
  # cleanCargoSource keeps only Rust sources. What it drops and the build needs:
  # rustfmt.toml and clippy.toml, which the fmt and clippy checks read the
  # workspace style rules from; the .jai fixtures and insta snapshots beside the
  # tests; and `modules/` and `docs/examples/` — the distribution `oj` ships
  # and the acceptance suite that compiles against it, without which nothing
  # can be built at all.
  src = pkgs.lib.cleanSourceWith {
    src = ../.;
    name = "orangejuice-source";
    filter = path: type:
      craneLib.filterCargoSources path type
      || builtins.match ".*/(rustfmt|clippy)\\.toml$" path != null
      || builtins.match ".*/tests/.*\\.(jai|snap)$" path != null
      || builtins.match ".*/modules(/.*)?$" path != null
      || builtins.match ".*/docs$" path != null
      || builtins.match ".*/docs/examples(/.*)?$" path != null;
  };

  # `llvm-sys` links LLVM, libffi, zlib, libxml2, ncurses and libstdc++
  # dynamically. A binary nix installs gets an rpath from the fixup phase, but
  # a test binary is run out of the build directory before that, so the rpath
  # has to be linked in — the same one `nix/shell.nix` puts on the dev shell.
  runtimeLibraries = [
    llvm.llvm.lib
    pkgs.zlib
    pkgs.libffi
    pkgs.libxml2
    pkgs.ncurses
    pkgs.stdenv.cc.cc.lib
  ];

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
    RUSTFLAGS = "-C link-arg=-Wl,-rpath,${pkgs.lib.makeLibraryPath runtimeLibraries}";
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
craneLib.buildPackage (commonArgs // {
  inherit cargoArtifacts;
  doCheck = true;

  # `oj` finds its modules from its own binary, by walking up from it until a
  # `modules/Preload.jai` turns up — so an installed compiler needs the
  # distribution installed beside it. Without this there is no Preload and no
  # program compiles.
  postInstall = ''
    cp -r ${src}/modules $out/modules
    chmod -R u+w $out/modules
  '';

  passthru = { inherit commonArgs cargoArtifacts; };

  meta = {
    description = "A cleanroom implementation of the Jai programming language";
    license = pkgs.lib.licenses.mit;
    mainProgram = "oj";
    maintainers = [ "Hanna Rose <me@hanna.lol>" ];
    platforms = [ "x86_64-linux" ];
  };
})
