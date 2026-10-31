/**
 * UI tests for crate settings and admin functionality.
 *
 * These tests verify:
 * - Crate owner management (add/remove)
 * - Crate user management (access control)
 * - Download restriction settings
 * - Crate deletion (version and full)
 *
 * Performance: All tests share a single local Kellnr instance.
 */

import path from "node:path";
import type { Page } from "@playwright/test";
import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import { CratesPage, CratePage, LoginPage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  publishCrate,
  assertKellnrBinaryExists,
} from "./testUtils";
import { startLocalKellnr, type StartedLocalKellnr } from "./lib/local";
import { extractRegistryTokenFromCargoConfig } from "./lib/registry";

/**
 * Helper to publish test crates to a running Kellnr instance.
 * Uses test-auth-req crates which have no crates.io dependencies.
 */
async function publishTestCrates(log: (msg: string) => void): Promise<void> {
  const registry = "kellnr-test";
  const tokenSourceCrateDir = path.resolve(
    process.cwd(),
    "crates",
    "test-auth-req",
    "foo-bar"
  );
  const registryToken = extractRegistryTokenFromCargoConfig({
    crateDir: tokenSourceCrateDir,
    registryName: registry,
  });

  log("Publishing crate: test_lib");
  await publishCrate({
    cratePath: "tests/crates/test-auth-req/test_lib",
    registry,
    registryToken,
  });

  log("Publishing crate: foo-bar");
  await publishCrate({
    cratePath: "tests/crates/test-auth-req/foo-bar",
    registry,
    registryToken,
  });
}

/**
 * Logs in and waits until the server has accepted the login, so the session
 * cookie is set when the next request goes out.
 */
async function loginAndWait(page: Page, baseUrl: string, user: string, password: string): Promise<void> {
  await page.goto(`${baseUrl}/login`);
  const loggedIn = page.waitForResponse(
    (r) => r.url().endsWith("/api/v1/auth/login") && r.ok()
  );
  await new LoginPage(page).login(user, password);
  await loggedIn;
}

/**
 * Calls the API from the page, so the request carries the session cookie.
 * Returns the HTTP status.
 */
async function apiCall(page: Page, method: string, url: string, body?: unknown): Promise<number> {
  return page.evaluate(async ({ method, url, body }) => {
    const response = await fetch(url, {
      method,
      headers: { "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    return response.status;
  }, { method, url, body });
}

test.describe("Crate Settings UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedLocalKellnr;
  let baseUrl: string;

  test.beforeAll(async () => {
    // Setup needs more time for publishing crates
    test.setTimeout(120_000);

    assertKellnrBinaryExists();
    console.log("[setup] Kellnr binary is available");

    const suffix = `${Date.now()}`;

    started = await startLocalKellnr({
      name: `kellnr-settings-${suffix}`,
      env: {
        KELLNR_REGISTRY__AUTH_REQUIRED: "true",
      },
    });

    baseUrl = started.baseUrl;
    console.log(`[setup] Server ready at ${baseUrl}`);

    console.log("[setup] Publishing test crates");
    await publishTestCrates(console.log);
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping Kellnr process");
      await started.stop();
    }
  });

  test("admin can view and modify crate owners", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to crate settings
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();
    await cratePage.clickTab("settings");
    await page.waitForTimeout(500);

    // Verify owners list is displayed - new UI uses settings-card with settings-header
    const ownersCard = page.locator(".settings-card").filter({ hasText: "Crate owners" });
    await expect(ownersCard).toBeVisible();

    // There should be at least one owner (the publisher) - new UI uses settings-list
    const ownersList = ownersCard.locator(".settings-list");
    await expect(ownersList).toBeVisible();

    // Verify add owner form is visible - new UI uses settings-form-header
    const addOwnerSection = page.locator(".settings-form-header").filter({ hasText: "Add crate owner" });
    await expect(addOwnerSection).toBeVisible();

    // Add button should be visible
    const addButton = page.locator(".v-card").filter({ hasText: "Add crate owner" }).getByRole("button", { name: "Add" });
    await expect(addButton).toBeVisible();
  });

  test("admin can view crate access control settings", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to crate settings
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();
    await cratePage.clickTab("settings");
    await page.waitForTimeout(500);

    // Verify access control section is displayed - new UI uses settings-card
    const accessControlCard = page.locator(".settings-card").filter({ hasText: "Access control" });
    await expect(accessControlCard).toBeVisible();

    // Download restrictions subsection - look for the subsection header
    const downloadRestrictionsSection = page.locator(".settings-subsection-header").filter({ hasText: "Download restrictions" });
    await expect(downloadRestrictionsSection).toBeVisible();

    // Crate users subsection
    const crateUsersSection = page.locator(".settings-subsection-header").filter({ hasText: "Crate users" });
    await expect(crateUsersSection).toBeVisible();

    // Crate groups subsection
    const crateGroupsSection = page.locator(".settings-subsection-header").filter({ hasText: "Crate groups" });
    await expect(crateGroupsSection).toBeVisible();

    // Verify download restriction checkbox exists - new label text
    const checkbox = page.getByLabel("Restrict downloads to crate users only");
    await expect(checkbox).toBeVisible();

    // Verify save changes button exists - new UI has "Save changes" instead
    const saveButton = page.getByRole("button", { name: "Save changes" });
    await expect(saveButton).toBeVisible();
  });

  test("owners and admins see who can access a restricted crate", async ({ page, browser }) => {
    // A user and a group that get access, created through the API with the admin session
    await loginAndWait(page, baseUrl, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);

    const reader = { name: "badge-reader", pwd1: "badge-reader-pwd", pwd2: "badge-reader-pwd" };
    expect(await apiCall(page, "POST", "/api/v1/users", reader)).toBe(200);
    expect(await apiCall(page, "POST", "/api/v1/groups", { name: "badge-group" })).toBe(200);

    const cratePage = new CratePage(page);
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    await cratePage.waitForCrateData();

    // No badge while downloads are not restricted
    await expect(cratePage.crateTitle).toBeVisible();
    await expect(cratePage.accessBadge).toHaveCount(0);

    // Restricting the crate shows the badge, adding access updates it
    await cratePage.toggleDownloadRestriction();
    await expect(cratePage.accessBadge).toHaveText("Restricted: 0 users, 0 groups");
    // This instance runs with auth_required = true, so no warning
    await expect(cratePage.accessBadge).toHaveAttribute("data-auth-required", "true");
    await expect(cratePage.accessBadge).not.toHaveClass(/access-badge--warning/);

    await cratePage.addCrateUser(reader.name);
    await expect(cratePage.accessBadge).toHaveText("Restricted: 1 user, 0 groups");

    expect(await apiCall(page, "PUT", "/api/v1/crates/foo-bar/crate_groups/badge-group")).toBe(200);
    await page.reload();
    await cratePage.waitForCrateData();
    await expect(cratePage.accessBadge).toHaveText("Restricted: 1 user, 1 group");

    // The badge opens the settings tab at the access lists
    await cratePage.clickTab("about");
    await cratePage.accessBadge.click();
    await expect(page).toHaveURL(/tab=settings/);
    await expect(page.locator("#crate-access")).toBeInViewport();
    await expect(page.getByTestId("settings-user-name")).toHaveText([reader.name]);

    // A crate user who does not own the crate sees no badge
    const readerContext = await browser.newContext();
    try {
      const readerPage = await readerContext.newPage();
      await loginAndWait(readerPage, baseUrl, reader.name, reader.pwd1);
      await readerPage.goto(`${baseUrl}/crate?name=foo-bar`);
      const readerCratePage = new CratePage(readerPage);
      await readerCratePage.waitForCrateData();
      await expect(readerCratePage.crateTitle).toBeVisible();
      await expect(readerCratePage.settingsTab).toHaveCount(0);
      await expect(readerCratePage.accessBadge).toHaveCount(0);
    } finally {
      await readerContext.close();
    }

    // Lift the restriction again for the tests below
    await cratePage.toggleDownloadRestriction();
    await expect(cratePage.accessBadge).toHaveCount(0);
  });

  test("admin can delete a crate version", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to crate admin tab
    await page.goto(`${baseUrl}/crate?name=test_lib`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();
    await cratePage.clickTab("admin");
    await page.waitForTimeout(500);

    // Verify delete buttons are visible
    await expect(cratePage.deleteVersionButton).toBeVisible();
    await expect(cratePage.deleteCrateButton).toBeVisible();

    // Verify warning is displayed - new text mentions "yanking"
    const warning = page.locator(".v-alert").filter({ hasText: "yanking" });
    await expect(warning).toBeVisible();

    // Warning should mention yanking as an alternative
    const warningText = await warning.textContent();
    expect(warningText).toContain("yanking");

    // Delete the crate version
    // Set up dialog handler before clicking
    page.on("dialog", (dialog) => dialog.accept());

    await cratePage.deleteVersionButton.click();

    // Should redirect to crates page after deletion
    await page.waitForURL("**/crates**");

    // Verify crate is deleted
    const cratesPage = new CratesPage(page);
    await cratesPage.waitForSearchResults();

    // test_lib should no longer be visible
    const hasTestLib = await cratesPage.hasCrate("test_lib");
    expect(hasTestLib).toBe(false);

    // foo-bar should still be there
    const hasFooBar = await cratesPage.hasCrate("foo-bar");
    expect(hasFooBar).toBe(true);
  });

  test("non-logged-in user cannot see Settings or Admin tabs", async ({ page }) => {
    // Navigate to crate detail page WITHOUT logging in
    await page.goto(`${baseUrl}/crate?name=foo-bar`);

    // With auth_required, we should be redirected to login
    await page.waitForURL("**/login**");

    // Verify we're on the login page
    const loginPage = new LoginPage(page);
    const isOnLoginPage = await loginPage.isOnLoginPage();
    expect(isOnLoginPage).toBe(true);
  });
});
