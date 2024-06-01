{
  description = "Development and build environment for kellnr";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };
  };

  outputs = { self, nixpkgs, flake-utils, crane, fenix, ... }:
    flake-utils.lib.eachSystem [ "aarch64-darwin" "aarch64-linux" "x86_64-linux" ] (system:
      let
        pkgs = import nixpkgs { inherit system; };
        inherit (pkgs) lib;

        craneLib = crane.mkLib nixpkgs.legacyPackages.${system};
        
        webuiFilter = path: _type: builtins.match ".*.(js|json|ts|vue|html|png|css|svg)$" path != null;
        webuiOrCargo = path: type:
          (webuiFilter path type) || (craneLib.filterCargoSources path type);

        # src = lib.cleanSourceWith {
        #   src = craneLib.path ./.;
        #   filter = webuiOrCargo;
        # };
        #
        
        # src = craneLib.cleanCargoSource ./.;

        src = ./.;


        commonArgs = {
          inherit src;
          strictDeps = true;
          pname = "kellnr";

          nativeBuildInputs = [
            pkgs.cmake
            pkgs.nodejs_22
          ] ++ lib.optionals pkgs.stdenv.isLinux [
            pkgs.pkg-config
            pkgs.rustPlatform.bindgenHook
          ];

          buildInputs = [
            pkgs.cargo-nextest
          ] ++ lib.optional pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Cocoa
            pkgs.libiconv
            pkgs.iconv
            pkgs.cacert
            pkgs.curl
          ] ++ lib.optional pkgs.stdenv.isLinux [
            pkgs.openssl.dev
          ];

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${pkgs.llvmPackages.libclang.lib}/lib/clang/${pkgs.lib.getVersion pkgs.clang}/include";

          # Skip test as we run them with cargo-nextest
          doCheck = false;
        };

        craneLibLLvmTools = craneLib.overrideToolchain
          (fenix.packages.${system}.complete.withComponents [
            "cargo"
            "rustc"
            "clippy"
            "rustfmt"
            "rust-analyzer"
          ]);

        # Build *just* the cargo dependencies, so we can reuse
        # all of that work (e.g. via cachix) when running in CI
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Install the NPM dependencies
        nodejs = pkgs.nodejs_22;
        node2nixOutput = import ui/nix { inherit pkgs nodejs system; };
        # nodeDeps = (pkgs.callPackage ./ui/nix/default.nix { }).nodeDependencies;
        nodeDeps = node2nixOutput.nodeDependencies;

        # Build the actual crate itself, reusing the dependency
        # artifacts from above.
        kellnr-crate = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts nodeDeps;

          preConfigurePhases = [
            "npmInstall"
            "debug"
          ];

          debug = ''
            echo "---- DEBUG ----"
            echo "Path: "
            pwd
            echo "Directory: "
            ls -la
            ls -la ui
          '';

          npmInstall = ''
            cd ui;
            ln -s ${nodeDeps}/lib/node_modules ./node_modules;
            export PATH="${nodeDeps}/bin:$PATH";
            npm run build --verbose;
            cd ..;
          '';

          installPhase = ''
            # Copy kellnr binary into bin directory
            mkdir -p $out/bin;
            cp target/release/kellnr $out/bin;

            # Copy default config into bin directory
            mkdir -p $out/bin/config;
            cp config/default.toml $out/bin/config;

            # Copy the built UI into the bin directory
            mkdir -p $out/bin/static;
            cp -r ui/dist/* $out/bin/static;

            # Debug output
            ls -la $out/bin;
            ls -la $out/bin/static;
            ls -la $out/bin/config;
          '';

          # fixupPhase = ''
          # '';

        });
      in
      with pkgs;
      {
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

          packages = [
            pkgs.rust-analyzer
            pkgs.cargo-nextest
            pkgs.cargo-machete
            pkgs.lazygit
            pkgs.just
            pkgs.node2nix
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


