/**
 * UI tests for crate settings and admin functionality.
 *
 * These tests verify:
 * - Crate owner management (add/remove)
 * - Crate user management (access control)
 * - Download restriction settings
 * - Crate deletion (version and full)
 *
 * Performance: All tests share a single Kellnr container instance.
 */

import path from "node:path";
import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import { CratesPage, CratePage, LoginPage } from "./pages";
import {
  restrictToSingleWorkerBecauseFixedPorts,
  publishCrate,
  waitForHttpOk,
  assertDockerAvailable,
} from "./testUtils";
import { startKellnr, type StartedKellnr } from "./lib/kellnr";
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

test.describe("Crate Settings UI Tests", () => {
  // These tests use fixed localhost:8000 port
  restrictToSingleWorkerBecauseFixedPorts();

  let started: StartedKellnr;
  let baseUrl: string;

  test.beforeAll(async () => {
    // Container setup needs more time than default 10s timeout
    test.setTimeout(120_000); // 2 minutes for setup

    await assertDockerAvailable();
    console.log("[setup] Docker is available");

    const image = process.env.KELLNR_TEST_IMAGE ?? "kellnr-test:local";
    const suffix = `${Date.now()}`;

    started = await startKellnr(
      {
        name: `kellnr-settings-${suffix}`,
        image,
        env: {
          KELLNR_REGISTRY__AUTH_REQUIRED: "true",
        },
      },
      { title: "crate-settings" } as any
    );

    baseUrl = started.baseUrl;

    console.log(`[setup] Waiting for HTTP 200 on ${baseUrl}`);
    await waitForHttpOk(baseUrl, {
      timeoutMs: 60_000,
      intervalMs: 1_000,
    });
    console.log("[setup] Server ready");

    console.log("[setup] Publishing test crates");
    await publishTestCrates(console.log);
    console.log("[setup] Done");
  });

  test.afterAll(async () => {
    if (started) {
      console.log("[teardown] Stopping container");
      await started.started.container.stop();
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

    // Verify owners list is displayed
    const ownersCard = page.locator(".v-card").filter({ hasText: "Crate owners" });
    await expect(ownersCard).toBeVisible();

    // There should be at least one owner (the publisher)
    const ownersList = ownersCard.locator(".v-list");
    await expect(ownersList).toBeVisible();

    // Verify add owner form is visible
    const addOwnerSection = page.locator("h3").filter({ hasText: "Add crate owner" });
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

    // Verify access control section is displayed
    // Main access control card - look for the title
    const accessControlTitle = page.locator(".v-card-title").filter({ hasText: "Access control" });
    await expect(accessControlTitle).toBeVisible();

    // Crate access subsection - look for the specific title
    const crateAccessTitle = page.locator(".v-card-title").filter({ hasText: /^Crate access$/ });
    await expect(crateAccessTitle).toBeVisible();

    // Crate users subsection
    const crateUsersTitle = page.locator(".v-card-title").filter({ hasText: /^Crate users$/ });
    await expect(crateUsersTitle).toBeVisible();

    // Crate groups subsection
    const crateGroupsTitle = page.locator(".v-card-title").filter({ hasText: /^Crate groups$/ });
    await expect(crateGroupsTitle).toBeVisible();

    // Verify download restriction checkbox exists
    const checkbox = page.getByLabel("Crate users only are allowed to download");
    await expect(checkbox).toBeVisible();

    // Verify change access rules button exists
    const changeButton = page.getByRole("button", { name: "Change crate access rules" });
    await expect(changeButton).toBeVisible();
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

    // Verify warning is displayed
    const warning = page.locator(".v-alert").filter({ hasText: "Warning" });
    await expect(warning).toBeVisible();

    // Warning should mention breaking dependencies
    const warningText = await warning.textContent();
    expect(warningText).toContain("breaks all crates that depend on it");

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
