# Kellnr Smoke Tests (Playwright Test)

This folder contains the **Playwright Test** based smoke/integration tests.

## Prerequisites

- Nix development shell via the repo `flake.nix`
- Docker installed and running
- A Rust toolchain available (the Nix dev shell provides it)
- Cargo configured to publish to the local Kellnr registry used by these tests

## Install Node dependencies

From the repository root:

- Enter the dev shell:
  - `nix develop`

- Install dependencies:
  - `cd tests`
  - `npm install`

## Running the tests

These tests expect an environment variable pointing to the docker image to test:

- `KELLNR_TEST_IMAGE` (required)
  - Example: `kellnr-test:local`

Optional:
- `KELLNR_BASE_URL` (default: `http://localhost:8000`)
- `TESTS_DEBUG=1` (enables extra debug logging in the helper utilities)
- `PLAYWRIGHT_UI=1` (enables browser projects in the Playwright config; currently these smoke tests do not require browsers)

Examples:

- Run all tests:
  - `cd tests`
  - `KELLNR_TEST_IMAGE="kellnr-test:local" npm test`

- Run a single test file:
  - `cd tests`
  - `KELLNR_TEST_IMAGE="kellnr-test:local" npx playwright test src/sparse-registry.spec.ts`

- Show the HTML report:
  - `cd tests`
  - `npx playwright show-report`

## Notes on ports / parallelism

At the moment, tests bind to fixed ports (e.g. `localhost:8000`). Because of this:
- The Playwright config limits workers in CI.
- The specs are configured as `serial` to avoid port collisions.

Once tests are adapted to use dynamic ports (or isolated networks), we can increase concurrency.

## Debugging failures

When a test fails:
- Playwright provides a clear failure trace in the console output.
- The HTML report can be opened via `npx playwright show-report`.
- The tests attach Docker logs as Playwright artifacts to make CI debugging easier.

Artifacts are stored under:
- `tests/test-results/`
- `tests/playwright-report/`

## Adding new tests

1. Create a corresponding `*.spec.ts` in `tests/src/`.
2. Reuse shared orchestration helpers from `tests/src/testUtils.ts`.
3. Prefer attaching logs on failure (and optionally also on success while the migration is ongoing).