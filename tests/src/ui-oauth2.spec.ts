/**
 * UI tests for OAuth2/OIDC authentication flow.
 *
 * Tests:
 * - OAuth2 button visibility when enabled/disabled
 * - OAuth2 button text customization
 * - OAuth2 login flow with mock OIDC server
 * - User auto-provisioning
 *
 * Performance: Tests use a mock OIDC server for fast, reliable testing.
 *
 * Note: All test suites run serially to avoid port conflicts on localhost:8000.
 */

import { test, expect } from "./lib/ui-fixtures";
import { LoginPage, HeaderComponent } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  assertKellnrBinaryExists,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import {
  startMockOidcServer,
  getOAuth2EnvVars,
  type StartedMockOidc,
} from "./lib/mock-oidc";

// Run all test suites serially to avoid port conflicts
test.describe.configure({ mode: "serial" });

test.describe("OAuth2 UI Tests - Disabled", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;

  test.beforeAll(async () => {
    test.setTimeout(60_000);
    assertKellnrBinaryExists();

    const suffix = `${Date.now()}`;

    // Start Kellnr WITHOUT OAuth2 enabled
    started = await startLocalKellnr({
      name: `kellnr-oauth2-disabled-${suffix}`,
      env: {
        KELLNR_OAUTH2__ENABLED: "false",
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl} (OAuth2 disabled)`);
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping Kellnr process");
      await started.stop();
      // Give the port time to be released
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  });

  test("OAuth2 button is not visible when disabled", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);

    const loginPage = new LoginPage(page);

    // Wait for page to load
    await expect(loginPage.signInTitle).toBeVisible();

    // OAuth2 button should NOT be visible
    const isVisible = await loginPage.isOAuth2ButtonVisible();
    expect(isVisible).toBe(false);

    // Divider should also not be visible
    await expect(loginPage.oauth2Divider).not.toBeVisible();
  });

  test("regular login still works when OAuth2 is disabled", async ({ page }) => {
    const loginPage = new LoginPage(page);
    const header = new HeaderComponent(page);

    await page.goto(`${baseUrl}/login`);
    await expect(loginPage.signInTitle).toBeVisible();

    // Regular login should work
    await loginPage.fillUsername("admin");
    await loginPage.fillPassword("admin");
    await loginPage.clickConfirm();

    await page.waitForURL("**/");
    const isLoggedIn = await header.isLoggedIn();
    expect(isLoggedIn).toBe(true);
  });
});

test.describe("OAuth2 UI Tests - Enabled", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let mockOidc: StartedMockOidc;
  let baseUrl: string;

  test.beforeAll(async () => {
    test.setTimeout(120_000); // More time for OIDC server startup
    assertKellnrBinaryExists();

    const suffix = `${Date.now()}`;

    // Start mock OIDC server first
    console.log("[setup] Starting mock OIDC server...");
    mockOidc = await startMockOidcServer({ name: `mock-oidc-${suffix}` });
    console.log(`[setup] Mock OIDC server ready at ${mockOidc.config.issuerUrl}`);

    // Start Kellnr WITH OAuth2 enabled
    started = await startLocalKellnr({
      name: `kellnr-oauth2-enabled-${suffix}`,
      env: {
        ...getOAuth2EnvVars(mockOidc.config),
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl} (OAuth2 enabled)`);
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping Kellnr process");
      await started.stop();
    }
    if (mockOidc) {
      console.log("[teardown] Stopping mock OIDC server");
      await mockOidc.stop();
    }
    // Give the port time to be released
    await new Promise((resolve) => setTimeout(resolve, 1000));
  });

  test("OAuth2 button is visible when enabled", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);

    const loginPage = new LoginPage(page);

    // Wait for page to load
    await expect(loginPage.signInTitle).toBeVisible();

    // OAuth2 button should be visible
    await loginPage.waitForOAuth2Button();
    const isVisible = await loginPage.isOAuth2ButtonVisible();
    expect(isVisible).toBe(true);

    // Divider should also be visible
    await expect(loginPage.oauth2Divider).toBeVisible();
  });

  test("OAuth2 button shows configured text", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);

    const loginPage = new LoginPage(page);
    await loginPage.waitForOAuth2Button();

    const buttonText = await loginPage.getOAuth2ButtonText();
    expect(buttonText).toContain("Login with SSO");
  });

  test("both login methods are available", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);

    const loginPage = new LoginPage(page);

    // Wait for page to load
    await expect(loginPage.signInTitle).toBeVisible();

    // Regular login form should be visible
    await expect(loginPage.usernameInput).toBeVisible();
    await expect(loginPage.passwordInput).toBeVisible();
    await expect(loginPage.confirmButton).toBeVisible();

    // OAuth2 button should also be visible
    await loginPage.waitForOAuth2Button();
    await expect(loginPage.oauth2Button).toBeVisible();
  });

  test("regular login still works when OAuth2 is enabled", async ({ page }) => {
    const loginPage = new LoginPage(page);
    const header = new HeaderComponent(page);

    await page.goto(`${baseUrl}/login`);
    await expect(loginPage.signInTitle).toBeVisible();

    // Regular login should still work
    await loginPage.fillUsername("admin");
    await loginPage.fillPassword("admin");
    await loginPage.clickConfirm();

    await page.waitForURL("**/");
    const isLoggedIn = await header.isLoggedIn();
    expect(isLoggedIn).toBe(true);
  });

  test("clicking OAuth2 button redirects to OIDC provider", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);

    const loginPage = new LoginPage(page);
    await loginPage.waitForOAuth2Button();

    // Click the OAuth2 button
    await loginPage.clickOAuth2Login();

    // Should redirect to the mock OIDC server's authorize endpoint
    // Wait for URL to contain /authorize (the OIDC authorization endpoint)
    await page.waitForURL(/.*\/authorize.*/, { timeout: 10000 });

    // URL should contain the OIDC authorization endpoint
    const url = page.url();
    expect(url).toContain("/authorize");
  });

  test("OAuth2 login flow creates new user", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);

    const loginPage = new LoginPage(page);
    const header = new HeaderComponent(page);

    await loginPage.waitForOAuth2Button();

    // Click the OAuth2 button
    await loginPage.clickOAuth2Login();

    // Wait for redirect to mock OIDC server
    await page.waitForURL(/.*\/authorize.*/, { timeout: 10000 });

    // The mock OIDC server shows a login form with:
    // - Input field for username/subject
    // - Textarea for optional claims JSON
    // - "SIGN-IN" button
    const usernameInput = page.locator("input").first();
    const signInButton = page.getByRole("button", { name: "Sign-in" });

    // Wait for the login form
    await usernameInput.waitFor({ state: "visible", timeout: 10000 });

    await usernameInput.fill("testuser");
    await signInButton.click();

    // Should redirect back to Kellnr after successful auth
    await page.waitForURL(`${baseUrl}/**`, { timeout: 15000 });

    // User should be logged in
    const isLoggedIn = await header.isLoggedIn();
    expect(isLoggedIn).toBe(true);
  });
});

test.describe("OAuth2 UI Tests - Custom Button Text", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let mockOidc: StartedMockOidc;
  let baseUrl: string;

  test.beforeAll(async () => {
    test.setTimeout(120_000);
    assertKellnrBinaryExists();

    const suffix = `${Date.now()}`;

    // Start mock OIDC server
    mockOidc = await startMockOidcServer({ name: `mock-oidc-custom-${suffix}` });

    // Start Kellnr with custom button text
    started = await startLocalKellnr({
      name: `kellnr-oauth2-custom-${suffix}`,
      env: {
        ...getOAuth2EnvVars(mockOidc.config),
        KELLNR_OAUTH2__BUTTON_TEXT: "Sign in with Company SSO",
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl} (custom button text)`);
  });

  test.afterAll(async () => {
    if (started) await started.stop();
    if (mockOidc) await mockOidc.stop();
  });

  test("OAuth2 button shows custom text", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);

    const loginPage = new LoginPage(page);
    await loginPage.waitForOAuth2Button();

    const buttonText = await loginPage.getOAuth2ButtonText();
    expect(buttonText).toContain("Sign in with Company SSO");
  });
});
