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

build-release: clean-static npm-build
	cargo build --release --features vendored-openssl

clippy:
  cargo clippy --workspace --all-targets --all-features

run: npm-build build
	cargo run

clean:
	cargo clean

test: # Run all tests which do NOT require Docker
	cargo nextest run --workspace -E 'not test(~postgres_)'

test-smoke: # Run the smoke tests which require Docker
	{{test_smoke}}

test-pgdb: # Run Postgresql integration tests which require Docker
	{{test_pgdb}}

test-all: test test-pgdb test-smoke

clean-static:
  rm -rf static/*

clean-node:
	rm -rf ui/node_modules
	rm -rf ui/package-lock.json

clean-all: clean clean-node clean-static

npm-dev:
	cd ui && npm run dev

npm-build: npm-install
	cd ui && npm run build
	mkdir -p static
	cp -r ui/dist/* static/

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

ci-release: npm-build
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

# "true" if docker is installed, "false" otherwise
# Docker is needed for the Postgresql integration tests
has_docker := if `command -v docker > /dev/null 2>&1; echo $?` == "0" { "true" } else { "false" }

test_pgdb := if has_docker == "true" { "cargo nextest run --workspace -E 'test(~postgres_)'" } else { "echo 'ERROR: Docker is not installed. The Postgresql integration tests require Docker'" }

test_smoke := if has_docker == "true" { "cd tests && lua run_tests.lua" } else { "echo 'ERROR: Docker is not installed. The smoke tests require Docker'" }

docker:
	echo "{{has_docker}}"
