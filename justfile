# Just commands for Kellnr

# "true" if docker is installed, "false" otherwise
# Docker is needed for the Postgresql integration tests
has_docker := if `command -v docker > /dev/null 2>&1; echo $?` == "0" { "true" } else { "false" }
test_all := if has_docker == "true" { "cargo nextest run --workspace" } else { "echo 'ERROR: Docker is not installed. The Postgresql integration tests require Docker'" }

docker:
	echo "{{has_docker}}"

# Commands
build:
	cargo build

build-release:
	cargo build --release

run: npm-install build
	cargo run

test:
	# Run all tests except the Postgresql integration tests, which require Docker
	cargo nextest run --workspace -E 'not binary_id(db::postgres_test)'

test-all:
	{{test_all}}

clean:
	cargo clean

clean-node:
	rm -rf ui/node_modules

clean-all: clean clean-node

npm-install:
	cd ui && npm install

# Aliases
alias b := build
alias br := build-release
alias r := run
alias t := test
alias ta := test-all
alias c := clean
