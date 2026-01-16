/**
 * Playwright test fixtures for UI tests.
 *
 * Provides:
 * - Extended test with page object fixtures
 * - Kellnr container lifecycle management
 * - Common test utilities
 */

import { test as base, expect, type Page } from "@playwright/test";
import type { TestInfo } from "@playwright/test";
import {
  LoginPage,
  HeaderComponent,
  CratesPage,
  LandingPage,
} from "../pages";
import { startKellnr, type StartedKellnr } from "./kellnr";
import { withStartedContainer } from "./docker";
import {
  createBufferedTestLogger,
  waitForHttpOk,
  assertDockerAvailable,
} from "../testUtils";

/**
 * Default admin credentials for Kellnr.
 * These are the default credentials set when auth_required is enabled.
 */
export const DEFAULT_ADMIN_USER = "admin";
export const DEFAULT_ADMIN_PASSWORD = "admin";

/**
 * Extended test fixtures for UI tests.
 */
export type UITestFixtures = {
  /** Pre-configured LoginPage instance */
  loginPage: LoginPage;
  /** Pre-configured HeaderComponent instance */
  header: HeaderComponent;
  /** Pre-configured CratesPage instance */
  cratesPage: CratesPage;
  /** Pre-configured LandingPage instance */
  landingPage: LandingPage;
};

/**
 * Extended test with page object fixtures.
 *
 * Usage:
 *   import { test, expect } from "./lib/ui-fixtures";
 *
 *   test("example", async ({ loginPage, header }) => {
 *     await loginPage.goto();
 *     // ...
 *   });
 */
export const test = base.extend<UITestFixtures>({
  loginPage: async ({ page }, use) => {
    const loginPage = new LoginPage(page);
    await use(loginPage);
  },

  header: async ({ page }, use) => {
    const header = new HeaderComponent(page);
    await use(header);
  },

  cratesPage: async ({ page }, use) => {
    const cratesPage = new CratesPage(page);
    await use(cratesPage);
  },

  landingPage: async ({ page }, use) => {
    const landingPage = new LandingPage(page);
    await use(landingPage);
  },
});

export { expect };

/**
 * Helper to run a UI test with a Kellnr container.
 *
 * This handles:
 * - Starting the Kellnr container
 * - Waiting for HTTP readiness
 * - Cleaning up on test completion
 * - Logging and artifact collection
 *
 * Usage:
 *   test("my ui test", async ({ page }, testInfo) => {
 *     await withKellnrUI(testInfo, {}, async (baseUrl) => {
 *       await page.goto(baseUrl);
 *       // ... test code
 *     });
 *   });
 */
export async function withKellnrUI<T>(
  testInfo: TestInfo,
  options: {
    /** Name prefix for the container */
    name?: string;
    /** Additional environment variables */
    env?: Record<string, string>;
    /** Enable auth required mode */
    authRequired?: boolean;
  },
  fn: (baseUrl: string) => Promise<T>,
): Promise<T> {
  const tlog = createBufferedTestLogger(
    testInfo,
    options.name ?? "kellnr-ui-test",
  );
  const log = tlog.log;

  try {
    await assertDockerAvailable();
    log("Docker is available");

    const suffix = `${testInfo.workerIndex}-${Date.now()}`;
    const image = process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";

    const env: Record<string, string> = {
      ...options.env,
    };

    if (options.authRequired) {
      env.KELLNR_REGISTRY__AUTH_REQUIRED = "true";
    }

    log(`Starting Kellnr container with image: ${image}`);

    const startedKellnr: StartedKellnr = await startKellnr(
      {
        name: `kellnr-ui-${options.name ?? "test"}-${suffix}`,
        image,
        env,
      },
      testInfo,
    );

    return await withStartedContainer(
      testInfo,
      startedKellnr.started,
      async () => {
        log(`Waiting for HTTP 200 on ${startedKellnr.baseUrl}`);
        await waitForHttpOk(startedKellnr.baseUrl, {
          timeoutMs: 60_000,
          intervalMs: 1_000,
        });
        log("Server ready");

        return await fn(startedKellnr.baseUrl);
      },
      { alwaysCollectLogs: true },
    );
  } finally {
    await tlog.flush();
  }
}

/**
 * Helper to login with default admin credentials.
 */
export async function loginAsAdmin(page: Page): Promise<void> {
  const loginPage = new LoginPage(page);
  await loginPage.goto();
  await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
  await loginPage.waitForNavigation("/");
}

/**
 * Helper to check if auth is required by trying to access a protected page.
 */
export async function isAuthRequired(page: Page): Promise<boolean> {
  const currentUrl = page.url();
  await page.goto("/settings");
  const redirectedToLogin = page.url().includes("/login");
  await page.goto(currentUrl); // Go back
  return redirectedToLogin;
}
