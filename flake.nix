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

        # Rust toolchain
        rustToolchain = pkgs.rust-bin.stable.latest.default;

        # Crane library for Rust builds
        craneLib = (crane.mkLib pkgs).overrideToolchain (_: rustToolchain);

        # Build the UI with buildNpmPackage
        uiAssets = pkgs.buildNpmPackage {
          pname = "kellnr-ui";
          version = "0.1.0";
          src = ./ui;

          npmDepsHash = "sha256-d58iw7aMQddUGT0EQYq150rlOnVmMOKYFqdpJXgVRmo=";

          # Don't run the default build, we need vite build
          buildPhase = ''
            npm run build
          '';

          installPhase = ''
            cp -r dist $out
          '';

          # Node.js version
          nodejs = pkgs.nodejs_22;
        };

        # Source filtering for Rust
        rustFilter = path: type:
          (craneLib.filterCargoSources path type) ||
          # Include the pre-built UI assets location
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

            # Node.js for UI development
            nodejs_22

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
          ] ++ lib.optionals stdenv.isLinux [
            # Playwright browser dependencies for Linux
            alsa-lib
            at-spi2-atk
            atk
            cairo
            cups
            dbus
            expat
            fontconfig
            freetype
            gdk-pixbuf
            glib
            gtk3
            libdrm
            libnotify
            libuuid
            libxkbcommon
            mesa
            nspr
            nss
            pango
            xorg.libX11
            xorg.libXcomposite
            xorg.libXcursor
            xorg.libXdamage
            xorg.libXext
            xorg.libXfixes
            xorg.libXi
            xorg.libXrandr
            xorg.libXrender
            xorg.libXtst
            xorg.libxcb
            xorg.libxshmfence
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
            echo ""

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

            # Playwright setup
            export PLAYWRIGHT_BROWSERS_PATH=0

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
