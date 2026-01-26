# Contributing to Kellnr

Thank you for your interest in contributing to Kellnr! This document provides guidelines and instructions to help make the contribution process smooth and effective.

## Development Workflow

### Branching Strategy

- **Main branch**: Contains the current stable release with a tagged version. Features and fixes that should be in the next release must be merged into this branch.
- **Feature branches**: Created from `main` for new features or bug fixes that are intended for the next release.

### Submitting Changes

#### For Bug Fixes

1. Fork the repository
2. Create a branch from `main`
3. Fix the bug
4. Ensure your code is formatted with `cargo fmt` (default settings)
5. Submit a pull request to the `main` branch

#### For New Features

1. **Open an issue first** describing the feature:
   - What is its use case?
   - Why should it be in Kellnr?
   - Any relevant technical details
2. Wait for discussion and approval
3. Fork the repository
4. Create a branch from `main`
5. Implement the feature
6. Ensure your code is formatted with `cargo fmt` (default settings)
7. Submit a pull request to the `main` branch
8. Reference the original issue in your pull request

## Development Environment

### Prerequisites

The following tools are needed to build Kellnr:
- [Rust](https://www.rust-lang.org/tools/install)
- [NPM / Node.js](https://docs.npmjs.com/downloading-and-installing-node-js-and-npm)
- [Docker](https://docs.docker.com/get-docker/)
- [just](https://github.com/casey/just) (task runner)

### Using Nix

For an easy development environment, you can use the provided `flake.nix`. Install nix from (download page)[https://nixos.org/download/].

```bash
# Start a development shell
nix develop --extra-experimental-features nix-command --extra-experimental-features flakes

# Build the project
nix build --extra-experimental-features nix-command --extra-experimental-features flakes
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

# Generate entities (where "postgres" (default) is the database name)
cd <folder for entities in migrations>
sea-orm-cli generate entity -u postgresql://admin:admin@127.0.0.1/postgres
```

## Build Options

The following environment variables can be set at compile time:

- `KELLNR_VERSION`: The version of kellnr currently being compiled (default: `0.0.0-unknown`).
- `KELLNR_CONFIG_DIR`: The configuration directory (default: `./config`, `../config`, or `../../config`).
- `KELLNR_DATA_DIR`: The directory where kellnr stores its data. (d)efault: `/opt/kdata`).

## Update Documentation

If your feature adds or modifies functionality, please update the documentation accordingly. The documentation is in a separate repository. You can find the [kellnr documentation](https://kellnr.io/documentation) here and the correponding repository here: [kellnr website repository](https://github.com/kellnr/website).
## Update Helm-Chart

If your feature is configurable with settings, please update the [kellnr helm chart](https://github.com/kellnr/helm), too. The settings should be reflected in the helm chart, such that they can be set by the `values.yaml` file.

## Code Style

- All code must be formatted with `cargo fmt` using the default settings
- Follow the existing coding patterns and naming conventions
- Write meaningful commit messages
- Add tests for new functionality
- Update documentation when necessary

## Questions?

If you have any questions about contributing, please open an issue for discussion.

Thank you for contributing to Kellnr!

# Creating a new Release

As `kellnr` is a multi-repo project, creating a new release involves several steps across different repositories. Please follow the steps below to ensure a smooth release process.

0. Create a new release in the `kellnr` repository from the `main` branch [here](https://github.com/kellnr/kellnr/releases).
    - The version should be in the format `vX.Y.Z`, e.g., `v1.2.3`.
    - This builds `kellnr` and creates the release artifacts, e.g. binary files and Docker images.
0. On a successfull release, a PR for the`helm` chart for `kellnr` is automatically created.
    - Navigate to the [kellnr helm chart repository](https://github.com/kellnr/helm)
    - Review the `Chart.yaml` file with the new version number and any other relevant changes.
    - If settings have changed, update the `values.yaml` file and templates accordingly.
    - Commit the changes and push them to the `main` branch.
    - Every push to the `main` branch will automatically create a new release in the helm chart repository.
0. On a successful release of kellnr, a PR for the changelog and rss feed is automatically created.:
    - Navigate to the [kellnr documentation repository](https://github.com/kellnr/website)
    - If needed, update the [documentation](https://github.com/kellnr/website/blob/main/src/views/DocumentationV5View.vue) pages to reflect any new features or changes in the release.
    - Commit the changes and push them to the `main` branch. This will automatically deploy the updated documentation.

By following these steps, you will ensure that the new release of `kellnr` is properly documented and available for users to deploy via the helm chart. Thank you for your contributions!
