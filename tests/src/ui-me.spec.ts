/**
 * UI tests for the /me route (cargo login token page).
 *
 * Tests the flow: cargo login tells users to visit /me to get a token.
 * - When logged in: /me should show the token management page
 * - When not logged in: /me should redirect to login, then back to tokens
 */

import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import { LoginPage, HeaderComponent } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  waitForHttpOk,
  assertDockerAvailable,
  createBeforeAllTestInfo,
} from "./testUtils";
import { startKellnr, type StartedKellnr } from "./lib/kellnr";

test.describe("/me Route Tests (cargo login flow)", () => {
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedKellnr;
  let baseUrl: string;

  test.beforeAll(async () => {
    test.setTimeout(120_000);
    await assertDockerAvailable();

    const image = process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";
    const suffix = `${Date.now()}`;

    started = await startKellnr(
      {
        name: `kellnr-me-${suffix}`,
        image,
        env: {
          KELLNR_REGISTRY__AUTH_REQUIRED: "true",
        },
      },
      createBeforeAllTestInfo("me-route")
    );

    baseUrl = started.baseUrl;
    await waitForHttpOk(baseUrl, { timeoutMs: 60_000, intervalMs: 1_000 });
  });

  test.afterAll(async () => {
    if (started) {
      await started.started.container.stop();
    }
  });

  test("/me redirects to login when not authenticated", async ({ page }) => {
    // Visit /me without logging in
    await page.goto(`${baseUrl}/me`);

    // Should redirect to login page
    await expect(page).toHaveURL(/\/login/);
  });

  test("/me shows tokens tab when logged in", async ({ page }) => {
    const loginPage = new LoginPage(page);
    const header = new HeaderComponent(page);

    // Login first
    await page.goto(`${baseUrl}/login`);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await page.waitForURL("**/");

    // Verify login is complete by checking logout button is visible
    await expect(header.logoutButton).toBeVisible({ timeout: 10000 });

    // Use client-side navigation to /me within the already-running Vue app
    // This avoids the race condition with Pinia persist plugin hydration
    await page.evaluate(() => {
      // Access Vue Router and navigate
      const router = (window as unknown as { __VUE_ROUTER__?: { push: (path: string) => void } }).__VUE_ROUTER__;
      if (router) {
        router.push('/me');
      } else {
        // Fallback: use history API which Vue Router listens to
        window.history.pushState({}, '', '/me');
        window.dispatchEvent(new PopStateEvent('popstate'));
      }
    });

    // Wait for Vue Router to process the navigation
    await page.waitForTimeout(500);

    // Should redirect to settings with tokens tab visible
    await expect(page).toHaveURL(/\/settings/, { timeout: 10000 });

    // Tokens tab should be active (Authentication Tokens heading visible)
    await expect(page.locator("span:text('Authentication Tokens')")).toBeVisible();
  });

  test("login redirects back to /me (tokens) after authentication", async ({ page }) => {
    const loginPage = new LoginPage(page);

    // Visit /me without logging in - should redirect to login with redirect param
    await page.goto(`${baseUrl}/me`);
    await expect(page).toHaveURL(/\/login/);

    // Verify the redirect query param is set
    const url = new URL(page.url());
    expect(url.searchParams.get("redirect")).toMatch(/\/me|\/settings/);

    // Login
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);

    // Should redirect back to settings with tokens tab
    await expect(page).toHaveURL(/\/settings/, { timeout: 10000 });
    await expect(page.locator("span:text('Authentication Tokens')")).toBeVisible();
  });

  test("can create token from /me page", async ({ page }) => {
    const loginPage = new LoginPage(page);
    const header = new HeaderComponent(page);

    // Login first
    await page.goto(`${baseUrl}/login`);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await page.waitForURL("**/");

    // Verify login is complete
    await expect(header.logoutButton).toBeVisible({ timeout: 10000 });

    // Use client-side navigation to /me
    await page.evaluate(() => {
      window.history.pushState({}, '', '/me');
      window.dispatchEvent(new PopStateEvent('popstate'));
    });
    await page.waitForTimeout(500);

    // Should be on settings page with tokens tab
    await expect(page).toHaveURL(/\/settings/, { timeout: 10000 });

    // Should see create token button
    const createButton = page.getByRole("button", { name: /create/i });
    await expect(createButton).toBeVisible();
  });
});
