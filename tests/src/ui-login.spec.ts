/**
 * UI tests for the login flow.
 *
 * Tests:
 * - Login page displays correctly
 * - Successful login with valid credentials
 * - Failed login with invalid credentials
 * - Form validation (empty fields)
 * - Logout functionality
 * - Remember me functionality
 *
 * Performance: All tests share a single local Kellnr instance.
 */

import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import { LoginPage, HeaderComponent } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  assertKellnrBinaryExists,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";

test.describe("Login UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;

  test.beforeAll(async () => {
    // Local process setup is faster but still allow extra time
    test.setTimeout(60_000);

    assertKellnrBinaryExists();
    console.log("[setup] Kellnr binary is available");

    const suffix = `${Date.now()}`;

    started = await startLocalKellnr({
      name: `kellnr-login-${suffix}`,
      env: {
        KELLNR_REGISTRY__AUTH_REQUIRED: "true",
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl}`);
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping Kellnr process");
      await started.stop();
    }
  });

  test("login page displays correctly", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);

    const loginPage = new LoginPage(page);

    // Check page title
    await expect(loginPage.signInTitle).toBeVisible();

    // Check form fields
    await expect(loginPage.usernameInput).toBeVisible();
    await expect(loginPage.passwordInput).toBeVisible();
    await expect(loginPage.rememberMeCheckbox).toBeVisible();
    await expect(loginPage.confirmButton).toBeVisible();
  });

  test("successful login with valid credentials", async ({ page }) => {
    const loginPage = new LoginPage(page);
    const header = new HeaderComponent(page);

    await page.goto(`${baseUrl}/login`);
    await expect(loginPage.signInTitle).toBeVisible();

    // Fill in credentials and submit
    await loginPage.fillUsername(DEFAULT_ADMIN_USER);
    await loginPage.fillPassword(DEFAULT_ADMIN_PASSWORD);
    await loginPage.clickConfirm();

    // Should redirect to home page
    await page.waitForURL("**/");

    // Should show logout button instead of login
    await expect(header.loginButton).not.toBeVisible();

    // User should be logged in (logout button visible)
    const isLoggedIn = await header.isLoggedIn();
    expect(isLoggedIn).toBe(true);
  });

  test("failed login with invalid credentials", async ({ page }) => {
    const loginPage = new LoginPage(page);

    await page.goto(`${baseUrl}/login`);

    // Attempt login with wrong password
    await loginPage.fillUsername(DEFAULT_ADMIN_USER);
    await loginPage.fillPassword("wrongpassword");
    await loginPage.clickConfirm();

    // Should show error alert
    await loginPage.waitForLoginError();
    const alertText = await loginPage.getAlertText();
    expect(alertText).toContain("Wrong user or password");

    // Should still be on login page
    expect(page.url()).toContain("/login");
  });

  test("form validation prevents empty submission", async ({ page }) => {
    const loginPage = new LoginPage(page);

    await page.goto(`${baseUrl}/login`);

    // With empty fields, button should be disabled
    let isEnabled = await loginPage.isConfirmButtonEnabled();
    expect(isEnabled).toBe(false);

    // Fill only username
    await loginPage.fillUsername(DEFAULT_ADMIN_USER);
    // Button should still be disabled
    isEnabled = await loginPage.isConfirmButtonEnabled();
    expect(isEnabled).toBe(false);

    // Fill password to enable button
    await loginPage.fillPassword(DEFAULT_ADMIN_PASSWORD);
    // Button should now be enabled
    isEnabled = await loginPage.isConfirmButtonEnabled();
    expect(isEnabled).toBe(true);
  });

  test("logout functionality", async ({ page }) => {
    const loginPage = new LoginPage(page);
    const header = new HeaderComponent(page);

    // Login first
    await page.goto(`${baseUrl}/login`);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await page.waitForURL("**/");

    // Verify logged in state
    let isLoggedIn = await header.isLoggedIn();
    expect(isLoggedIn).toBe(true);

    // Click logout
    await header.clickLogout();

    // Wait for logout to complete
    await page.waitForTimeout(1000);

    // Should show login button
    await expect(header.loginButton).toBeVisible();
  });

  test("redirect to login when accessing protected page while not authenticated", async ({ page }) => {
    // Try to access settings without login
    await page.goto(`${baseUrl}/settings`);

    // Should be redirected to login with redirect query param
    await page.waitForURL("**/login**");
    expect(page.url()).toContain("/login");
  });
});
