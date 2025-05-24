# Kellnr Testing

This directory contains additional tests for the Kellnr registry. Tests that cannot be run or are cumbersome to run with `cargo test` are placed here.
This may include tests that require a specific environment or full integrations tests against a running instance of Kellnr.

## Running Tests

To install all dependencies and run the test, you can use the Nix development shell. This will set up the environment with all necessary tools and dependencies.

The only requirement that is not handled by the `flake.nix` file is `Docker`. You need to have Docker installed and running on your machine to run the tests that require a running instance of Kellnr or dependencies like a Postgresql database or S3 storage.

```bash
# In the root of the repository, where the flake.nix file is located
nix develop

# Run all tests
just test-all
```

