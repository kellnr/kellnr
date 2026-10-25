/**
 * UI tests for user management functionality.
 *
 * Tests:
 * - Admin can view user management section
 * - Admin cannot demote themselves (button disabled)
 * - Admin can create a new user
 * - Admin can promote and demote a user
 * - Admin cannot lock themselves (button disabled)
 * - Admin cannot delete themselves (button disabled)
 * - Non-admin users cannot access user management
 * - Settings default tab and ?tab= deep links (regression guard)
 *
 * Performance: All tests share a single local Kellnr instance.
 * Note: Tests run serially and share state, so users created in earlier tests
 * are available in later tests.
 */

import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import { LoginPage, HeaderComponent } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  assertKellnrBinaryExists,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";

test.describe("User Management UI Tests", () => {
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
      name: `kellnr-usermgmt-${suffix}`,
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

  test("admin can view user management section", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to settings page
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    // Click on Users in the sidebar navigation
    const userMgmtNavItem = page.getByTestId("nav-user-management");
    await expect(userMgmtNavItem).toBeVisible();
    await userMgmtNavItem.click();
    await page.waitForTimeout(500);

    // Verify Users section header is visible in the content area
    const userMgmtHeader = page.locator(".section-header").filter({ hasText: "Users" });
    await expect(userMgmtHeader).toBeVisible();

    // Verify admin user is listed
    const adminUserItem = page.locator('[data-testid="user-item"]').filter({ hasText: DEFAULT_ADMIN_USER });
    await expect(adminUserItem).toBeVisible();

    // Verify the admin badge is shown
    const adminBadge = adminUserItem.locator(".role-badge.admin");
    await expect(adminBadge).toBeVisible();
  });

  // Regression guard: the OAuth2 config lookup on this page used to force the
  // Tokens tab unconditionally, which broke the default tab and every deep
  // link even on instances with OAuth2 switched off.
  test("settings opens the password tab by default when SSO is not enforced", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");
    await page.waitForTimeout(1000);

    await page.goto(`${baseUrl}/settings`);

    const changePwdHeader = page.locator(".section-header").filter({ hasText: "Change password" });
    await expect(changePwdHeader).toBeVisible();
  });

  test("settings honours the tab query parameter when SSO is not enforced", async ({ page }) => {
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");
    await page.waitForTimeout(1000);

    await page.goto(`${baseUrl}/settings?tab=users`);

    const userMgmtHeader = page.locator(".section-header").filter({ hasText: "Users" });
    await expect(userMgmtHeader).toBeVisible();
  });

  test("admin promote/demote button is disabled for self", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to settings page
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    // Click on Users in the sidebar navigation
    const userMgmtNavItem = page.getByTestId("nav-user-management");
    await userMgmtNavItem.click();
    await page.waitForTimeout(500);

    // Find the admin user's row
    const adminUserItem = page.locator('[data-testid="user-item"]').filter({ hasText: DEFAULT_ADMIN_USER });
    await expect(adminUserItem).toBeVisible();

    // Admins cannot demote themselves, so their own row offers no Demote button
    await expect(adminUserItem.getByTestId("user-self-badge")).toHaveText("You");
    await expect(adminUserItem.getByRole("button", { name: "Demote" })).toHaveCount(0);
  });

  test("admin can create a new user", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to settings page
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    // Click on Users in the sidebar navigation
    const userMgmtNavItem = page.getByTestId("nav-user-management");
    await userMgmtNavItem.click();
    await page.waitForTimeout(500);

    // Fill in the user creation form using labels
    const usernameField = page.locator(".form-field").filter({ hasText: "Username" }).locator("input");
    const passwordField = page.locator(".form-field").filter({ hasText: /^Password$/ }).locator("input");
    const confirmField = page.locator(".form-field").filter({ hasText: "Confirm password" }).locator("input");

    await usernameField.fill("testuser");
    await passwordField.fill("testpassword");
    await confirmField.fill("testpassword");

    // Click Create user button
    const createButton = page.getByRole("button", { name: "Create user" });
    await createButton.click();

    // Wait for the user to appear in the list
    const newUserItem = page.locator('[data-testid="user-item"]').filter({ hasText: "testuser" });
    await expect(newUserItem).toBeVisible({ timeout: 10000 });

    // Verify the new user has "User" badge (not admin)
    const userBadge = newUserItem.locator(".role-badge.user");
    await expect(userBadge).toBeVisible();

    // Verify the Promote button is available for the new user
    const promoteButton = newUserItem.getByRole("button", { name: "Promote" });
    await expect(promoteButton).toBeVisible();
    await expect(promoteButton).toBeEnabled();
  });

  test("admin can promote and demote a user", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to settings page
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    // Click on Users in the sidebar navigation
    const userMgmtNavItem = page.getByTestId("nav-user-management");
    await userMgmtNavItem.click();
    await page.waitForTimeout(500);

    // Find the test user created in the previous test
    const testUserItem = page.locator('[data-testid="user-item"]').filter({ hasText: "testuser" });
    await expect(testUserItem).toBeVisible();

    // Verify user starts as non-admin
    const userBadge = testUserItem.locator(".role-badge.user");
    await expect(userBadge).toBeVisible();

    // --- PROMOTE USER TO ADMIN ---

    // Find and click the Promote button
    const promoteButton = testUserItem.getByRole("button", { name: "Promote" });
    await expect(promoteButton).toBeVisible();
    await expect(promoteButton).toBeEnabled();
    await promoteButton.click();

    // Confirm the promotion dialog
    let confirmButton = page.getByRole("button", { name: "Confirm" });
    await expect(confirmButton).toBeVisible();
    await confirmButton.click();

    // Wait for the change to take effect
    await page.waitForTimeout(1000);

    // Verify the user now has "Admin" badge
    const adminBadge = testUserItem.locator(".role-badge.admin");
    await expect(adminBadge).toBeVisible();

    // Verify the button now says "Demote" instead of "Promote"
    const demoteButton = testUserItem.getByRole("button", { name: "Demote" });
    await expect(demoteButton).toBeVisible();
    await expect(demoteButton).toBeEnabled();

    // --- DEMOTE USER BACK TO REGULAR USER ---

    // Click the Demote button
    await demoteButton.click();

    // Confirm the demotion dialog
    confirmButton = page.getByRole("button", { name: "Confirm" });
    await expect(confirmButton).toBeVisible();
    await confirmButton.click();

    // Wait for the change to take effect
    await page.waitForTimeout(1000);

    // Verify the user now has "User" badge instead of "Admin"
    const userBadgeAfterDemotion = testUserItem.locator(".role-badge.user");
    await expect(userBadgeAfterDemotion).toBeVisible();

    // Verify the button now says "Promote" instead of "Demote"
    const promoteButtonAfterDemotion = testUserItem.getByRole("button", { name: "Promote" });
    await expect(promoteButtonAfterDemotion).toBeVisible();
  });

  test("admin cannot lock themselves", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to settings page
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    // Click on Users in the sidebar navigation
    const userMgmtNavItem = page.getByTestId("nav-user-management");
    await userMgmtNavItem.click();
    await page.waitForTimeout(500);

    // Find the admin user's row
    const adminUserItem = page.locator('[data-testid="user-item"]').filter({ hasText: DEFAULT_ADMIN_USER });
    await expect(adminUserItem).toBeVisible();

    // Admins cannot lock themselves, so their own row offers no Lock button
    const lockButton = adminUserItem.locator("button").filter({ has: page.locator(".mdi-lock-outline") });
    await expect(lockButton).toHaveCount(0);
  });

  test("admin cannot delete themselves", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to settings page
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    // Click on Users in the sidebar navigation
    const userMgmtNavItem = page.getByTestId("nav-user-management");
    await userMgmtNavItem.click();
    await page.waitForTimeout(500);

    // Find the admin user's row
    const adminUserItem = page.locator('[data-testid="user-item"]').filter({ hasText: DEFAULT_ADMIN_USER });
    await expect(adminUserItem).toBeVisible();

    // Admins cannot delete themselves, so their own row offers no Delete button
    const deleteButton = adminUserItem.locator("button").filter({ has: page.locator(".mdi-delete-outline") });
    await expect(deleteButton).toHaveCount(0);
  });

  test("non-admin user cannot access user management", async ({ page }) => {
    // This test uses the "testuser" created in "admin can create a new user" test.
    // That user was demoted back to regular user in the previous test.

    // Login as admin to verify testuser exists and is a regular user
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Logout admin
    await page.goto(`${baseUrl}`);
    await page.waitForTimeout(500);
    const header = new HeaderComponent(page);
    if (await header.isLoggedIn()) {
      await header.clickLogout();
      await page.waitForTimeout(500);
    }

    // Login as the non-admin testuser
    await page.goto(`${baseUrl}/login`);
    await loginPage.login("testuser", "testpassword");
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Try to navigate to settings page
    await page.goto(`${baseUrl}/settings`);
    await page.waitForTimeout(500);

    // Users nav item should not be visible for non-admin users
    const userMgmtNavItemNonAdmin = page.getByTestId("nav-user-management");
    await expect(userMgmtNavItemNonAdmin).not.toBeVisible();
  });
});
