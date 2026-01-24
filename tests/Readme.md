# Kellnr UI Tests (Playwright)

This folder contains **Playwright** based end-to-end UI tests for Kellnr.

All tests verify both backend functionality AND UI accessibility through browser automation, following the Page Object Model pattern.

## Test Suite Overview

**53 UI tests across 12 test files:**

| Test File | Tests | What It Verifies |
|-----------|-------|------------------|
| `ui-crate-settings.spec.ts` | 4 | Crate owner management, access control, version deletion |
| `ui-crate-with-data.spec.ts` | 5 | Crate display, navigation, statistics, admin features |
| `ui-crates.spec.ts` | 7 | Crates page, search, filters, empty states |
| `ui-docs.spec.ts` | 2 | Documentation generation + UI link verification |
| `ui-landing-stats.spec.ts` | 5 | Landing page statistics cards clickability |
| `ui-login.spec.ts` | 6 | Login/logout, auth, form validation, protected routes |
| `ui-me.spec.ts` | 4 | /me route, cargo login flow, token management |
| `ui-migration.spec.ts` | 1 | Database migration + UI accessibility |
| `ui-navigation.spec.ts` | 7 | Header nav, theme toggle, routing, branding |
| `ui-proxy-crates.spec.ts` | 3 | Proxy toggle, cached crates, statistics |
| `ui-s3-storage.spec.ts` | 4 | S3 storage backend + UI verification |
| `ui-user-management.spec.ts` | 5 | User management, admin promotion/demotion |

## Prerequisites

- Node.js and npm
- A Rust toolchain (for building Kellnr)
- Docker (only for S3 and migration tests)

Optional:
- Nix development shell via the repo `flake.nix`

**Note**: Playwright browsers are automatically installed when running tests via justfile commands. No manual installation needed!

## Running the tests

### From Repository Root (Recommended)

Use the justfile commands (these automatically install browsers):

```bash
# Fast: Run in Chromium only (default)
just test-ui

# Comprehensive: Run in all 3 browsers
just test-ui-all-browser

# Individual browsers
just test-ui-chromium
just test-ui-firefox
just test-ui-webkit

# Headed mode (watch tests in browser)
just test-ui-headed
```

### From tests/ Directory

**Note**: If running npm directly (not via justfile), install browsers first:

```bash
cd tests
npm install
npx playwright install --with-deps
```

Then run tests:

```bash
# Chromium only (fast, recommended for development)
npm test

# All 3 browsers (Chromium, Firefox, WebKit)
PLAYWRIGHT_UI=1 npm test

# Specific browser
npm test -- --project=chromium
npm test -- --project=firefox
npm test -- --project=webkit

# Specific test file
npm test -- ui-docs.spec.ts --project=chromium

# Headed mode (visible browser)
npm test -- --headed

# Debug mode (step through test)
npm test -- --debug
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `KELLNR_SKIP_BUILD` | - | Set to `1` to skip building Kellnr (use existing binary) |
| `KELLNR_FORCE_REBUILD` | - | Set to `1` to force rebuild even if binary exists |
| `KELLNR_BINARY_PATH` | `target/debug/kellnr` | Path to Kellnr binary |
| `KELLNR_BASE_URL` | `http://localhost:8000` | Base URL for tests |
| `PLAYWRIGHT_UI` | - | Set to `1` to enable all browsers (Chromium, Firefox, WebKit) |

## Architecture

### Page Object Model

Tests use the Page Object Model pattern for maintainability:

```
tests/src/
├── pages/               # Page Object classes
│   ├── LandingPage.ts
│   ├── LoginPage.ts
│   ├── CratePage.ts
│   ├── CratesPage.ts
│   └── NavigationPage.ts
├── lib/                 # Test infrastructure
│   ├── ui-fixtures.ts   # Playwright fixtures
│   ├── local.ts         # Local Kellnr process management
│   ├── docker.ts        # Docker container helpers (S3, migration)
│   ├── kellnr.ts        # Kellnr configuration helpers
│   └── registry.ts      # Cargo registry helpers
└── ui-*.spec.ts         # Test specifications
```

### Shared Local Process Pattern

Each test file starts a local Kellnr process that is shared by all tests in that file:
- Kellnr binary built once in `globalSetup` via `just npm-build && just build`
- Each test file spawns its own Kellnr process in `beforeAll`
- All tests in the file use the same process
- Process stopped and data directory cleaned up in `afterAll`

Data isolation is achieved via unique directories: `/tmp/kellnr-test-ui/<uuid>/`

**Exceptions:**
- **S3 tests**: Use Docker for MinIO container, local Kellnr connects to it
- **Migration tests**: Use Docker for old Kellnr version, local for new version

## Notes on Ports / Parallelism

Tests bind to fixed port `localhost:8000` because:
- Crate-local `.cargo/config.toml` files reference this fixed URL
- `crates.io` proxy downloads need stable URLs

**Concurrency:**
- All tests run **serially** with a single worker to avoid port conflicts
- Browser projects (chromium/firefox/webkit) run **sequentially** when using `PLAYWRIGHT_UI=1`

## Debugging Failures

When a test fails:

1. **Check console output** - Playwright provides clear failure traces
2. **View HTML report** - `npx playwright show-report`
3. **Inspect artifacts** - Screenshots, videos, traces attached on failure
4. **Check Kellnr logs** - Located in the test data directory (logged to console on failure)

Artifacts are stored in:
- `tests/test-results/` - Test run artifacts
- `tests/playwright-report/` - HTML report

### Debug a Specific Test

```bash
# Headed mode - watch the test run
npm test -- ui-docs.spec.ts --headed

# Slow motion - see what's happening
npm test -- ui-docs.spec.ts --headed --slow-mo=1000

# Debug mode - step through with Playwright Inspector
npm test -- ui-docs.spec.ts --debug
```

## Adding New Tests

1. **Create test file** in `tests/src/` following naming convention:
   - `ui-{feature}.spec.ts` (e.g., `ui-settings.spec.ts`)

2. **Follow Page Object Model**:
   - Create/extend page objects in `tests/src/pages/`
   - Add test IDs (`data-testid`) to UI components when needed
   - Use page object methods instead of raw selectors

3. **Use shared local process pattern**:
   ```typescript
   import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
   import { restrictToSingleWorkerBecauseFixedPorts, assertKellnrBinaryExists } from "./testUtils";

   test.describe("My Feature Tests", () => {
     restrictToSingleWorkerBecauseFixedPorts();

     let started: StartedLocalKellnr;
     let baseUrl: string;

     test.beforeAll(async () => {
       assertKellnrBinaryExists();
       started = await startLocalKellnr({
         name: "my-feature-test",
         env: {
           // Optional: test-specific environment variables
         },
       });
       baseUrl = started.baseUrl;
     });

     test.afterAll(async () => {
       if (started) {
         await started.stop();
       }
     });

     test("my test", async ({ page }) => {
       await page.goto(`${baseUrl}/some-page`);
       // Use page object methods
     });
   });
   ```

4. **Run your test**:
   ```bash
   npm test -- ui-{feature}.spec.ts --headed
   ```

## CI/CD Integration

**Recommended setup:**

- **Fast feedback**: Use `just test-ui` (Chromium only)
- **Comprehensive testing**: Use `just test-ui-all-browser` before releases
- **Parallel execution**: Not currently supported due to fixed ports

## Additional Resources

- [Playwright Documentation](https://playwright.dev)
- [Page Object Model Pattern](https://playwright.dev/docs/pom)
- [Playwright Best Practices](https://playwright.dev/docs/best-practices)
