{
  description = "Development and build environment for kellnr";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane = {
      url = "github:ipetkov/crane";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, flake-utils, crane, rust-overlay, ... }:
    flake-utils.lib.eachSystem [ "aarch64-darwin" "aarch64-linux" "x86_64-linux" ] (system:
      let
        pkgs = import nixpkgs { inherit system; overlays = [ (import rust-overlay) ]; };
        inherit (pkgs) lib;

        craneLib = (crane.mkLib pkgs).overrideToolchain (p: p.rust-bin.stable.latest.default.override { });

        # Set a filter of files that are included in the build source directory.
        # This is used to filter out files that are not needed for the build to
        # not rebuild on every file change, e.g. in a Readme.md file.
        webuiFilter = path: _type:
          let extensions = [ "js" "json" "ts" "vue" "html" "png" "css" "svg" ];
          in lib.any (ext: lib.hasSuffix ".${ext}" path) extensions;
        webuiOrCargo = path: type:
          (webuiFilter path type) || (craneLib.filterCargoSources path type);
        # Include all Rust and WebUI files in the source directory.
        src = lib.cleanSourceWith {
          src = craneLib.path ./.;
          filter = webuiOrCargo;
        };

        commonArgs = {
          inherit src;
          strictDeps = true;
          pname = "kellnr";

          nativeBuildInputs = [
            pkgs.cmake
            pkgs.nodejs_24
          ] ++ lib.optionals pkgs.stdenv.isLinux [
            pkgs.pkg-config
            pkgs.rustPlatform.bindgenHook
          ];

          buildInputs = [
            pkgs.cargo-nextest
            pkgs.openssl.dev
          ] ++ lib.optional pkgs.stdenv.isDarwin [
            pkgs.libiconv
            pkgs.iconv
            pkgs.cacert
            pkgs.curl
          ] ++ lib.optional pkgs.stdenv.isLinux [
          ];

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${pkgs.llvmPackages.libclang.lib}/lib/clang/${pkgs.lib.getVersion pkgs.clang}/include";

          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";

        };

        # Build *just* the cargo dependencies, so we can reuse
        # all of that work (e.g. via cachix) when running in CI
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Install the NPM dependencies
        node2nixOutput = import ui/nix { inherit pkgs system; nodejs = pkgs.nodejs_24; };
        nodeDeps = node2nixOutput.nodeDependencies;

        # Build the actual crate itself, reusing the dependency
        # artifacts from above.
        kellnr-crate = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts nodeDeps;

          # Skip test as we run them with cargo-nextest
          doCheck = false;

          preConfigurePhases = [
            "npmBuild"
          ];

          npmBuild = ''
            cd ui;
            ln -s ${nodeDeps}/lib/node_modules ./node_modules;
            export PATH="${nodeDeps}/bin:$PATH";
            npm run build --verbose;
            cd ..;
          '';

          installPhase =
            let
              binDir = "$out/bin";
              configDir = "${binDir}/config";
              staticDir = "${binDir}/static";
            in
            ''
              # Copy kellnr binary into bin directory
              mkdir -p ${binDir};
              cp target/release/kellnr ${binDir};
     
              # Copy default config
              mkdir -p ${configDir};
              cp config/default.toml ${configDir};
     
              # Copy the built UI
              mkdir -p ${staticDir};
              cp -r ui/dist/* ${staticDir};
            '';
        });
      in
      with pkgs;
      {
        checks = {
          inherit kellnr-crate;

          # Check formatting with rustfmt.
          fmt = craneLib.cargoFmt (commonArgs // {
            inherit src;
          });

          # Check for clippy warnings.
          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--workspace --all-targets -- --deny warnings";
          });
        };

        devShells.default = craneLib.devShell (commonArgs // {
          inputsFrom = [ kellnr-crate ];

          shellHook = ''
            alias c=cargo
            alias j=just
            alias lg=lazygit
          '' + lib.optionalString stdenv.isDarwin ''
            export DYLD_LIBRARY_PATH="$(rustc --print sysroot)/lib:$DYLD_LIBRARY_PATH"
            export RUST_SRC_PATH="$(rustc --print sysroot)/lib/rustlib/src/rust/src"
          '';

          packages = with pkgs; [
            rust-analyzer
            cargo-nextest
            cargo-machete
            lazygit
            just
            node2nix
            jd-diff-patch
            sea-orm-cli
          ];
        });

        packages = {
          default = kellnr-crate;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = kellnr-crate;
        };
      }
    );
}

