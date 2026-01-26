![GitHub](https://img.shields.io/github/license/kellnr/kellnr)
![GitHub release (with filter)](https://img.shields.io/github/v/release/kellnr/kellnr)
![Tests](https://img.shields.io/github/actions/workflow/status/kellnr/kellnr/test.yaml?label=tests)
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
    -e "KELLNR_ORIGIN__HOSTNAME=localhost" ghcr.io/kellnr/kellnr:5
```

For more information about how to configure and run **kellnr**, check out the [documentation](https://kellnr.io/documentation).

## CLI Usage

When running kellnr as a standalone binary, use the following commands:

```bash
# Show help and available commands
kellnr

# Start the server
kellnr run

# Start with custom data directory and port
kellnr run --registry-data-dir /var/lib/kellnr --local-port 8080

# Show all available run options
kellnr run --help

# Show current configuration as TOML
kellnr config show

# Create a default configuration file
kellnr config init

# Create config file at custom path
kellnr config init -o /etc/kellnr/kellnr.toml

# Use a specific configuration file
kellnr -c /path/to/kellnr.toml run
```

Configuration can be provided through (in order of priority):
1. **CLI arguments** (highest priority)
2. **Environment variables** (e.g., `KELLNR_REGISTRY__DATA_DIR`)
3. **Configuration file** (TOML format)
4. **Default values** (lowest priority)

You can find the latest binary releases here: [Kellnr Binary Releases](https://github.com/kellnr/kellnr/releases). 

For the latest Docker images, check here: 

- [Kellnr Docker Images](https://github.com/kellnr/kellnr/pkgs/container/kellnr) 
- [Kellnr Minimal Docker Images](https://github.com/kellnr/kellnr/pkgs/container/kellnr-minimal)

The latest Kubernetes Helm chart can be found here: [Kellnr Helm Chart](https://github.com/kellnr/helm/releases)

## Features

- **Host crates**: Kellnr can host crates. This means that you can upload your own crates to Kellnr and use them in your projects. No extra tooling required, `cargo` works out of the box.
- **Web UI**: Kellnr comes with a web UI to manage the crates. This makes it easy to upload new crates, manage the versions and see the documentation of the crates.
- **Docs-rs support**: Kellnr supports the [docs.rs](https://docs.rs) documentation service. This means that you can host your own documentation for your crates with Kellnr.
- **Crates.io proxy**: Kellnr can act as a proxy for [crates.io](https://crates.io). This means that you can use Kellnr as a cache for crates.io to speed up the download of crates.
- **Build in Rust**: Kellnr is written in Rust. This means that you can easily extend Kellnr with your own features or fix bugs. No other dependencies are needed.
- **Multi-Db support**: Kellnr supports multiple databases. You can use Sqlite or PostgreSQL as the storage backend for Kellnr.
- **Local File System or S3**: Kellnr supports the local file system or S3 as the storage backend for the crates.
- **User and group management**: Kellnr supports user and group management. This means that you can create users and groups and assign them to crates. This is useful in a corporate environment, where you want to control the access to the crates. You can create read-only users or require authentication for crate-pulls.

## Differences to crates.io

- **Private**: Kellnr is designed to be used in a corporate environment or home-labs. It is possible to host Kellnr on your own hardware, such that you can control the access to the crates.
- **Easy to host**: Kellnr is designed to be easy to host. It is possible to run Kellnr on a single machine, without the need for a complex setup. Stand-alone, Docker and Kubernetes deployments are supported.
- **User management**: Kellnr supports user management. This means that you can create users and assign them to own crates. This is useful in a corporate environment, where you want to control the access to the crates.

## Why I created Kellnr

As a security engineer and researcher I fight vulnerabilities in software for a living. With [Rust](https://www.rust-lang.org) becoming more and more popular, I see a lot of potential in the language to write secure software. However, to adapt Rust in a corporate environment, I need to be able to control the dependencies of the software I write. This is where **kellnr** comes into play. I hope that **kellnr** can accelerate the adoption of Rust in corporate environments, by providing a secure and private registry for Rust crates. In the end, I want to make the world a little bit more secure by promoting the use of Rust.

## Contribute

You are welcome to contribute to **kellnr**. Create an issue or a pull-request here on Github.

If you want to contribute with code, please read the [contributing guide](CONTRIBUTING.md) first.
