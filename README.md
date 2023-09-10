# Kellnr - The private crate registry

Kellnr is an open-source [Rust](https://www.rust-lang.org) registry for crates. Think of [crates.io](https://crates.io) but on your own hardware.

Find out more on: [kellnr.io](https://kellnr.io)

## Contribute

You are welcome to contribute to **kellnr**. Create an issue or a pull-request here on Github.

If you want to contribute with code, here are some hints to get you started.

### Prerequisites

The following tools are needed to build **kellnr**: [Rust](https://www.rust-lang.org/tools/install), [NPM / Node.js](https://docs.npmjs.com/downloading-and-installing-node-js-and-npm) and [Docker](https://docs.docker.com/get-docker/).

**kellnr** is written in Rust with a UI in [vue.js](https://vuejs.org). NPM and Node.js are only needed at build time, but not at runtime. The UI is hosted by **kellnr** itself, such that no Node.js is needed. Docker is needed for integration tests against the [PostgreSQL](https://www.postgresql.org) backend.

### Build Kellnr

Install all NPM dependencies (only needed once):

```bash
cd ui
npm install
```

After installing the dependencies, the [build.rs](./build.rs) file does the rest.

```bash
# Build Kellnr
cargo build

# Run Kellnr
cargo run

# Test Kellnr
cargo test --all
```

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
