############################################
# Just commands for Kellnr
#
# It's recommended to use the just commands
# instead of the cargo commands, as they
# provide additional functionality.
############################################


##########################################
# Common commands
##########################################

@_default:
    {{just_executable()}} --list

check:
	cargo check

build:
	cargo build --features vendored-openssl

build-release: npm-build
	cargo build --release --features vendored-openssl

clippy:
  cargo clippy --workspace --all-targets --all-features

# Run clippy and fail on first warning (for CI)
clippy-deny:
  cargo clippy --workspace --all-targets --all-features -- --deny warnings

# Check formatting without modifying files (for CI)
fmt-check:
  cargo fmt --all -- --check

run: npm-build build
	cargo run


test: npm-build # Run all tests which do NOT require Docker
	cargo nextest run --workspace -E 'not test(~postgres_)'

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

test-pgdb: npm-build # Run Postgresql integration tests which require Docker
	{{test_pgdb}}

test-all: test test-pgdb test-ui

clean:
	cargo clean

clean-node:
	rm -rf ui/node_modules
	rm -rf ui/package-lock.json

clean-all: clean clean-node

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

npm-build: npm-install
  cd ui && npm run build
  mkdir -p crates/embedded-resources/static
  rm -rf crates/embedded-resources/static/*
  cp -r ui/dist/* crates/embedded-resources/static/

npm-install:
	cd ui && npm install

##########################################
# Commands used by the Nix package manager
##########################################

# Used to create the needed Nix expressions for the Node.js dependencies.
# Run this command everytime you edit the ui/package.json file.
node2nix: clean-node patch-package
	node2nix --development \
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

# "true" if docker is installed, "false" otherwise
# Docker is needed for the Postgresql integration tests
has_docker := if `command -v docker > /dev/null 2>&1; echo $?` == "0" { "true" } else { "false" }

test_pgdb := if has_docker == "true" { "cargo nextest run --workspace -E 'test(~postgres_)'" } else { "echo 'ERROR: Docker is not installed. The Postgresql integration tests require Docker'" }

test_ui_all_browsers := if has_docker == "true" { "cd tests && npm install && npx playwright install --with-deps && PLAYWRIGHT_UI=1 npx playwright test" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }

test_ui_chromium := if has_docker == "true" { "cd tests && npm install && npx playwright install --with-deps && PLAYWRIGHT_UI=1 npx playwright test --project=chromium" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }

test_ui_firefox := if has_docker == "true" { "cd tests && npm install && npx playwright install --with-deps && PLAYWRIGHT_UI=1 npx playwright test --project=firefox" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }

test_ui_webkit := if has_docker == "true" { "cd tests && npm install && npx playwright install --with-deps && PLAYWRIGHT_UI=1 npx playwright test --project=webkit" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }

test_ui_headed := if has_docker == "true" { "cd tests && npm install && npx playwright install --with-deps && PLAYWRIGHT_UI=1 npx playwright test --headed" } else { "echo 'ERROR: Docker is not installed. The UI tests require Docker'" }

docker:
	echo "{{has_docker}}"
