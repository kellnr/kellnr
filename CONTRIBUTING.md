# Contributing to Kellnr

Thank you for your interest in contributing to Kellnr! This document provides guidelines and instructions to help make the contribution process smooth and effective.

## Development Workflow

### Branching Strategy

- **Main branch**: Contains the current stable release. Do not submit pull requests directly to this branch, except for critical hotfixes.
- **Devel branch**: Contains the next version to be released. All pull requests must be submitted to this branch.

### Submitting Changes

#### For Bug Fixes

1. Fork the repository
2. Create a branch from `devel`, except for hotfixes
3. Fix the bug
4. Ensure your code is formatted with `cargo fmt` (default settings)
5. Submit a pull request to the `devel` branch

#### For New Features

1. **Open an issue first** describing the feature:
   - What is its use case?
   - Why should it be in Kellnr?
   - Any relevant technical details
2. Wait for discussion and approval
3. Fork the repository
4. Create a branch from `devel`
5. Implement the feature
6. Ensure your code is formatted with `cargo fmt` (default settings)
7. Submit a pull request to the `devel` branch
8. Reference the original issue in your pull request

## Development Environment

### Prerequisites

The following tools are needed to build Kellnr:
- [Rust](https://www.rust-lang.org/tools/install)
- [NPM / Node.js](https://docs.npmjs.com/downloading-and-installing-node-js-and-npm)
- [Docker](https://docs.docker.com/get-docker/)
- [just](https://github.com/casey/just) (task runner)

### Using Nix

For an easy development environment, you can use the provided `flake.nix`:

```bash
# Start a development shell
nix develop

# Build the project
nix build
```

### Build and Test

The project uses `just` as a task runner. Common tasks:

```bash
# List all available tasks
just -l

# Build the project (debug)
just build

# Build the project (release)
just build-release

# Build the frontend (result is placed in ./static)
just npm-build

# Test the project (without Docker integration tests, requires cargo-nextest)
just test

# Test the project (with Docker integration tests, requires cargo-nextest)
just test-all

# Run the project
just run
```

## Node Dependencies

When updating any Node.js dependencies, make sure to run:

```bash
just node2nix
```

This will update the Node dependencies for Nix as well.

## Database Migrations

When making changes that require database migrations:

1. Create a new migration in the 'crates/db/migration' directory.
2. Add the migration to the [crates/db/migration/src/lib.rs](./crates/db/migration/src/lib.rs) file.
3. Run the migrations against the PostgreSQL database backend. The Sqlite backend does not work in some edge cases with migrations.
4. Generate the entities using the Sea ORM CLI.
5. Put the generated entities in the `crates/db/migration/src/YOUR_MIGRATION_ENTITIES` directory.
6. Update the entities used by kellnr in the `crates/db/src/entity` directory.
7. Rename the `mod.rs` file to `lib.rs` in the `crates/db/src/entity` directory.


If you're using Sea ORM and need to generate entities from the database:

```bash
# Set up a local PostgreSQL container
docker run -it --rm -p 5432:5432 -e POSTGRES_PASSWORD=admin -e POSTGRES_USER=admin postgres

# Generate entities (where "kellnr-db" is the database name)
sea-orm-cli generate entity -u postgresql://admin:admin@127.0.0.1/kellnr-db
```

## Build Options

The following environment variables can be set at compile time:

- `KELLNR_VERSION`: The version of kellnr currently being compiled (default: `0.0.0-unknown`).
- `KELLNR_CONFIG_DIR`: The configuration directory (default: `./config`, `../config`, or `../../config`).
- `KELLNR_STATIC_DIR`: The static html directory (default: `./static`).

## Update Documentation

If your feature adds or modifies functionality, please update the documentation accordingly. The documentation is in a separate repository. You can find the [kellnr documentation](https://kellnr.io/documentation) here and the correponding repository here: [kellnr website repository](https://github.com/kellnr/website).

## Code Style

- All code must be formatted with `cargo fmt` using the default settings
- Follow the existing coding patterns and naming conventions
- Write meaningful commit messages
- Add tests for new functionality
- Update documentation when necessary

## Questions?

If you have any questions about contributing, please open an issue for discussion.

Thank you for contributing to Kellnr!
