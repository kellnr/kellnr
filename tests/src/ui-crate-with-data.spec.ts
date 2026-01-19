/**
 * UI tests for crate-related views with actual crate data.
 *
 * These tests first upload test crates and then verify:
 * - Crate listing page with real crates
 * - Individual crate detail page
 * - Crate metadata (About tab)
 * - Dependencies display
 * - Versions tab
 * - Settings tab (admin only)
 * - Landing page statistics
 *
 * Performance: All tests share a single Kellnr container instance.
 */

import path from "node:path";
import { test, expect, DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD } from "./lib/ui-fixtures";
import { CratesPage, CratePage, LandingPage, LoginPage } from "./pages";
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

test.describe("Crate Views with Data", () => {
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
        name: `kellnr-crates-data-${suffix}`,
        image,
        env: {
          // Don't require auth for viewing tests - only for admin tests
        },
      },
      { title: "crate-with-data" } as any
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

  test("crates page shows uploaded crates", async ({ page }) => {
    await page.goto(`${baseUrl}/crates`);
    const cratesPage = new CratesPage(page);
    await cratesPage.waitForSearchResults();

    // Should not show empty state
    const hasNoCrates = await cratesPage.hasNoCrates();
    expect(hasNoCrates).toBe(false);

    // Should have our published crates
    const hasFooBar = await cratesPage.hasCrate("foo-bar");
    expect(hasFooBar).toBe(true);

    const hasTestLib = await cratesPage.hasCrate("test_lib");
    expect(hasTestLib).toBe(true);

    // Search for specific crate
    await cratesPage.search("foo");
    await cratesPage.waitForSearchResults();

    // Should find foo-bar
    const hasFooBarAfterSearch = await cratesPage.hasCrate("foo-bar");
    expect(hasFooBarAfterSearch).toBe(true);

    // Should not show test_lib (doesn't match "foo")
    const hasTestLibAfterSearch = await cratesPage.hasCrate("test_lib");
    expect(hasTestLibAfterSearch).toBe(false);

    // Clear search shows all crates
    await cratesPage.clearSearch();
    await cratesPage.waitForSearchResults();

    // Should show all crates again
    const hasFooBarAfterClear = await cratesPage.hasCrate("foo-bar");
    expect(hasFooBarAfterClear).toBe(true);
  });

  test("crate detail page displays crate information", async ({ page }) => {
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    // Verify crate header
    const name = await cratePage.getCrateName();
    expect(name).toBe("foo-bar");

    const version = await cratePage.getVersion();
    expect(version).toBe("1.0.0");

    const description = await cratePage.getDescription();
    expect(description).toContain("Test hyphens in crate names");

    // Verify About tab metadata
    await cratePage.clickTab("about");
    await page.waitForTimeout(500);

    // Check for authors (foo-bar has secana)
    const aboutContent = await page.locator(".v-card").filter({ hasText: "Authors" }).textContent();
    expect(aboutContent).toContain("secana");

    // Verify Dependencies tab
    await cratePage.clickTab("dependencies");
    await page.waitForTimeout(500);

    // Check for normal dependencies - foo-bar depends on test_lib
    // Look for the dependency name in the dependencies section
    const testLibDep = page.locator(".dep-name").filter({ hasText: "test_lib" });
    await expect(testLibDep).toBeVisible();

    // Verify Versions tab
    await cratePage.clickTab("versions");
    await page.waitForTimeout(500);

    // Should show version 1.0.0 - use first() to avoid strict mode violation (version appears multiple times)
    const versionText = page.getByText("1.0.0").first();
    await expect(versionText).toBeVisible();

    // Verify sidebar install snippet
    const installSnippet = await cratePage.getInstallSnippet();
    expect(installSnippet).toContain("foo-bar");
    expect(installSnippet).toContain("1.0.0");
  });

  test("navigate from crates list to crate detail", async ({ page }) => {
    await page.goto(`${baseUrl}/crates`);
    const cratesPage = new CratesPage(page);
    await cratesPage.waitForSearchResults();

    // Click on foo-bar crate
    await cratesPage.clickCrate("foo-bar");

    // Wait for navigation
    await page.waitForURL("**/crate?name=foo-bar**");

    // Verify crate detail page loaded
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    const name = await cratePage.getCrateName();
    expect(name).toBe("foo-bar");
  });

  test("landing page shows statistics with crates", async ({ page }) => {
    await page.goto(`${baseUrl}/`);
    const landingPage = new LandingPage(page);
    await landingPage.waitForPageLoad();

    // Verify statistics show crate count
    await landingPage.waitForStatistics();

    // Should show 2 crates (test_lib, foo-bar)
    const crateCount = await landingPage.getTotalCratesCount();
    expect(crateCount).toBe(2);

    // Verify search from landing page
    await landingPage.search("foo");

    // Should navigate to crates page with search
    await page.waitForURL("**/crates?search=foo**");

    const cratesPage = new CratesPage(page);
    await cratesPage.waitForSearchResults();

    // Should find foo-bar
    const hasFooBar = await cratesPage.hasCrate("foo-bar");
    expect(hasFooBar).toBe(true);
  });

  test("admin can see Settings and Admin tabs", async ({ page }) => {
    // Login as admin
    await page.goto(`${baseUrl}/login`);
    const loginPage = new LoginPage(page);
    await loginPage.login(DEFAULT_ADMIN_USER, DEFAULT_ADMIN_PASSWORD);
    await loginPage.waitForNavigation("/");

    // Give the session time to establish
    await page.waitForTimeout(1000);

    // Navigate to crate detail page
    await page.goto(`${baseUrl}/crate?name=foo-bar`);
    const cratePage = new CratePage(page);
    await cratePage.waitForCrateData();

    // Verify admin tabs are visible
    // Settings tab should be visible for admin
    const settingsVisible = await cratePage.isTabVisible("settings");
    expect(settingsVisible).toBe(true);

    // Admin tab should be visible for admin
    const adminVisible = await cratePage.isTabVisible("admin");
    expect(adminVisible).toBe(true);

    // Open Settings tab and verify content
    await cratePage.clickTab("settings");
    await page.waitForTimeout(500);

    // Should see crate owners section - new UI uses settings-card
    const ownersCard = page.locator(".settings-card").filter({ hasText: "Crate Owners" });
    await expect(ownersCard).toBeVisible();

    // Should see access control section - new UI uses settings-card
    const accessCard = page.locator(".settings-card").filter({ hasText: "Access Control" });
    await expect(accessCard).toBeVisible();

    // Open Admin tab and verify content
    await cratePage.clickTab("admin");
    await page.waitForTimeout(500);

    // Should see delete buttons
    await expect(cratePage.deleteVersionButton).toBeVisible();
    await expect(cratePage.deleteCrateButton).toBeVisible();

    // Should see warning about deleting - new text mentions "yanking"
    const warning = page.locator(".v-alert").filter({ hasText: "yanking" });
    await expect(warning).toBeVisible();
  });
});
