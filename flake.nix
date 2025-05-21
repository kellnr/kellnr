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
        pkgs = import nixpkgs { inherit system; overlays = [ (import rust-overlay) ]; };
        inherit (pkgs) lib;

        # Define cross compilation targets
        targets = [
          "x86_64-unknown-linux-gnu"
          "x86_64-unknown-linux-musl"
          "aarch64-unknown-linux-gnu"
          "aarch64-unknown-linux-musl"
        ];

        # Configure a rust toolchain with all targets
        rustWithTargets = pkgs.rust-bin.stable.latest.default.override {
          targets = targets;
        };

        # Base crane lib with our toolchain - use function for cross-compilation compatibility
        baseCraneLib = (crane.mkLib pkgs).overrideToolchain (pkgs: rustWithTargets);

        # Set a filter of files that are included in the build source directory.
        webuiFilter = path: _type:
          let extensions = [ "js" "json" "ts" "vue" "html" "png" "css" "svg" ];
          in lib.any (ext: lib.hasSuffix ".${ext}" path) extensions;
        webuiOrCargo = path: type:
          (webuiFilter path type) || (baseCraneLib.filterCargoSources path type);
        src = lib.cleanSourceWith {
          src = baseCraneLib.path ./.;
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
            # Extract architecture and libc from target
            targetParts = builtins.split "-" target;
            arch = builtins.elemAt targetParts 0;
            libc = if builtins.elemAt targetParts 3 == "musl" then "musl" else "gnu";

            # Use appropriate pkgsCross based on target
            crossPkgs =
              if pkgs.stdenv.isDarwin then
                if arch == "x86_64" && libc == "musl" then
                  pkgs.pkgsCross.musl64
                else if arch == "aarch64" && libc == "musl" then
                  pkgs.pkgsCross.aarch64-multiplatform-musl
                else if arch == "aarch64" && libc == "gnu" then
                  pkgs.pkgsCross.aarch64-multiplatform
                else if arch == "x86_64" && libc == "gnu" then
                  pkgs.pkgsCross.gnu64
                else
                  pkgs
              else
                if arch == "x86_64" && libc == "musl" then
                  pkgs.pkgsCross.musl64
                else if arch == "aarch64" && libc == "musl" then
                  pkgs.pkgsCross.aarch64-multiplatform-musl
                else if arch == "aarch64" && libc == "gnu" then
                  pkgs.pkgsCross.aarch64-multiplatform
                else
                  pkgs;

            # Get the target OpenSSL
            targetOpenSsl = crossPkgs.openssl;

            # Find the cross compiler binaries
            crossLd =
              if libc == "musl" then
                "${crossPkgs.stdenv.cc}/bin/${target}-clang"
              else
                "${crossPkgs.stdenv.cc}/bin/${target}-gcc";

            crossAr = "${crossPkgs.stdenv.cc}/bin/${target}-ar";

            # Create crane lib specific to this target
            craneLib = (crane.mkLib crossPkgs).overrideToolchain (p: rustWithTargets);

            # Create a cargo config for cross-compilation
            cargoConfigContent = ''
              [target.${target}]
              linker = "${crossLd}"
              ar = "${crossAr}"
              rustflags = [
                "-C", "link-arg=-Wl,-O1",
                "-C", "link-arg=-Wl,--strip-all"
              ]
              
              [build]
              target = "${target}"
              
              [profile.release]
              lto = true
            '';

            # Target-specific build arguments
            targetArgs = commonArgs // {
              inherit cargoArtifacts nodeDeps;
              doCheck = false;

              # Set Rust target
              CARGO_BUILD_TARGET = target;

              # Use the target's OpenSSL
              OPENSSL_DIR = "${targetOpenSsl.dev}";
              OPENSSL_INCLUDE_DIR = "${targetOpenSsl.dev}/include";
              OPENSSL_LIB_DIR = "${targetOpenSsl.out}/lib";

              # Add proper build inputs
              buildInputs = commonArgs.buildInputs ++
                [
                  targetOpenSsl.dev
                  targetOpenSsl.out
                  crossPkgs.stdenv.cc
                ] ++
                (if pkgs.stdenv.isDarwin && (libc == "gnu" || libc == "musl") then [
                  crossPkgs.stdenv.cc.libc
                ] else [ ]) ++
                (if libc == "musl" then [
                  pkgs.musl
                ] else [ ]);

              # Build phases
              preConfigurePhases = [ "npmBuild" "setupCargoConfig" "checkLinker" ];

              setupCargoConfig = ''
                mkdir -p .cargo
                cat > .cargo/config.toml << EOF
                ${cargoConfigContent}
                EOF
                cat .cargo/config.toml
              '';

              # Add a diagnostic phase to check if the linker exists
              checkLinker = ''
                echo "Checking cross compiler tools:"
                echo "Linker: ${crossLd}"
                echo "AR: ${crossAr}"
                
                if [ -e "${crossLd}" ]; then
                  echo "Linker found: ${crossLd}"
                  ls -la "${crossLd}"
                else
                  echo "ERROR: Linker not found: ${crossLd}"
                  echo "Contents of directory:"
                  ls -la ${crossPkgs.stdenv.cc}/bin/
                fi
                
                if [ -e "${crossAr}" ]; then
                  echo "AR found: ${crossAr}"
                else
                  echo "ERROR: AR not found: ${crossAr}"
                fi
              '';

              npmBuild = buildUiAssets;

              installPhase =
                let
                  binDir = "$out/bin";
                  configDir = "${binDir}/config";
                  staticDir = "${binDir}/static";

                  # Binary path depends on target
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
            nixpkgs-fmt
            statix
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
