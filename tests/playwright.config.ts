import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright Test config for Kellnr smoke/integration tests and UI tests.
 *
 * Test Types:
 * - Smoke tests: API + Docker orchestration tests (no browser needed)
 * - UI tests: Browser-based tests using Page Object Model
 *
 * Running tests:
 * - `npm test` - runs smoke tests only (default)
 * - `PLAYWRIGHT_UI=1 npm test` - runs UI tests in Chromium, Firefox, and WebKit
 * - `npm test -- --project=chromium` - runs UI tests in Chromium only
 * - `npm test -- --grep "ui-"` - runs only UI test files
 * - `npm test -- --grep-invert "ui-"` - runs only smoke tests
 *
 * Notes:
 * - All tests bind to localhost:8000, so they must run serially (single worker)
 * - The reporter is configured to be CI-friendly and provide good debugging
 *   artifacts (traces/screenshots/videos) especially on retries and failures
 */

// Determine which projects to enable based on environment
const enableUITests = !!process.env.PLAYWRIGHT_UI;

export default defineConfig({
  // All tests rely on binding Kellnr to localhost:8000 (stable cratesio proxy download URLs).
  // To avoid port conflicts and flakiness, run the whole suite with a single worker.
  workers: 1,
  testDir: "./src",

  // Build the local Kellnr test image exactly once before any workers start.
  globalSetup: "./global-setup.ts",

  // Keep CI noise manageable while still being informative.
  // These tests bind to localhost:8000, so they must NOT run in parallel.
  fullyParallel: false,
  forbidOnly: !!process.env.CI,

  // Retries help when waiting for containers/ports.
  retries: process.env.CI ? 1 : 0,

  timeout: 10 * 60 * 1000, // 10 minutes per test
  expect: {
    timeout: 30 * 1000,
  },

  // Reporters:
  // - "line" or "dot" works well in CI logs.
  // - "html" is great for local debugging; in CI it can be uploaded as artifact.
  reporter: process.env.CI
    ? [["line"], ["html", { open: "never" }]]
    : [["list"], ["html", { open: "on-failure" }]],

  // Default "use" options are shared by all projects.
  use: {
    baseURL: process.env.KELLNR_BASE_URL ?? "http://localhost:8000",

    // Best practice for debugging flaky integration tests:
    trace: process.env.CI ? "on-first-retry" : "retain-on-failure",
    screenshot: "only-on-failure",
    video: process.env.CI ? "retain-on-failure" : "off",

    // Keep this explicit so `--headed` works deterministically.
    headless: true,
  },

  projects: enableUITests
    ? [
        // UI tests run in multiple browsers
        {
          name: "chromium",
          use: { ...devices["Desktop Chrome"] },
          testMatch: /ui-.*\.spec\.ts/,
        },
        {
          name: "firefox",
          use: { ...devices["Desktop Firefox"] },
          testMatch: /ui-.*\.spec\.ts/,
        },
        {
          name: "webkit",
          use: { ...devices["Desktop Safari"] },
          testMatch: /ui-.*\.spec\.ts/,
        },
      ]
    : [
        // Default: smoke tests only (no browser needed)
        {
          name: "smoke",
          testIgnore: /ui-.*\.spec\.ts/,
        },
      ],

  // Store artifacts in a predictable place for CI.
  outputDir: "./test-results",
});
