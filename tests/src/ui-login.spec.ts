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
 */

import { test, expect, withKellnrUI, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import { LoginPage, HeaderComponent } from "./pages";
import { restrictToSingleWorkerBecauseFixedPorts } from "./testUtils";

test.describe("Login UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  test("login page displays correctly", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "login-display", authRequired: true }, async (baseUrl) => {
      await test.step("navigate to login page", async () => {
        await page.goto(`${baseUrl}/login`);
      });

      await test.step("verify login form elements", async () => {
        const loginPage = new LoginPage(page);

        // Check page title
        await expect(loginPage.signInTitle).toBeVisible();

        // Check form fields
        await expect(loginPage.usernameInput).toBeVisible();
        await expect(loginPage.passwordInput).toBeVisible();
        await expect(loginPage.rememberMeCheckbox).toBeVisible();
        await expect(loginPage.confirmButton).toBeVisible();
      });
    });
  });

  test("successful login with valid credentials", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "login-success", authRequired: true }, async (baseUrl) => {
      const loginPage = new LoginPage(page);
      const header = new HeaderComponent(page);

      await test.step("navigate to login page", async () => {
        await page.goto(`${baseUrl}/login`);
        await expect(loginPage.signInTitle).toBeVisible();
      });

      await test.step("fill in credentials and submit", async () => {
        await loginPage.fillUsername(DEFAULT_ADMIN_USER);
        await loginPage.fillPassword(DEFAULT_ADMIN_PASSWORD);
        await loginPage.clickConfirm();
      });

      await test.step("verify successful login", async () => {
        // Should redirect to home page
        await page.waitForURL("**/");

        // Should show logout button instead of login
        await expect(header.loginButton).not.toBeVisible();

        // User should be logged in (logout button visible)
        const isLoggedIn = await header.isLoggedIn();
        expect(isLoggedIn).toBe(true);
      });
    });
  });

  test("failed login with invalid credentials", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "login-fail", authRequired: true }, async (baseUrl) => {
      const loginPage = new LoginPage(page);

      await test.step("navigate to login page", async () => {
        await page.goto(`${baseUrl}/login`);
      });

      await test.step("attempt login with wrong password", async () => {
        await loginPage.fillUsername(DEFAULT_ADMIN_USER);
        await loginPage.fillPassword("wrongpassword");
        await loginPage.clickConfirm();
      });

      await test.step("verify error message", async () => {
        // Should show error alert
        await loginPage.waitForLoginError();
        const alertText = await loginPage.getAlertText();
        expect(alertText).toContain("Wrong user or password");

        // Should still be on login page
        expect(page.url()).toContain("/login");
      });
    });
  });

  test("form validation prevents empty submission", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "login-validation", authRequired: true }, async (baseUrl) => {
      const loginPage = new LoginPage(page);

      await test.step("navigate to login page", async () => {
        await page.goto(`${baseUrl}/login`);
      });

      await test.step("verify confirm button is disabled with empty fields", async () => {
        // With empty fields, button should be disabled
        const isEnabled = await loginPage.isConfirmButtonEnabled();
        expect(isEnabled).toBe(false);
      });

      await test.step("fill only username", async () => {
        await loginPage.fillUsername(DEFAULT_ADMIN_USER);
        // Button should still be disabled
        const isEnabled = await loginPage.isConfirmButtonEnabled();
        expect(isEnabled).toBe(false);
      });

      await test.step("fill password to enable button", async () => {
        await loginPage.fillPassword(DEFAULT_ADMIN_PASSWORD);
        // Button should now be enabled
        const isEnabled = await loginPage.isConfirmButtonEnabled();
        expect(isEnabled).toBe(true);
      });
    });
  });

  test("logout functionality", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "logout", authRequired: true }, async (baseUrl) => {
      const loginPage = new LoginPage(page);
      const header = new HeaderComponent(page);

      await test.step("login first", async () => {
        await page.goto(`${baseUrl}/login`);
        await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
        await page.waitForURL("**/");
      });

      await test.step("verify logged in state", async () => {
        const isLoggedIn = await header.isLoggedIn();
        expect(isLoggedIn).toBe(true);
      });

      await test.step("click logout", async () => {
        await header.clickLogout();
      });

      await test.step("verify logged out state", async () => {
        // Wait for logout to complete
        await page.waitForTimeout(1000);

        // Should show login button
        await expect(header.loginButton).toBeVisible();
      });
    });
  });

  test("redirect to login when accessing protected page while not authenticated", async ({ page }, testInfo) => {
    testInfo.setTimeout(5 * 60 * 1000);

    await withKellnrUI(testInfo, { name: "auth-redirect", authRequired: true }, async (baseUrl) => {
      await test.step("try to access settings without login", async () => {
        await page.goto(`${baseUrl}/settings`);
      });

      await test.step("verify redirect to login", async () => {
        // Should be redirected to login with redirect query param
        await page.waitForURL("**/login**");
        expect(page.url()).toContain("/login");
      });
    });
  });
});
