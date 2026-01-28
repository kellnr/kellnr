############################################
# Just commands for Kellnr
#
# It's recommended to use the just commands
# instead of the cargo commands, as they
# provide additional functionality.
############################################
# Use PowerShell on Windows, sh elsewhere

set windows-powershell := true
set shell := ["sh", "-c"]

# Helper to setup MSVC environment on Windows before running cargo commands
# Usage: {{setup-msvc}} cargo build

setup-msvc := if os_family() == "windows" { ". ./.github/scripts/setup-msvc-env.ps1;" } else { "" }

##########################################
# Common commands
##########################################

@_default:
    {{ just_executable() }} --list

[unix]
check:
    cargo check

[windows]
check:
    {{ setup-msvc }} cargo check --no-default-features

[unix]
build:
    cargo build

[windows]
build:
    {{ setup-msvc }} cargo build --no-default-features

[unix]
build-release: npm-build
    cargo build --release

[windows]
build-release: npm-build
    {{ setup-msvc }} cargo build --release --no-default-features

[unix]
clippy:
    cargo clippy --workspace --all-targets --all-features

[windows]
clippy:
    {{ setup-msvc }} cargo clippy --workspace --all-targets --no-default-features

# Run clippy and fail on first warning (for CI)
[unix]
clippy-deny:
    cargo clippy --workspace --all-targets --all-features -- --deny warnings

[windows]
clippy-deny:
    {{ setup-msvc }} cargo clippy --workspace --all-targets --no-default-features -- --deny warnings

# Check formatting without modifying files (for CI)
[unix]
fmt-check:
    cargo fmt --all -- --check

[windows]
fmt-check:
    cargo fmt --all -- --check

[unix]
run: npm-build build
    cargo run

[windows]
run: npm-build build
    {{ setup-msvc }} cargo run --no-default-features

[unix]
test: npm-build
    cargo nextest run --workspace -E 'not test(~postgres_)'

[windows]
test: npm-build
    {{ setup-msvc }} cargo nextest run --workspace --no-default-features -E 'not test(~postgres_)'

[unix]
test-ui:
    {{ test_ui_chromium }}

[unix]
test-ui-all-browser:
    {{ test_ui_all_browsers }}

[unix]
test-ui-chromium:
    {{ test_ui_chromium }}

[unix]
test-ui-firefox:
    {{ test_ui_firefox }}

[unix]
test-ui-webkit:
    {{ test_ui_webkit }}

[unix]
test-ui-headed:
    {{ test_ui_headed }}

[windows]
test-ui:
    @echo "UI tests with Docker are not supported on Windows yet"

[windows]
test-ui-all-browser:
    @echo "UI tests with Docker are not supported on Windows yet"

[windows]
test-ui-chromium:
    @echo "UI tests with Docker are not supported on Windows yet"

[windows]
test-ui-firefox:
    @echo "UI tests with Docker are not supported on Windows yet"

[windows]
test-ui-webkit:
    @echo "UI tests with Docker are not supported on Windows yet"

[windows]
test-ui-headed:
    @echo "UI tests with Docker are not supported on Windows yet"

[unix]
test-pgdb: npm-build
    {{ test_pgdb }}

[windows]
test-pgdb: npm-build
    @echo "PostgreSQL Docker tests are not supported on Windows yet"

[unix]
test-all: test test-pgdb test-ui

[windows]
test-all: test

clean:
    cargo clean

[unix]
clean-node:
    rm -rf ui/node_modules
    rm -rf ui/package-lock.json

[windows]
clean-node:
    if (Test-Path ui/node_modules) { Remove-Item -Recurse -Force ui/node_modules }
    if (Test-Path ui/package-lock.json) { Remove-Item -Force ui/package-lock.json }

clean-all: clean clean-node

# Reformat all code `cargo fmt`. If nightly is available, use it for better results
[unix]
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

[windows]
fmt:
    $nightlyInstalled = (rustup toolchain list) -match 'nightly'; $fmtInstalled = if ($nightlyInstalled) { (rustup component list --toolchain nightly) -match 'rustfmt' } else { $false }; if ($nightlyInstalled -and $fmtInstalled) { Write-Host 'Reformatting Rust code using nightly Rust fmt to sort imports'; cargo +nightly fmt --all -- --config imports_granularity=Module,group_imports=StdExternalCrate } else { Write-Host 'Reformatting Rust with the stable cargo fmt. Install nightly with rustup install nightly for better results'; cargo fmt --all }

[unix]
npm-dev:
    cd ui && npm run dev

[windows]
npm-dev:
    cd ui; npm run dev

[unix]
npm-build: npm-install
    cd ui && npm run build
    mkdir -p crates/embedded-resources/static
    rm -rf crates/embedded-resources/static/*
    cp -r ui/dist/* crates/embedded-resources/static/

[windows]
npm-build: npm-install
    cd ui; npm run build
    New-Item -ItemType Directory -Force -Path crates/embedded-resources/static | Out-Null
    if (Test-Path crates/embedded-resources/static/*) { Remove-Item -Recurse -Force crates/embedded-resources/static/* }
    Copy-Item -Recurse -Force ui/dist/* crates/embedded-resources/static/

[unix]
npm-install:
    cd ui && npm install

[windows]
npm-install:
    cd ui; npm install

##########################################
# Commands used by the Nix package manager
##########################################
# Used to create the needed Nix expressions for the Node.js dependencies.

# Run this command everytime you edit the ui/package.json file.
[unix]
node2nix: clean-node patch-package
    node2nix --development \
    	--input ui/nix/package.json \
    	--node-env ui/nix/node-env.nix \
    	--composition ui/nix/default.nix \
    	--output ui/nix/node-package.nix

[unix]
patch-package:
    jd -o ui/nix/package.json \
    -p \
    -f patch ui/nix/package-patch.json ui/package.json || true

[windows]
node2nix:
    @echo "node2nix is not supported on Windows (Nix is Linux/macOS only)"

[windows]
patch-package:
    @echo "patch-package is not supported on Windows (Nix is Linux/macOS only)"

##########################################
# Commands used by the Github Actions CI
##########################################
# Set the target for the ci-release command.
# The target can be "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu",
# "x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl".
# It's used by the Github Actions CI to build the release binary for the specified target.

target := "x86_64-unknown-linux-gnu"

[unix]
ci-release: clean npm-build
    cross build --release --target {{ target }} --features vendored-openssl

[windows]
ci-release:
    @echo "ci-release with cross is not supported on Windows"

##########################################
# Commands for cross-rs to build the
# release binary for different targets
##########################################

[unix]
x-aarch64-musl:
    cross build --target aarch64-unknown-linux-musl --features vendored-openssl

[unix]
x-aarch64-gnu:
    cross build --target aarch64-unknown-linux-gnu --features vendored-openssl

[unix]
x-x86_64-musl:
    cross build --target x86_64-unknown-linux-musl --features vendored-openssl

[unix]
x-x86_64-gnu:
    cross build --target x86_64-unknown-linux-gnu --features vendored-openssl

[unix]
x-all: x-aarch64-musl x-aarch64-gnu x-x86_64-musl x-x86_64-gnu

[windows]
x-aarch64-musl:
    @echo "cross compilation is not supported on Windows"

[windows]
x-aarch64-gnu:
    @echo "cross compilation is not supported on Windows"

[windows]
x-x86_64-musl:
    @echo "cross compilation is not supported on Windows"

[windows]
x-x86_64-gnu:
    @echo "cross compilation is not supported on Windows"

[windows]
x-all:
    @echo "cross compilation is not supported on Windows"

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

# "true" if docker is installed, "false" otherwise
# Docker is needed for the Postgresql integration tests
# These variables only work on Unix systems

has_docker := if os_family() == "unix" { if `command -v docker > /dev/null 2>&1; echo $?` == "0" { "true" } else { "false" } } else { "false" }
test_pgdb := if has_docker == "true" { "cargo nextest run --workspace -E 'test(~postgres_)'" } else { "echo 'ERROR: Docker is not installed. The Postgresql integration tests require Docker'" }
test_ui_all_browsers := if has_docker == "true" { "cd tests && npm install && PLAYWRIGHT_UI=1 npx playwright test" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }
test_ui_chromium := if has_docker == "true" { "cd tests && npm install && PLAYWRIGHT_UI=1 npx playwright test --project=chromium" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }
test_ui_firefox := if has_docker == "true" { "cd tests && npm install && PLAYWRIGHT_UI=1 npx playwright test --project=firefox" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }
test_ui_webkit := if has_docker == "true" { "cd tests && npm install && PLAYWRIGHT_UI=1 npx playwright test --project=webkit" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }
test_ui_headed := if has_docker == "true" { "cd tests && npm install && PLAYWRIGHT_UI=1 npx playwright test --headed" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }

[unix]
docker:
    echo "{{ has_docker }}"

[windows]
docker:
    @echo "Docker check not implemented for Windows"
