/**
 * Playwright test fixtures for UI tests.
 *
 * Provides:
 * - Extended test with page object fixtures
 * - Common test utilities and helpers
 */

import { test as base, expect, type Page } from "@playwright/test";
import {
  LoginPage,
  HeaderComponent,
  CratesPage,
  LandingPage,
  CratePage,
} from "../pages";

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
  /** Pre-configured CratePage instance */
  cratePage: CratePage;
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

  cratePage: async ({ page }, use) => {
    const cratePage = new CratePage(page);
    await use(cratePage);
  },
});

export { expect };

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
