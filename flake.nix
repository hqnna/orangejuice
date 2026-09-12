{
  description = "orangejuice: a cleanroom implementation of the Jai programming language";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];

      perSystem = { pkgs, system, lib, ... }:
        let
          fenix = inputs.fenix.packages.${system};
          toolchain = fenix.complete.withComponents [
            "cargo"
            "clippy"
            "rust-src"
            "rustc"
            "rustfmt"
          ];
          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain toolchain;
          llvm = pkgs.llvmPackages_19;
          oj = import ./nix/package.nix { inherit pkgs craneLib llvm; };
          inherit (oj.passthru) commonArgs cargoArtifacts;

          # Every host builds `oj`, but only a host that is also a target can
          # run what it builds: the package and the checks compile and execute
          # real programs, and compile-time execution JITs target code into the
          # compiler's own process (`docs/spec.md` §2.1). Elsewhere the dev
          # shell is the whole output, and the suite is run from it knowing
          # which five test binaries do not pass yet.
          isTarget = system == "x86_64-linux";
        in
        {
          devShells.default = import ./nix/shell.nix {
            inherit pkgs llvm toolchain;
            rust-analyzer = fenix.rust-analyzer;
          };

          packages = lib.optionalAttrs isTarget {
            default = oj;
            oj = oj;
            # The release artifact: `oj`, its loader and libraries, and the
            # modules it ships, as one relocatable tree.
            portable = oj.passthru.portable;
          };

          apps = lib.optionalAttrs isTarget {
            default = {
              type = "app";
              program = lib.getExe oj;
            };
          };

          checks = lib.optionalAttrs isTarget {
            package = oj;

            clippy = craneLib.cargoClippy (commonArgs // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- -D warnings";
            });

            fmt = craneLib.cargoFmt {
              inherit (commonArgs) src pname version;
            };

            test = craneLib.cargoTest (commonArgs // { inherit cargoArtifacts; });

            doc = craneLib.cargoDoc (commonArgs // { inherit cargoArtifacts; });
          };
        };
    };
}
