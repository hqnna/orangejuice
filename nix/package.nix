{ pkgs, craneLib, llvm }:

let
  # One source of truth: the workspace manifest.
  version = (pkgs.lib.importTOML ../Cargo.toml).workspace.package.version;

  # cleanCargoSource keeps only Rust sources. What it drops and the build needs:
  # rustfmt.toml and clippy.toml, which the fmt and clippy checks read the
  # workspace style rules from; the .jai fixtures and insta snapshots beside the
  # tests; LICENSE and README.md, which the portable tarball carries; and
  # `modules/` and `docs/examples/` — the distribution `oj` ships
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
      || builtins.match ".*/(LICENSE|README\\.md)$" path != null
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
    inherit version;
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

  oj = craneLib.buildPackage (commonArgs // {
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

    passthru = { inherit commonArgs cargoArtifacts portable; };

    meta = {
      description = "A cleanroom implementation of the Jai programming language";
      license = pkgs.lib.licenses.mit;
      mainProgram = "oj";
      maintainers = [ "Hanna Rose <me@hanna.lol>" ];
      platforms = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];
    };
  });

  # What a release artifact is called: the version and the platform it runs on.
  platform =
    if pkgs.stdenv.hostPlatform.isDarwin
    then "${pkgs.stdenv.hostPlatform.qemuArch}-macos"
    else "${pkgs.stdenv.hostPlatform.qemuArch}-linux";

  # The release artifact: one directory that runs on any machine of its
  # platform.
  #
  # A fully static build is not an option here. Compile-time execution resolves
  # a `#foreign` procedure through LLVM's process search generator, which is
  # `dlsym(RTLD_DEFAULT, ...)`; in a static musl binary that returns null for
  # every symbol, so `malloc` would be unreachable and no `#run` could execute
  # — `Default_Metaprogram`, which drives an ordinary build, is itself a `#run`.
  #
  # The libraries travel with the compiler instead. How they are found back
  # differs: see the two recipes below.
  portable =
    if pkgs.stdenv.hostPlatform.isDarwin then portableDarwin else portableLinux;

  # On Linux the loader travels too. `bin/oj` is a wrapper that runs the
  # bundled `ld.so` against the bundled `lib/`, which is what frees the tree
  # from the host's glibc: it works on a musl system too, because it brings its
  # own.
  portableLinux = pkgs.runCommand "orangejuice-portable-${version}"
    {
      nativeBuildInputs = [ pkgs.patchelf pkgs.zstd pkgs.glibc.bin ];
      meta = oj.meta // {
        description = "${oj.meta.description} (portable tarball)";
      };
    }
    ''
      root=orangejuice-${version}-${platform}
      mkdir -p "$root/bin" "$root/lib" "$root/modules"

      cp ${oj}/bin/oj "$root/bin/oj.real"
      cp -r ${oj}/modules/. "$root/modules/"
      cp ${src}/LICENSE "$root/LICENSE"
      cp ${src}/README.md "$root/README.md"
      chmod -R u+w "$root"

      # Everything the binary resolves, and the interpreter that resolves it.
      for lib in $(ldd "$root/bin/oj.real" | awk '{ print $3 }' | grep '^/'); do
        cp -Ln "$lib" "$root/lib/"
      done
      interpreter=$(patchelf --print-interpreter "$root/bin/oj.real")
      cp -Ln "$interpreter" "$root/lib/"
      chmod -R u+w "$root/lib"
      patchelf --set-rpath '$ORIGIN/../lib' "$root/bin/oj.real"

      # The kernel does not expand $ORIGIN in an ELF interpreter path, so the
      # loader is named by the wrapper rather than in the header.
      cat > "$root/bin/oj" <<WRAPPER
      #!/bin/sh
      # Only shell builtins: a tarball unpacked on a broken PATH still runs.
      here=\''${0%/*}
      [ "\$here" = "\$0" ] && here=.
      root=\$(cd -- "\$here/.." && pwd) || exit 1
      : "\''${OJ_MODULES:=\$root/modules}"
      export OJ_MODULES
      exec "\$root/lib/$(basename "$interpreter")" --library-path "\$root/lib" "\$here/oj.real" "\$@"
      WRAPPER
      sed -i 's/^      //' "$root/bin/oj"
      chmod +x "$root/bin/oj"

      mkdir -p $out
      tar --sort=name --owner=0 --group=0 --numeric-owner --mtime=@1 \
        -cf - "$root" | zstd -19 -T0 -o "$out/$root.tar.zst"
      ln -s "$root.tar.zst" "$out/orangejuice-portable.tar.zst"
    '';

  # On Darwin there is no loader to bundle: the system ABI is stable and
  # `libSystem` and `libc++` are always there. What is not always there is the
  # handful of libraries nix built, which a Mach-O names by absolute store
  # path — so those are copied in and every reference to them rewritten to
  # `@loader_path`, which the loader resolves against the file doing the
  # referring rather than against the process.
  #
  # Editing a Mach-O invalidates its signature, and arm64 macOS will not run an
  # unsigned one at all, so everything touched is re-signed ad-hoc afterwards.
  portableDarwin = pkgs.runCommand "orangejuice-portable-${version}"
    {
      nativeBuildInputs = [ pkgs.darwin.cctools pkgs.darwin.sigtool pkgs.zstd ];
      meta = oj.meta // {
        description = "${oj.meta.description} (portable tarball)";
      };
    }
    ''
      root=orangejuice-${version}-${platform}
      mkdir -p "$root/bin" "$root/lib" "$root/modules"

      cp ${oj}/bin/oj "$root/bin/oj"
      cp -r ${oj}/modules/. "$root/modules/"
      cp ${src}/LICENSE "$root/LICENSE"
      cp ${src}/README.md "$root/README.md"
      chmod -R u+w "$root"

      # Everything the binary names by store path, and everything those name in
      # turn. `/usr/lib` is the system's and stays where it is.
      collect() {
        otool -L "$1" | tail -n +2 | awk '{ print $1 }' | grep '^/nix/store/' || true
      }

      pending=$(collect "$root/bin/oj")
      while [ -n "$pending" ]; do
        next=""
        for lib in $pending; do
          name=$(basename "$lib")
          [ -e "$root/lib/$name" ] && continue
          cp -L "$lib" "$root/lib/$name"
          chmod u+w "$root/lib/$name"
          next="$next $(collect "$lib")"
        done
        pending="$next"
      done

      # Each bundled library answers to its own name beside its referrer, and
      # so does every reference between them.
      for file in "$root/lib/"*.dylib; do
        install_name_tool -id "@loader_path/$(basename "$file")" "$file"
        for lib in $(otool -L "$file" | tail -n +2 | awk '{ print $1 }' | grep '^/nix/store/'); do
          install_name_tool -change "$lib" "@loader_path/$(basename "$lib")" "$file"
        done
        codesign -f -s - "$file"
      done

      for lib in $(otool -L "$root/bin/oj" | tail -n +2 | awk '{ print $1 }' | grep '^/nix/store/'); do
        install_name_tool -change "$lib" "@loader_path/../lib/$(basename "$lib")" "$root/bin/oj"
      done
      codesign -f -s - "$root/bin/oj"

      mkdir -p $out
      tar --sort=name --owner=0 --group=0 --numeric-owner --mtime=@1 \
        -cf - "$root" | zstd -19 -T0 -o "$out/$root.tar.zst"
      ln -s "$root.tar.zst" "$out/orangejuice-portable.tar.zst"
    '';
in
oj
