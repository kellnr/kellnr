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
    flake-utils.lib.eachSystem [ "aarch64-darwin" "x86_64-darwin" "aarch64-linux" "x86_64-linux" ] (system:
      let
        # Define cross compilation targets as Nixpkgs system strings
        targets = [
          "x86_64-unknown-linux-gnu"
          "x86_64-unknown-linux-musl"
          "aarch64-unknown-linux-gnu"
          "aarch64-unknown-linux-musl"
        ];

        overlays = [ (import rust-overlay) ];

        # Function to get pkgs for a given system/crossSystem
        pkgsFor = { localSystem, crossSystem ? null }:
          import nixpkgs ({
            inherit localSystem overlays;
          } // (if crossSystem != null then { inherit crossSystem; } else { }));

        # Function to get a rust toolchain with all targets for a given pkgs
        rustWithTargetsFor = pkgs: pkgs.rust-bin.stable.latest.default.override {
          targets = targets;
        };

        # Base pkgs for the host system
        pkgs = pkgsFor { localSystem = system; };

        inherit (pkgs) lib;

        # Set a filter of files that are included in the build source directory.
        webuiFilter = path: _type:
          let extensions = [ "js" "json" "ts" "vue" "html" "png" "css" "svg" ];
          in lib.any (ext: lib.hasSuffix ".${ext}" path) extensions;
        webuiOrCargo = path: type:
          (webuiFilter path type) || ((crane.mkLib pkgs).filterCargoSources path type);
        src = lib.cleanSourceWith {
          src = (crane.mkLib pkgs).path ./.;
          filter = webuiOrCargo;
        };

        # Common arguments for all builds
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
          ];

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${pkgs.llvmPackages.libclang.lib}/lib/clang/${pkgs.lib.getVersion pkgs.clang}/include";

          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
        };

        # Standard cargo artifacts for the host system
        baseCraneLib = (crane.mkLib pkgs).overrideToolchain (_: rustWithTargetsFor pkgs);
        cargoArtifacts = baseCraneLib.buildDepsOnly commonArgs;

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

        # Function to create a package for a specific target
        makePackage = target:
          let
            # Use Nixpkgs crossSystem for cross builds
            crossPkgs = pkgsFor { localSystem = system; crossSystem = target; };
            # Use the buildPackages toolchain for cross builds!
            craneLib = (crane.mkLib crossPkgs).overrideToolchain (_: rustWithTargetsFor crossPkgs.buildPackages);

            targetArgs = commonArgs // {
              inherit cargoArtifacts nodeDeps;
              doCheck = false;

              # Set Rust target
              CARGO_BUILD_TARGET = target;

              # Use the target's OpenSSL
              OPENSSL_DIR = "${crossPkgs.openssl.dev}";
              OPENSSL_INCLUDE_DIR = "${crossPkgs.openssl.dev}/include";
              OPENSSL_LIB_DIR = "${crossPkgs.openssl.out}/lib";

              # Add proper build inputs
              buildInputs = commonArgs.buildInputs ++ [
                crossPkgs.openssl.dev
                crossPkgs.openssl.out
              ];

              # Build phases
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
            };
          in
          craneLib.buildPackage targetArgs;

        # Build package for each target
        packages = builtins.listToAttrs (map
          (target: {
            name = builtins.replaceStrings [ "-" ] [ "_" ] target;
            value = makePackage target;
          })
          targets
        );

        # Also build for the host system
        hostPackage = baseCraneLib.buildPackage (commonArgs // {
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
        });
      in
      with pkgs;
      {
        checks = {
          inherit hostPackage;

          # Check formatting with rustfmt.
          fmt = baseCraneLib.cargoFmt (commonArgs // {
            inherit src;
          });

          # Check for clippy warnings.
          clippy = baseCraneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--workspace --all-targets -- --deny warnings";
          });
        };

        devShells.default = baseCraneLib.devShell (commonArgs // {
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
            export COMBINED_CERT_FILE="$CUSTOM_CERT_DIR/combined-ca-bundle.pem"
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
            jq
            curl
            gnused
          ];
        });

        # Make all cross-compiled packages available
        packages = packages // {
          default = hostPackage;
        };

        apps = {
          default = flake-utils.lib.mkApp {
            drv = hostPackage;
          };
        } // builtins.listToAttrs (map
          (target: {
            name = builtins.replaceStrings [ "-" ] [ "_" ] target;
            value = flake-utils.lib.mkApp {
              drv = packages.${builtins.replaceStrings [ "-" ] [ "_" ] target};
            };
          })
          targets
        );
      }
    );
}
