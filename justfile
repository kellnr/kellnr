#!/usr/bin/env just --justfile

############################################
# Just commands for Kellnr
#
# It's recommended to use the just commands
# instead of the cargo commands, as they
# provide additional functionality.
#
# Run `just` to see the list of available commands.
#
############################################


# How to call the current just executable. Note that just_executable() may have `\` in Windows paths, so we need to quote it.
just := quote(just_executable())
# cargo-binstall needs a workaround due to caching when used in CI
binstall_args := if env('CI', '') != '' {'--no-confirm --no-track --disable-telemetry'} else {''}

# if running in CI, treat warnings as errors by setting RUSTFLAGS and RUSTDOCFLAGS to '-D warnings' unless they are already set
# Use `CI=true just ci-test` to run the same tests as in GitHub CI.
# Use `just env-info` to see the current values of RUSTFLAGS and RUSTDOCFLAGS
ci_mode := if env('CI', '') != '' {'1'} else {''}
export RUSTFLAGS := env('RUSTFLAGS', if ci_mode == '1' {'-D warnings'} else {''})
export RUSTDOCFLAGS := env('RUSTDOCFLAGS', if ci_mode == '1' {'-D warnings'} else {''})
export RUST_BACKTRACE := env('RUST_BACKTRACE', if ci_mode == '1' {'1'} else {'0'})

# Detect if docker is installed: "1" if installed, "0" otherwise
# Use `just env-info` to see if Docker is detected
# Docker is needed for the Postgresql integration tests
has_docker := if `command -v docker > /dev/null 2>&1; echo $?` == "0" { "1" } else { "0" }
test_pgdb := if has_docker == "1" { "cargo nextest run --workspace -E 'test(~postgres_)'" } else { "echo 'ERROR: Docker is not installed. The Postgresql integration tests require Docker'" }
test_smoke := if has_docker == "1" { "cd tests && npm install && npx playwright test" } else { "echo 'ERROR: Docker is not installed. The smoke tests require Docker'" }
test_ui_all_browsers := if has_docker == "1" { "cd tests && npm install && npx playwright install --with-deps && PLAYWRIGHT_UI=1 npx playwright test" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }
test_ui_chromium := if has_docker == "1" { "cd tests && npm install && npx playwright install --with-deps && PLAYWRIGHT_UI=1 npx playwright test --project=chromium" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }
test_ui_firefox := if has_docker == "1" { "cd tests && npm install && npx playwright install --with-deps && PLAYWRIGHT_UI=1 npx playwright test --project=firefox" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }
test_ui_webkit := if has_docker == "1" { "cd tests && npm install && npx playwright install --with-deps && PLAYWRIGHT_UI=1 npx playwright test --project=webkit" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }
test_ui_headed := if has_docker == "1" { "cd tests && npm install && npx playwright install --with-deps && PLAYWRIGHT_UI=1 npx playwright test --headed" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }

##########################################
# Common commands
##########################################

@_default:
    {{just}} --list

# Print environment info
env-info:
    @echo "Running {{if ci_mode == '1' {'in CI mode'} else {'in dev mode'} }} {{if has_docker == '1' {'with'} else {'without'} }} Docker on {{os()}} / {{arch()}}"
    @echo "PWD {{justfile_directory()}}"
    {{just}} --version
    rustc --version
    cargo --version
    rustup --version
    @echo "RUSTFLAGS='$RUSTFLAGS'"
    @echo "RUSTDOCFLAGS='$RUSTDOCFLAGS'"
    @echo "RUST_BACKTRACE='$RUST_BACKTRACE'"

# Quick compile without building a binary
check:
    cargo check

# Build the project
build:
    cargo build --features vendored-openssl

build-release: npm-build
    cargo build --release --features vendored-openssl

clippy:
  cargo clippy --workspace --all-targets --all-features

run: npm-build build
    cargo run

# Run all tests which do NOT require Docker
test: npm-build
    cargo nextest run --workspace -E 'not test(~postgres_)'

# Run the smoke tests which require Docker
test-smoke:
    {{test_smoke}}

test-ui: # Run Playwright UI tests (requires Docker)
	{{test_ui_chromium}}

test-ui-all-browser: # Run Playwright UI tests in all browsers (requires Docker)
	{{test_ui_all_browsers}}

test-ui-chromium: # Run Playwright UI tests in Chromium only (requires Docker)
	{{test_ui_chromium}}

test-ui-firefox: # Run Playwright UI tests in Firefox only (requires Docker)
	{{test_ui_firefox}}

test-ui-webkit: # Run Playwright UI tests in WebKit only (requires Docker)
	{{test_ui_webkit}}

test-ui-headed: # Run Playwright UI tests with browser visible (requires Docker)
	{{test_ui_headed}}

# Run Postgresql integration tests which require Docker
test-pgdb: npm-build
    {{test_pgdb}}

test-all: test test-pgdb test-ui

# Find unused dependencies. Uses `cargo-udeps`
udeps:  (cargo-install 'cargo-udeps')
    cargo +nightly udeps --workspace --all-features --all-targets

# Clean all build artifacts
clean:
    cargo clean

clean-node:
    rm -rf ui/node_modules
    rm -rf ui/package-lock.json

clean-static:
    rm -rf crates/embedded-resources/static

clean-all: clean clean-node clean-static

# Reformat all code `cargo fmt`. If nightly is available, use it for better results
fmt:
    #!/usr/bin/env bash
    set -euo pipefail
    if (rustup toolchain list | grep nightly && rustup component list --toolchain nightly | grep rustfmt) &> /dev/null; then
        echo 'Reformatting Rust code using nightly Rust fmt to sort imports'
        cargo +nightly fmt --all -- --config imports_granularity=Module,group_imports=StdExternalCrate
    else
        echo 'Reformatting Rust with the stable cargo fmt.  Install nightly with `rustup install nightly` for better results'
        cargo fmt --all
    fi

npm-dev:
    cd ui && npm run dev

npm-build: npm-install clean-static
    cd ui && npm run build
    mkdir -p crates/embedded-resources/static
    cp -r ui/dist/* crates/embedded-resources/static/

npm-install:
    cd ui && npm install

# Check if a certain Cargo command is installed, and install it if needed
[private]
cargo-install $COMMAND $INSTALL_CMD='' *args='':
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v $COMMAND > /dev/null; then
        echo "$COMMAND could not be found. Installing..."
        if ! command -v cargo-binstall > /dev/null; then
            set -x
            cargo install ${INSTALL_CMD:-$COMMAND} --locked {{args}}
            { set +x; } 2>/dev/null
        else
            set -x
            cargo binstall ${INSTALL_CMD:-$COMMAND} {{binstall_args}} --locked {{args}}
            { set +x; } 2>/dev/null
        fi
    fi

##########################################
# Commands used by the Nix package manager
##########################################

# Used to create the needed Nix expressions for the Node.js dependencies.
# Run this command everytime you edit the ui/package.json file.
node2nix: clean-node patch-package
    node2nix \
        --development \
        --input ui/nix/package.json \
        --node-env ui/nix/node-env.nix \
        --composition ui/nix/default.nix \
        --output ui/nix/node-package.nix

patch-package:
    jd -o ui/nix/package.json \
       -p \
       -f patch ui/nix/package-patch.json ui/package.json || true


##########################################
# Commands used by the Github Actions CI
##########################################

# Set the target for the ci-release command.
# The target can be "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu",
# "x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl".
# It's used by the Github Actions CI to build the release binary for the specified target.
target := "x86_64-unknown-linux-gnu"

ci-release: clean npm-build
    cross build --release --target {{target}} --features vendored-openssl

##########################################
# Commands for cross-rs to build the
# release binary for different targets
##########################################

x-aarch64-musl:
    cross build --target aarch64-unknown-linux-musl --features vendored-openssl

x-aarch64-gnu:
    cross build --target aarch64-unknown-linux-gnu --features vendored-openssl

x-x86_64-musl:
    cross build --target x86_64-unknown-linux-musl --features vendored-openssl

x-x86_64-gnu:
    cross build --target x86_64-unknown-linux-gnu --features vendored-openssl

x-all: x-aarch64-musl x-aarch64-gnu x-x86_64-musl x-x86_64-gnu

##########################################
# Aliases
##########################################

alias b := build
alias br := build-release
alias r := run
alias t := test
alias c := check
alias tui := test-ui
alias tuic := test-ui-chromium
