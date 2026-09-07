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
      systems = [ "x86_64-linux" ];

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
        in
        {
          packages.default = oj;
          packages.oj = oj;

          apps.default = {
            type = "app";
            program = lib.getExe oj;
          };

          devShells.default = import ./nix/shell.nix {
            inherit pkgs llvm toolchain;
            rust-analyzer = fenix.rust-analyzer;
          };

          checks = {
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
