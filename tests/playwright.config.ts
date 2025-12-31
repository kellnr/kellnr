import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright Test config for Kellnr smoke/integration tests.
 *
 * Notes:
 * - These tests are primarily "API + Docker orchestration" today, so we keep
 *   browser projects optional/disabled by default (can be enabled later).
 * - The reporter is configured to be CI-friendly and provide good debugging
 *   artifacts (traces/screenshots/videos) especially on retries and failures.
 */
export default defineConfig({
  // Several smoke tests rely on binding Kellnr to localhost:8000 (stable cratesio proxy download URLs).
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
    // If/when you add UI tests, Playwright will respect these by default.
    baseURL: process.env.KELLNR_BASE_URL ?? "http://localhost:8000",

    // Best practice for debugging flaky integration tests:
    trace: process.env.CI ? "on-first-retry" : "retain-on-failure",
    screenshot: "only-on-failure",
    video: process.env.CI ? "retain-on-failure" : "off",

    // Keep this explicit so `--headed` works deterministically.
    headless: true,
  },

  // Optional browser projects (disabled by default in CI until you actually add UI tests).
  // You can enable them later by setting PLAYWRIGHT_UI=1 (or by editing this file).
  projects: process.env.PLAYWRIGHT_UI
    ? [
        {
          name: "chromium",
          use: { ...devices["Desktop Chrome"] },
        },
        {
          name: "firefox",
          use: { ...devices["Desktop Firefox"] },
        },
        {
          name: "webkit",
          use: { ...devices["Desktop Safari"] },
        },
      ]
    : [
        // Default "project" used for API + orchestration tests. No browser needed.
        {
          name: "smoke",
        },
      ],

  // Store artifacts in a predictable place for CI.
  outputDir: "./test-results",
});
