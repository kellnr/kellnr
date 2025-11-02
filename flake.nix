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

  outputs = { self, nixpkgs, flake-utils, crane, rust-overlay, ... }:
    flake-utils.lib.eachSystem [ "aarch64-darwin" "x86_64-darwin" "aarch64-linux" "x86_64-linux" ] (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        inherit (pkgs) lib;

        # Define cross-compilation targets
        targets = [
          "x86_64-unknown-linux-gnu"
          "x86_64-unknown-linux-musl"
          "aarch64-unknown-linux-gnu"
          "aarch64-unknown-linux-musl"
        ];

        # Cross-compilation package sets
        pkgsCrossX86_64 = import nixpkgs {
          inherit system;
          crossSystem = lib.systems.examples.gnu64;
          overlays = [ (import rust-overlay) ];
        };

        pkgsCrossX86_64Musl = import nixpkgs {
          inherit system;
          crossSystem = lib.systems.examples.musl64;
          overlays = [ (import rust-overlay) ];
        };

        pkgsCrossAarch64 = import nixpkgs {
          inherit system;
          crossSystem = lib.systems.examples.aarch64-multiplatform;
          overlays = [ (import rust-overlay) ];
        };

        pkgsCrossAarch64Musl = import nixpkgs {
          inherit system;
          crossSystem = lib.systems.examples.aarch64-multiplatform-musl;
          overlays = [ (import rust-overlay) ];
        };

        # Create crane lib with all targets
        craneLib = (crane.mkLib pkgs).overrideToolchain (
          p: p.rust-bin.stable.latest.default.override {
            inherit targets;
          }
        );

        # Set a filter of files that are included in the build source directory.
        webuiFilter = path: _type:
          let extensions = [ "js" "json" "ts" "vue" "html" "png" "css" "svg" ];
          in lib.any (ext: lib.hasSuffix ".${ext}" path) extensions;
        webuiOrCargo = path: type:
          (webuiFilter path type) || (craneLib.filterCargoSources path type);
        src = lib.cleanSourceWith {
          src = craneLib.path ./.;
          filter = webuiOrCargo;
        };

        # Install the NPM dependencies
        node2nixOutput = import ui/nix {
          inherit pkgs system;
          nodejs = pkgs.nodejs_24;
        };
        nodeDeps = node2nixOutput.nodeDependencies;

        # Function to build UI assets (only needed once)
        buildUiAssets = ''
          cd ui;
          ln -s ${nodeDeps}/lib/node_modules ./node_modules;
          export PATH="${nodeDeps}/bin:$PATH";
          npm run build --verbose;
          cd ..;
        '';

        # Common arguments for all builds
        commonArgs = {
          inherit src;
          strictDeps = true;
          pname = "kellnr";

          nativeBuildInputs = [
            pkgs.cmake
            pkgs.nodejs_24
            pkgs.pkg-config
            pkgs.rustPlatform.bindgenHook
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];

          buildInputs = [
            pkgs.cargo-nextest
            pkgs.openssl.dev
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            pkgs.iconv
            pkgs.cacert
            pkgs.curl
          ];

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${pkgs.llvmPackages.libclang.lib}/lib/clang/${pkgs.lib.getVersion pkgs.clang}/include";
        };

        # Build *just* the cargo dependencies, so we can reuse
        # all of that work (e.g. via cachix) when running in CI
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Build the actual crate for the host system
        hostPackage = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts nodeDeps;
            doCheck = false;

            preConfigurePhases = [ "npmBuild" ];
            npmBuild = buildUiAssets;

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
          }
        );

        # Helper function to build for specific target with proper cross-compilation support
        mkTargetBuild = { target, targetPkgs, isCross ? false }:
          let
            # Create crane lib for the target packages
            targetCraneLib = (crane.mkLib targetPkgs).overrideToolchain (
              p: p.rust-bin.stable.latest.default.override {
                targets = [ target ];
              }
            );

            # Build arguments specific to the target
            targetCommonArgs = {
              inherit src;
              strictDeps = true;
              pname = "kellnr";

              nativeBuildInputs = [
                pkgs.cmake
                pkgs.nodejs_24
                pkgs.rustPlatform.bindgenHook  # Use host bindgenHook for cross-compilation
              ] ++ lib.optionals isCross [
                pkgs.stdenv.cc
                pkgs.pkg-config
              ] ++ lib.optionals (!isCross) [
                targetPkgs.stdenv.cc
                targetPkgs.pkg-config
              ];

              buildInputs = [
                pkgs.cargo-nextest
                targetPkgs.openssl.dev
                targetPkgs.openssl
              ];

              CARGO_BUILD_TARGET = target;

              # Use host libclang for bindgen (it runs on host, not target)
              LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
              BINDGEN_EXTRA_CLANG_ARGS = if isCross then
                builtins.concatStringsSep " " [
                  "--target=${target}"
                  "-nostdinc"
                  "-isystem ${targetPkgs.stdenv.cc.libc.dev}/include"
                  "-isystem ${pkgs.llvmPackages.libclang.lib}/lib/clang/${lib.getVersion pkgs.clang}/include"
                ]
              else
                "-isystem ${pkgs.llvmPackages.libclang.lib}/lib/clang/${lib.getVersion pkgs.clang}/include";

              # Override NIX_CFLAGS_COMPILE to prevent wrong headers during cross-compilation
              NIX_CFLAGS_COMPILE = lib.optionalString isCross "-isystem ${targetPkgs.stdenv.cc.libc.dev}/include";

              # OpenSSL configuration for cross-compilation
              OPENSSL_DIR = "${targetPkgs.openssl.dev}";
              OPENSSL_LIB_DIR = "${targetPkgs.openssl.out}/lib";
              OPENSSL_INCLUDE_DIR = "${targetPkgs.openssl.dev}/include";
              PKG_CONFIG_PATH = "${targetPkgs.openssl.dev}/lib/pkgconfig";

              # Configure cross-compilation environment
              depsBuildBuild = lib.optionals isCross [
                pkgs.stdenv.cc
              ];

              # Set CC and AR for cross-compilation
              CC = if isCross then "${targetPkgs.stdenv.cc}/bin/${targetPkgs.stdenv.cc.targetPrefix}cc" else null;
              AR = if isCross then "${targetPkgs.stdenv.cc.bintools}/bin/${targetPkgs.stdenv.cc.targetPrefix}ar" else null;

              # Cargo environment for cross-compilation
              CARGO_BUILD_RUSTFLAGS = lib.optionalString isCross "-C linker=${targetPkgs.stdenv.cc}/bin/${targetPkgs.stdenv.cc.targetPrefix}cc";
            };

            targetCargoArtifacts = targetCraneLib.buildDepsOnly targetCommonArgs;
          in
          targetCraneLib.buildPackage (
            targetCommonArgs
            // {
              inherit nodeDeps;
              cargoArtifacts = targetCargoArtifacts;
              doCheck = false;

              preConfigurePhases = [ "npmBuild" ];
              npmBuild = buildUiAssets;

              installPhase =
                let
                  binDir = "$out/bin";
                  configDir = "${binDir}/config";
                  staticDir = "${binDir}/static";
                  binaryPath = "target/${target}/release/kellnr";
                in
                ''
                  # Copy kellnr binary into bin directory
                  mkdir -p ${binDir};
                  cp ${binaryPath} ${binDir}/kellnr;

                  # Copy default config
                  mkdir -p ${configDir};
                  cp config/default.toml ${configDir};

                  # Copy the built UI
                  mkdir -p ${staticDir};
                  cp -r ui/dist/* ${staticDir};
                '';
            }
          );

        # Build for x86_64 GNU (native or cross-compiled)
        kellnr-x86_64-gnu = mkTargetBuild {
          target = "x86_64-unknown-linux-gnu";
          targetPkgs = if system == "x86_64-linux" then pkgs else pkgsCrossX86_64;
          isCross = system != "x86_64-linux";
        };

        # Build for x86_64 MUSL (always cross-compiled from perspective of most systems)
        kellnr-x86_64-musl = mkTargetBuild {
          target = "x86_64-unknown-linux-musl";
          targetPkgs = pkgsCrossX86_64Musl;
          isCross = true;
        };

        # Build for aarch64 GNU (native or cross-compiled)
        kellnr-aarch64-gnu = mkTargetBuild {
          target = "aarch64-unknown-linux-gnu";
          targetPkgs = if system == "aarch64-linux" then pkgs else pkgsCrossAarch64;
          isCross = system != "aarch64-linux";
        };

        # Build for aarch64 MUSL (always cross-compiled from perspective of most systems)
        kellnr-aarch64-musl = mkTargetBuild {
          target = "aarch64-unknown-linux-musl";
          targetPkgs = pkgsCrossAarch64Musl;
          isCross = true;
        };

      in
      with pkgs;
      {
        checks = {
          inherit hostPackage;

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
          inputsFrom = [ hostPackage ];

          shellHook = ''
            echo "Kellnr Development Environment"
            echo "==========================="
            echo "Rust version: $(rustc --version)"
            echo "Cargo version: $(cargo --version)"
            echo "Node.js version: $(node --version)"
            echo "NPM version: $(npm --version)"
            echo "Nixpkgs version: ${nixpkgs.lib.version}"
            echo "Lua version: $(lua -v)"
            echo "Docker version: $(docker --version 2>/dev/null || echo 'Docker not available')"

            # Setup custom CA certificate for testing against local Kellnr registries
            export CUSTOM_CERT_DIR="$PWD/.certs"

            # Remove existing directory if it exists with wrong permissions
            if [ -d "$CUSTOM_CERT_DIR" ]; then
              rm -rf "$CUSTOM_CERT_DIR"
            fi

            # Create directory with explicit permissions
            mkdir -p "$CUSTOM_CERT_DIR"
            chmod 755 "$CUSTOM_CERT_DIR"  # Set correct directory permissions

            # Create combined cert bundle with explicit permissions
            export COMBINED_CERT_FILE="$CUSTOM_CERT_DIR/combined-ca-bundle.crt"
            cp "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" "$COMBINED_CERT_FILE"
            chmod 644 "$COMBINED_CERT_FILE"  # Set read/write permissions

            # Add the certificate from tests/ca.crt
            if [ -f "$PWD/tests/ca.crt" ]; then
              cat "$PWD/tests/ca.crt" >> "$COMBINED_CERT_FILE"
              echo "Added tests/ca.crt certificate to CA bundle"
            else
              echo "Warning: tests/ca.crt not found"
            fi

            # Set SSL cert environment variables to use the combined bundle
            export SSL_CERT_FILE="$COMBINED_CERT_FILE"
            export NIX_SSL_CERT_FILE="$COMBINED_CERT_FILE"
            export REQUESTS_CA_BUNDLE="$COMBINED_CERT_FILE"
            export NODE_EXTRA_CA_CERTS="$COMBINED_CERT_FILE"

            alias c=cargo
            alias j=just
            alias lg=lazygit

            # Ensure the script can find modules in the current directory and parent directory
            export LUA_PATH="./?.lua;../?.lua;$(lua -e 'print(package.path)')"
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
            nixpkgs-fmt
            statix
            lua5_4
            lua54Packages.luasocket
            lua54Packages.luafilesystem
            lua54Packages.cjson
            lua54Packages.http
            jq
            curl
            gnused
          ];
        });

        packages = {
          default = hostPackage;
          x86_64-unknown-linux-gnu = kellnr-x86_64-gnu;
          x86_64_unknown_linux_gnu = kellnr-x86_64-gnu;
          x86_64-unknown-linux-musl = kellnr-x86_64-musl;
          x86_64_unknown_linux_musl = kellnr-x86_64-musl;
          aarch64-unknown-linux-gnu = kellnr-aarch64-gnu;
          aarch64_unknown_linux_gnu = kellnr-aarch64-gnu;
          aarch64-unknown-linux-musl = kellnr-aarch64-musl;
          aarch64_unknown_linux_musl = kellnr-aarch64-musl;
        };

        apps = {
          default = flake-utils.lib.mkApp {
            drv = hostPackage;
          };
          x86_64-unknown-linux-gnu = flake-utils.lib.mkApp {
            drv = kellnr-x86_64-gnu;
          };
          x86_64_unknown_linux_gnu = flake-utils.lib.mkApp {
            drv = kellnr-x86_64-gnu;
          };
          x86_64-unknown-linux-musl = flake-utils.lib.mkApp {
            drv = kellnr-x86_64-musl;
          };
          x86_64_unknown_linux_musl = flake-utils.lib.mkApp {
            drv = kellnr-x86_64-musl;
          };
          aarch64-unknown-linux-gnu = flake-utils.lib.mkApp {
            drv = kellnr-aarch64-gnu;
          };
          aarch64_unknown_linux_gnu = flake-utils.lib.mkApp {
            drv = kellnr-aarch64-gnu;
          };
          aarch64-unknown-linux-musl = flake-utils.lib.mkApp {
            drv = kellnr-aarch64-musl;
          };
          aarch64_unknown_linux_musl = flake-utils.lib.mkApp {
            drv = kellnr-aarch64-musl;
          };
        };
      }
    );
}
