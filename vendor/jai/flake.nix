{
  description = "The vendored Jai reference compiler (beta 0.2.009), wrapped for NixOS";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { nixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};

      # jai-linux reads /etc/ld.so.conf and falls back to /lib, /usr/lib and
      # /usr/lib64 for `#library,system` and for the -L flags it hands to lld;
      # it honours neither LD_LIBRARY_PATH nor a patched rpath, so the whole
      # distribution runs inside an FHS sandbox instead.
      targetPkgs = p: [
        p.gcc
        p.glibc
        p.glibc.dev
        p.stdenv.cc.cc.lib
        p.zlib
        p.alsa-lib
        p.libGL
        p.vulkan-loader
        p.libx11
      ];

      # The distribution is git-ignored, so this flake cannot carry it: locate
      # it the way oj-testsupport does, from OJ_JAI_DIR or from the nearest
      # enclosing checkout.
      locate = ''
        distribution=''${OJ_JAI_DIR:-}
        if [ -z "$distribution" ]; then
          directory=$PWD
          while [ ! -d "$directory/vendor/jai/modules" ]; do
            if [ "$directory" = / ]; then
              echo "no jai distribution found. Set OJ_JAI_DIR to a directory containing modules/, or unpack the beta 0.2.009 distribution into vendor/jai." >&2
              exit 1
            fi
            directory=$(dirname "$directory")
          done
          distribution=$directory/vendor/jai
        fi
      '';

      compiler = pkgs.buildFHSEnv {
        name = "jai";
        inherit targetPkgs;
        runScript = pkgs.writeShellScript "jai-run" ''
          set -euo pipefail
          ${locate}
          exec "$distribution/bin/jai-linux" "$@"
        '';
      };

      # Programs jai builds are linked against /lib64/ld-linux-x86-64.so.2 and
      # the sandbox's libraries, so they run under this shell.
      shell = pkgs.buildFHSEnv {
        name = "jai-shell";
        inherit targetPkgs;
        runScript = pkgs.writeShellScript "jai-shell-run" ''
          set -euo pipefail
          ${locate}
          export OJ_JAI_DIR=$distribution
          export PATH=$distribution/bin:$PATH
          exec ''${SHELL:-bash} "$@"
        '';
      };
    in
    {
      packages.${system} = {
        default = compiler;
        jai = compiler;
        jai-shell = shell;
      };

      apps.${system} = {
        default = {
          type = "app";
          program = "${compiler}/bin/jai";
          meta.description = "The vendored Jai reference compiler";
        };
        shell = {
          type = "app";
          program = "${shell}/bin/jai-shell";
          meta.description = "A shell where jai-linux and the programs it builds run";
        };
      };
    };
}
