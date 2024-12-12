![GitHub](https://img.shields.io/github/license/kellnr/kellnr)
![GitHub release (with filter)](https://img.shields.io/github/v/release/kellnr/kellnr)
![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/kellnr/kellnr/ci.yaml)
![GitHub Sponsors](https://img.shields.io/github/sponsors/secana)


# Kellnr - The private crate registry

Kellnr is an open-source [Rust](https://www.rust-lang.org) registry for crates. Think of [crates.io](https://crates.io) but on your own hardware.

 > [!NOTE]  
 > Find out more on: [kellnr.io](https://kellnr.io)

## Quickstart

The easiest way to get started is to use the Docker image. You can start **kellnr** with the following command:

```bash
docker run \
    -p 8000:8000 \
    -e "KELLNR_ORIGIN__HOSTNAME=localhost" ghcr.io/kellnr/kellnr:5.2.5
```

Fore more information about how to configure and run **kellnr**, check out the [documentation](https://kellnr.io/documentation).

You can find the latest binary releases here: [Kellnr Binary Releases](https://github.com/kellnr/kellnr/releases). 

For the latest Docker images, check here: [Kellnr Docker Images](https://github.com/kellnr/kellnr/pkgs/container/kellnr). 

The latest Kubernetes Helm chart can be found here: [Kellnr Helm Chart](https://github.com/kellnr/helm/releases)

## Features

- **Host crates**: Kellnr can host crates. This means that you can upload your own crates to Kellnr and use them in your projects. No extra tooling required, `cargo` works out of the box.
- **Web UI**: Kellnr comes with a web UI to manage the crates. This makes it easy to upload new crates, manage the versions and see the documentation of the crates.
- **Docs-rs support**: Kellnr supports the [docs.rs](https://docs.rs) documentation service. This means that you can host your own documentation for your crates with Kellnr.
- **Crates.io proxy**: Kellnr can act as a proxy for [crates.io](https://crates.io). This means that you can use Kellnr as a cache for crates.io to speed up the download of crates.
- **Build in Rust**: Kellnr is written in Rust. This means that you can easily extend Kellnr with your own features or fix bugs. No other dependencies are needed.
- **Multi-Db support**: Kellnr supports multiple databases. You can use Sqlite or PostgreSQL as the storage backend for Kellnr.

## Differences to crates.io

- **Private**: Kellnr is designed to be used in a corporate environment or home-labs. It is possible to host Kellnr on your own hardware, such that you can control the access to the crates.
- **Easy to host**: Kellnr is designed to be easy to host. It is possible to run Kellnr on a single machine, without the need for a complex setup. Stand-alone, Docker and Kubernetes deployments are supported.
- **User management**: Kellnr supports user management. This means that you can create users and assign them to own crates. This is useful in a corporate environment, where you want to control the access to the crates.

## Why I created Kellnr

As a security engineer and researcher I fight vulnerabilities in software for a living. With [Rust](https://www.rust-lang.org) becoming more and more popular, I see a lot of potential in the language to write secure software. However, to adapt Rust in a corporate environment, I need to be able to control the dependencies of the software I write. This is where **kellnr** comes into play. I hope that **kellnr** can accelerate the adoption of Rust in corporate environments, by providing a secure and private registry for Rust crates. In the end, I want to make the world a little bit more secure by promoting the use of Rust.

## Contribute

You are welcome to contribute to **kellnr**. Create an issue or a pull-request here on Github.

If you want to contribute with code, here are some hints to get you started.

### Prerequisites

The following tools are needed to build **kellnr**: [Rust](https://www.rust-lang.org/tools/install), [NPM / Node.js](https://docs.npmjs.com/downloading-and-installing-node-js-and-npm) and [Docker](https://docs.docker.com/get-docker/).

**kellnr** is written in Rust with a UI in [vue.js](https://vuejs.org). NPM and Node.js are only needed at build time, but not at runtime. The UI is hosted by **kellnr** itself, such that no Node.js is needed. Docker is needed for integration tests against the [PostgreSQL](https://www.postgresql.org) backend.

### Build Kellnr

The project uses [just](https://github.com/casey/just) as a task runner. Check the [justfile](./justfile) for all available tasks, or run `just -l` to see all available tasks.

```bash
# For all available tasks 
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

If you use [Nix](https://nixos.org/), you can use the provided `flake.nix` to build the project and start a development shell.

```bash
# Start a development shell
nix develop

# Build the project
nix build
```

#### Build options

The following environment variables can be set at compile time to tell kellnr where it can find some
relevant files.

- `KELLNR_CONFIG_DIR`: The configuration directory (default: `./config`, `../config`, or `../../config`).
- `KELLNR_STATIC_DIR`: The static html directory (default: `./static`).

### Sea ORM & PostgreSQL

**kellnr** uses Sqlite or PostreSQL as the storage backend for all crate related information. If you need a local PostgreSQL to test against, this Docker command sets one up on your local machine.

```bash
# Run local postgres container.
docker run -it --rm -p 5432:5432 -e POSTGRES_PASSWORD=admin -e POSTGRES_USER=admin postgres
```

If you want to generate entities with Sea ORM from the database, run:

```bash
# in the folder, where the entities should be generated, where "kellnr-db" is the database name.
sea-orm-cli generate entity -u postgresql://admin:admin@127.0.0.1/kellnr-db
```
