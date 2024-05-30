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

        craneLib = crane.lib.${system};
        src = craneLib.cleanCargoSource (craneLib.path ./.);

        commonArgs = {
          inherit src;
          strictDeps = true;
          pname = "kellnr";

          nativeBuildInputs = [
            pkgs.cmake
          ] ++ lib.optionals pkgs.stdenv.isLinux [
            pkgs.pkg-config
            pkgs.rustPlatform.bindgenHook
          ];

          buildInputs = [
            pkgs.nodejs_22
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

        # Build the actual crate itself, reusing the dependency
        # artifacts from above.
        kellnr-crate = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;

          installPhase = ''
          '';

          fixupPhase = ''
          '';

        });
      in
      with pkgs;
      {
        devShells.default = craneLib.devShell (commonArgs // {
          inputsFrom = [ kellnr-crate ];

          shellHook = ''
            alias c=cargo
	    alias cta="cargo nextest run --workspace -E 'not binary_id(db::postgres_test)'"
            alias ctai="cargo nextest run --workspace"
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


