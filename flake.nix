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

  outputs =
    { nixpkgs
    , flake-utils
    , crane
    , rust-overlay
    , ...
    }:
    flake-utils.lib.eachSystem [ "aarch64-darwin" "x86_64-darwin" "aarch64-linux" "x86_64-linux" ] (
      system:
      let
        overlays = [ (import rust-overlay) ];

        pkgs = import nixpkgs {
          inherit system overlays;
        };

        inherit (pkgs) lib;

        # Rust toolchain with llvm-tools for coverage
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "llvm-tools-preview" ];
        };

        # Crane library for Rust builds
        craneLib = (crane.mkLib pkgs).overrideToolchain (_: rustToolchain);

        # Build the UI with buildNpmPackage
        # NOTE: When package-lock.json changes, the npmDepsHash must be updated.
        # For Dependabot PRs, the hash is auto-updated by the dependabot-nix-hash workflow.
        # For manual updates, run: nix run nixpkgs#prefetch-npm-deps -- ui/package-lock.json
        uiAssets = pkgs.buildNpmPackage {
          pname = "kellnr-ui";
          version = "0.1.0";
          src = ./ui;

          npmDepsHash = "sha256-e03UE/xS8hv4gMgVxlqjJWXPVLjeWGqupprbjIiNKf4=";

          buildPhase = ''
            npm run build
          '';

          installPhase = ''
            cp -r dist $out
          '';

          nodejs = pkgs.nodejs_22;
        };

        # Source filtering for Rust
        rustFilter = path: type:
          (craneLib.filterCargoSources path type) ||
          (lib.hasInfix "crates/embedded-resources/static" path);

        src = lib.cleanSourceWith {
          src = craneLib.path ./.;
          filter = rustFilter;
        };

        # Common Rust build arguments
        commonArgs = {
          inherit src;
          strictDeps = true;
          pname = "kellnr";

          nativeBuildInputs = [
            pkgs.cmake
            pkgs.pkg-config
          ] ++ lib.optionals pkgs.stdenv.isLinux [
            pkgs.rustPlatform.bindgenHook
          ];

          buildInputs = [
            pkgs.openssl.dev
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          OPENSSL_NO_VENDOR = "1";
        };

        # Build dependencies separately for caching
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Build the full package
        kellnr = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          doCheck = false;

          # Copy UI assets before building
          preBuild = ''
            mkdir -p crates/embedded-resources/static
            cp -r ${uiAssets}/* crates/embedded-resources/static/
          '';
        });
      in
      {
        checks = {
          inherit kellnr;

          fmt = craneLib.cargoFmt { inherit src; };

          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--workspace --all-targets -- --deny warnings";
          });
        };

        packages = {
          default = kellnr;
          inherit kellnr uiAssets;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = kellnr;
        };

        devShells.default = craneLib.devShell {

          inputsFrom = [ kellnr ];

          packages = with pkgs; [
            # Rust tools
            rust-analyzer
            cargo-nextest
            cargo-machete
            cargo-cyclonedx
            cargo-llvm-cov

            # Node.js for UI development
            nodejs_24

            # Database tools
            sea-orm-cli

            # Dev utilities
            just
            lazygit
            jq
            curl
            gnused

            # Nix tools
            nixpkgs-fmt
            statix

            # Testing
            python3
            playwright-driver.browsers
          ] ++ lib.optionals stdenv.isLinux [
          ];

          shellHook = ''
            echo "Kellnr Development Environment"
            echo "==============================="
            echo "Rust version: $(rustc --version)"
            echo "Node.js version: $(node --version)"
            echo ""
            echo "Commands:"
            echo "  just build     - Build kellnr"
            echo "  just run       - Run kellnr locally"
            echo "  just test      - Run tests"
            echo "  just npm-dev   - Run UI dev server"
            echo "  just sbom      - Generate SBOM (CycloneDX)"
            echo ""

            # LLVM tools for cargo-llvm-cov
            export LLVM_COV="${pkgs.llvmPackages.bintools-unwrapped}/bin/llvm-cov"
            export LLVM_PROFDATA="${pkgs.llvmPackages.bintools-unwrapped}/bin/llvm-profdata"

            # Setup custom CA certificate for testing
            export CUSTOM_CERT_DIR="$PWD/.certs"
            if [ -d "$CUSTOM_CERT_DIR" ]; then
              rm -rf "$CUSTOM_CERT_DIR"
            fi
            mkdir -p "$CUSTOM_CERT_DIR"
            chmod 755 "$CUSTOM_CERT_DIR"

            export COMBINED_CERT_FILE="$CUSTOM_CERT_DIR/combined-ca-bundle.pem"
            cp "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" "$COMBINED_CERT_FILE"
            chmod 644 "$COMBINED_CERT_FILE"

            if [ -f "$PWD/tests/ca.crt" ]; then
              cat "$PWD/tests/ca.crt" >> "$COMBINED_CERT_FILE"
            fi

            export SSL_CERT_FILE="$COMBINED_CERT_FILE"
            export NIX_SSL_CERT_FILE="$COMBINED_CERT_FILE"
            export REQUESTS_CA_BUNDLE="$COMBINED_CERT_FILE"
            export NODE_EXTRA_CA_CERTS="$COMBINED_CERT_FILE"

            # Playwright setup for NixOS
            # Use pre-patched browsers from nixpkgs instead of downloading
            export PLAYWRIGHT_BROWSERS_PATH="${pkgs.playwright-driver.browsers}"
            export PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
            export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
            echo "Playwright browsers: $PLAYWRIGHT_BROWSERS_PATH"
            echo "Nixpkgs playwright version: ${pkgs.playwright-driver.version}"
            echo "NOTE: Your tests/package.json @playwright/test version must match!"

            alias c=cargo
            alias j=just
            alias lg=lazygit
          '' + lib.optionalString pkgs.stdenv.isDarwin ''
            export DYLD_LIBRARY_PATH="$(rustc --print sysroot)/lib:$DYLD_LIBRARY_PATH"
          '';
        };
      }
    );
}
