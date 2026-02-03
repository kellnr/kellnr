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
 *
 * Performance: All tests share a single local Kellnr instance.
 * Note: Tests run serially and share state, so users created in earlier tests
 * are available in later tests.
 */

import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import { LoginPage } from "./pages";
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

    // Click on User Management in the sidebar navigation
    const userMgmtNavItem = page.locator(".v-list-item-title").filter({ hasText: "User Management" });
    await expect(userMgmtNavItem).toBeVisible();
    await userMgmtNavItem.click();
    await page.waitForTimeout(500);

    // Verify User Management section header is visible in the content area
    const userMgmtHeader = page.locator(".section-header").filter({ hasText: "User Management" });
    await expect(userMgmtHeader).toBeVisible();

    // Verify admin user is listed
    const adminUserItem = page.locator('[data-testid="user-item"]').filter({ hasText: DEFAULT_ADMIN_USER });
    await expect(adminUserItem).toBeVisible();

    // Verify the admin badge is shown
    const adminBadge = adminUserItem.locator(".role-badge.admin");
    await expect(adminBadge).toBeVisible();
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

    // Click on User Management in the sidebar navigation
    const userMgmtNavItem = page.locator(".v-list-item-title").filter({ hasText: "User Management" });
    await userMgmtNavItem.click();
    await page.waitForTimeout(500);

    // Find the admin user's row
    const adminUserItem = page.locator('[data-testid="user-item"]').filter({ hasText: DEFAULT_ADMIN_USER });
    await expect(adminUserItem).toBeVisible();

    // Find the Demote button within the admin user's row
    const demoteButton = adminUserItem.getByRole("button", { name: "Demote" });
    await expect(demoteButton).toBeVisible();

    // Verify the button is disabled (self-demotion prevention)
    await expect(demoteButton).toBeDisabled();
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

    // Click on User Management in the sidebar navigation
    const userMgmtNavItem = page.locator(".v-list-item-title").filter({ hasText: "User Management" });
    await userMgmtNavItem.click();
    await page.waitForTimeout(500);

    // Fill in the user creation form using labels
    const usernameField = page.locator(".form-field").filter({ hasText: "Username" }).locator("input");
    const passwordField = page.locator(".form-field").filter({ hasText: /^Password$/ }).locator("input");
    const confirmField = page.locator(".form-field").filter({ hasText: "Confirm Password" }).locator("input");

    await usernameField.fill("testuser");
    await passwordField.fill("testpassword");
    await confirmField.fill("testpassword");

    // Click Create User button
    const createButton = page.getByRole("button", { name: "Create User" });
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

    // Click on User Management in the sidebar navigation
    const userMgmtNavItem = page.locator(".v-list-item-title").filter({ hasText: "User Management" });
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

    // Click on User Management in the sidebar navigation
    const userMgmtNavItem = page.locator(".v-list-item-title").filter({ hasText: "User Management" });
    await userMgmtNavItem.click();
    await page.waitForTimeout(500);

    // Find the admin user's row
    const adminUserItem = page.locator('[data-testid="user-item"]').filter({ hasText: DEFAULT_ADMIN_USER });
    await expect(adminUserItem).toBeVisible();

    // Find the Lock button within the admin user's row (it has icon mdi-lock-outline)
    const lockButton = adminUserItem.locator("button").filter({ has: page.locator(".mdi-lock-outline") });
    await expect(lockButton).toBeVisible();

    // Verify the lock button is disabled (self-locking prevention)
    await expect(lockButton).toBeDisabled();
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

    // Click on User Management in the sidebar navigation
    const userMgmtNavItem = page.locator(".v-list-item-title").filter({ hasText: "User Management" });
    await userMgmtNavItem.click();
    await page.waitForTimeout(500);

    // Find the admin user's row
    const adminUserItem = page.locator('[data-testid="user-item"]').filter({ hasText: DEFAULT_ADMIN_USER });
    await expect(adminUserItem).toBeVisible();

    // Find the Delete button within the admin user's row (it has icon mdi-delete-outline)
    const deleteButton = adminUserItem.locator("button").filter({ has: page.locator(".mdi-delete-outline") });
    await expect(deleteButton).toBeVisible();

    // Verify the delete button is disabled (self-deletion prevention)
    await expect(deleteButton).toBeDisabled();
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
    const logoutButton = page.getByRole("button", { name: "Logout" });
    if (await logoutButton.isVisible()) {
      await logoutButton.click();
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

    // User Management nav item should not be visible for non-admin users
    const userMgmtNavItemNonAdmin = page.locator(".v-list-item-title").filter({ hasText: "User Management" });
    await expect(userMgmtNavItemNonAdmin).not.toBeVisible();
  });
});
