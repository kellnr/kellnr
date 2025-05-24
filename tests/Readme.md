# Kellnr Testing

This directory contains additional tests for the Kellnr registry. Tests that cannot be run or are cumbersome to run with `cargo test` are placed here.
This may include tests that require a specific environment or full integrations tests against a running instance of Kellnr.

## Running Tests

To install all dependencies and run the test, you can use the Nix development shell. This will set up the environment with all necessary tools and dependencies.

```bash
# In the root of the repository, where the flake.nix file is located
nix develop

# Execute the tests in the tests directory
./run_tests.lua
```

